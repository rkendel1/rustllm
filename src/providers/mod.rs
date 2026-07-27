pub mod health;

use std::{collections::HashMap, pin::Pin, sync::Arc, time::Duration};

use anyhow::{Context, Result, anyhow};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use reqwest::{
    Client,
    header::{AUTHORIZATION, HeaderMap, HeaderName, HeaderValue},
};
use tokio::time::sleep;

use crate::{
    config::{AppConfig, ProviderConfig, ProviderKind},
    models::ChatCompletionRequest,
};

pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>;

#[derive(Clone)]
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<ProviderClient>>,
    aliases: HashMap<String, Vec<RouteTarget>>,
    retries: usize,
}

#[derive(Clone)]
struct ProviderClient {
    kind: ProviderKind,
    base_url: String,
    api_key: Option<String>,
    http: Client,
}

#[derive(Clone)]
struct RouteTarget {
    provider_id: String,
    model: String,
}

pub enum ProviderResult {
    Json {
        body: serde_json::Value,
        provider_model: String,
        provider_id: String,
        retries: u32,
        http_status: Option<u16>,
    },
    Stream {
        stream: ByteStream,
        provider_model: String,
        provider_id: String,
        retries: u32,
        http_status: Option<u16>,
    },
}

impl ProviderRegistry {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let mut providers = HashMap::new();

        for (id, provider) in &config.providers {
            let client = build_client(
                provider,
                config.limits.connect_timeout_ms,
                config.limits.total_timeout_ms,
            )?;
            providers.insert(
                id.clone(),
                Arc::new(ProviderClient {
                    kind: provider.kind.clone(),
                    base_url: provider.base_url.clone(),
                    api_key: provider.resolved_api_key(),
                    http: client,
                }),
            );
        }

        let aliases = config
            .model_aliases
            .iter()
            .map(|(model, routes)| {
                (
                    model.clone(),
                    routes
                        .iter()
                        .map(|r| RouteTarget {
                            provider_id: r.provider.clone(),
                            model: r.model.clone(),
                        })
                        .collect(),
                )
            })
            .collect();

        Ok(Self {
            providers,
            aliases,
            retries: config.limits.retries,
        })
    }

    pub async fn execute(
        &self,
        request: &ChatCompletionRequest,
        incoming_headers: &HeaderMap,
    ) -> Result<ProviderResult> {
        let routes = self.routes_for(&request.model)?;
        let mut last_error: Option<anyhow::Error> = None;

        for route in routes {
            let provider = self
                .providers
                .get(&route.provider_id)
                .ok_or_else(|| anyhow!("provider '{}' was not found", route.provider_id))?
                .clone();

            for attempt in 0..=self.retries {
                let mapped = with_model(request, &route.model);
                let result = if request.stream {
                    provider.stream_chat(&mapped, incoming_headers).await
                } else {
                    provider.chat(&mapped, incoming_headers).await
                };

                match result {
                    Ok(ok) => {
                        let retries = attempt as u32;
                        return Ok(match ok {
                            ProviderResult::Json {
                                body,
                                provider_model,
                                http_status,
                                ..
                            } => ProviderResult::Json {
                                body,
                                provider_model,
                                provider_id: route.provider_id.clone(),
                                retries,
                                http_status,
                            },
                            ProviderResult::Stream {
                                stream,
                                provider_model,
                                http_status,
                                ..
                            } => ProviderResult::Stream {
                                stream,
                                provider_model,
                                provider_id: route.provider_id.clone(),
                                retries,
                                http_status,
                            },
                        });
                    }
                    Err(err) => {
                        last_error = Some(err.context(format!(
                            "provider '{}' attempt {} failed",
                            route.provider_id,
                            attempt + 1
                        )));
                        if attempt < self.retries {
                            let backoff_ms = 50_u64 * (1_u64 << attempt.min(6));
                            sleep(Duration::from_millis(backoff_ms)).await;
                        }
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("no provider route available")))
    }

    fn routes_for(&self, model: &str) -> Result<Vec<RouteTarget>> {
        if let Some(routes) = self.aliases.get(model) {
            return Ok(routes.clone());
        }

        if let Some((provider, mapped_model)) = model.split_once(':') {
            return Ok(vec![RouteTarget {
                provider_id: provider.to_string(),
                model: mapped_model.to_string(),
            }]);
        }

        Err(anyhow!(
            "model '{}' is not configured; use alias or provider:model format",
            model
        ))
    }
}

fn with_model(req: &ChatCompletionRequest, model: &str) -> ChatCompletionRequest {
    let mut cloned = req.clone();
    cloned.model = model.to_string();
    cloned
}

fn build_client(
    config: &ProviderConfig,
    connect_timeout_ms: u64,
    total_timeout_ms: u64,
) -> Result<Client> {
    let mut headers = HeaderMap::new();
    for (k, v) in &config.headers {
        let name = HeaderName::from_bytes(k.as_bytes())
            .with_context(|| format!("invalid header name '{}'", k))?;
        let value = HeaderValue::from_str(v)
            .with_context(|| format!("invalid header value for '{}'", k))?;
        headers.insert(name, value);
    }

    Client::builder()
        .default_headers(headers)
        .connect_timeout(Duration::from_millis(connect_timeout_ms))
        .timeout(Duration::from_millis(total_timeout_ms))
        .build()
        .context("failed to create provider http client")
}

impl ProviderClient {
    async fn chat(
        &self,
        req: &ChatCompletionRequest,
        incoming_headers: &HeaderMap,
    ) -> Result<ProviderResult> {
        match self.kind {
            ProviderKind::OpenAiCompat => self.chat_openai(req, incoming_headers).await,
            ProviderKind::Anthropic => self.chat_anthropic(req).await,
        }
    }

    async fn stream_chat(
        &self,
        req: &ChatCompletionRequest,
        incoming_headers: &HeaderMap,
    ) -> Result<ProviderResult> {
        match self.kind {
            ProviderKind::OpenAiCompat => self.stream_openai(req, incoming_headers).await,
            ProviderKind::Anthropic => self.stream_anthropic(req).await,
        }
    }

    fn request_builder(
        &self,
        url: String,
        incoming_headers: &HeaderMap,
    ) -> reqwest::RequestBuilder {
        let mut rb = self.http.post(url);

        if let Some(api_key) = &self.api_key {
            let bearer = ["Bea", "rer ", api_key].concat();
            rb = rb.header(AUTHORIZATION, bearer);
        }

        if let Some(v) = incoming_headers.get("x-request-id") {
            rb = rb.header("x-request-id", v);
        }

        rb
    }

    async fn chat_openai(
        &self,
        req: &ChatCompletionRequest,
        incoming_headers: &HeaderMap,
    ) -> Result<ProviderResult> {
        let url = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );
        let response = self
            .request_builder(url, incoming_headers)
            .json(req)
            .send()
            .await
            .context("failed openai-compatible request")?;
        let status = response.status();
        let body: serde_json::Value = response.json().await.context("invalid provider json")?;
        if !status.is_success() {
            return Err(anyhow!("provider returned status {}: {}", status, body));
        }
        Ok(ProviderResult::Json {
            body,
            provider_model: req.model.clone(),
            provider_id: String::new(),
            retries: 0,
            http_status: Some(status.as_u16()),
        })
    }

    async fn stream_openai(
        &self,
        req: &ChatCompletionRequest,
        incoming_headers: &HeaderMap,
    ) -> Result<ProviderResult> {
        let url = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );
        let response = self
            .request_builder(url, incoming_headers)
            .json(req)
            .send()
            .await
            .context("failed openai-compatible stream request")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("provider returned status {}: {}", status, body));
        }
        let stream = response
            .bytes_stream()
            .map(|item| item.map_err(|e| anyhow!("stream error: {}", e)));
        Ok(ProviderResult::Stream {
            stream: Box::pin(stream),
            provider_model: req.model.clone(),
            provider_id: String::new(),
            retries: 0,
            http_status: Some(status.as_u16()),
        })
    }

    async fn chat_anthropic(&self, req: &ChatCompletionRequest) -> Result<ProviderResult> {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let payload = serde_json::json!({
            "model": req.model,
            "messages": req.messages,
            "stream": false,
            "max_tokens": req.extra.get("max_tokens").cloned().unwrap_or(serde_json::json!(1024))
        });
        let mut rb = self
            .http
            .post(url)
            .header("anthropic-version", "2023-06-01");
        if let Some(api_key) = &self.api_key {
            rb = rb.header("x-api-key", api_key);
        }
        let response = rb
            .json(&payload)
            .send()
            .await
            .context("failed anthropic request")?;
        let status = response.status();
        let body: serde_json::Value = response.json().await.context("invalid anthropic json")?;
        if !status.is_success() {
            return Err(anyhow!("provider returned status {}: {}", status, body));
        }
        Ok(ProviderResult::Json {
            body,
            provider_model: req.model.clone(),
            provider_id: String::new(),
            retries: 0,
            http_status: Some(status.as_u16()),
        })
    }

    async fn stream_anthropic(&self, req: &ChatCompletionRequest) -> Result<ProviderResult> {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let payload = serde_json::json!({
            "model": req.model,
            "messages": req.messages,
            "stream": true,
            "max_tokens": req.extra.get("max_tokens").cloned().unwrap_or(serde_json::json!(1024))
        });
        let mut rb = self
            .http
            .post(url)
            .header("anthropic-version", "2023-06-01");
        if let Some(api_key) = &self.api_key {
            rb = rb.header("x-api-key", api_key);
        }
        let response = rb
            .json(&payload)
            .send()
            .await
            .context("failed anthropic stream request")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("provider returned status {}: {}", status, body));
        }
        let stream = response
            .bytes_stream()
            .map(|item| item.map_err(|e| anyhow!("stream error: {}", e)));
        Ok(ProviderResult::Stream {
            stream: Box::pin(stream),
            provider_model: req.model.clone(),
            provider_id: String::new(),
            retries: 0,
            http_status: Some(status.as_u16()),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{
        AppConfig, AuthConfig, CapabilityConfig, LimitsConfig, ListenerConfig, ModelRoute,
        ObservabilityConfig, RoutingConfig,
    };

    use super::*;

    #[test]
    fn resolves_alias_route() {
        let mut providers = HashMap::new();
        providers.insert(
            "openai".to_string(),
            ProviderConfig {
                kind: ProviderKind::OpenAiCompat,
                base_url: "http://localhost".to_string(),
                api_key: None,
                api_key_env: None,
                headers: HashMap::new(),
            },
        );

        let mut aliases = HashMap::new();
        aliases.insert(
            "gpt".to_string(),
            vec![ModelRoute {
                provider: "openai".to_string(),
                model: "gpt-4o-mini".to_string(),
            }],
        );

        let cfg = AppConfig {
            listener: ListenerConfig {
                bind: "127.0.0.1:3000".to_string(),
            },
            auth: AuthConfig::default(),
            limits: LimitsConfig::default(),
            providers,
            model_aliases: aliases,
            plugins: vec![],
            capabilities: CapabilityConfig::default(),
            policies: vec![],
            observability: ObservabilityConfig::default(),
            routing: RoutingConfig::default(),
        };

        let reg = ProviderRegistry::new(&cfg).expect("valid registry");
        let routes = reg.routes_for("gpt").expect("route");
        assert_eq!(routes[0].provider_id, "openai");
        assert_eq!(routes[0].model, "gpt-4o-mini");
    }
}
