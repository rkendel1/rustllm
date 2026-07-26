pub mod auth;
pub mod config;
pub mod metrics;
pub mod models;
pub mod plugins;
pub mod providers;
pub mod rate_limit;

use std::{sync::Arc, time::Instant};

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::json;
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    auth::{extract_bearer, is_authorized},
    config::AppConfig,
    metrics::Metrics,
    models::{ChatCompletionRequest, openai_error},
    plugins::{Hook, PluginManager},
    providers::{ProviderRegistry, ProviderResult},
    rate_limit::RateLimiter,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub providers: Arc<ProviderRegistry>,
    pub plugins: Arc<PluginManager>,
    pub limiter: Arc<RateLimiter>,
    pub metrics: Arc<Metrics>,
}

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/health", get(health))
        .route("/metrics", get(metrics_handler))
        .route("/v1/chat/completions", post(chat_completions))
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.metrics.render() {
        Ok(body) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/plain; version=0.0.4")
            .body(Body::from(body))
            .expect("valid response"),
        Err(err) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("metrics encode failed: {}", err),
            "server_error",
        ),
    }
}

async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut request): Json<ChatCompletionRequest>,
) -> Response {
    let started = Instant::now();
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let auth_hook_input = json!({
        "headers": headers_to_map(&headers),
        "request_id": request_id,
    });
    if let Err(err) = state.plugins.execute(Hook::OnAuth, &auth_hook_input) {
        state.metrics.plugin_errors_total.inc();
        error!(error = %err, "auth plugin error");
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "plugin auth failure",
            "plugin_error",
        );
    }

    if !is_authorized(&headers, &state.config.auth) {
        state
            .metrics
            .requests_total
            .with_label_values(&["401"])
            .inc();
        return error_response(StatusCode::UNAUTHORIZED, "unauthorized", "invalid_api_key");
    }

    let caller_key = extract_bearer(&headers);
    if !state.limiter.check(caller_key.as_deref()) {
        state
            .metrics
            .requests_total
            .with_label_values(&["429"])
            .inc();
        return error_response(StatusCode::TOO_MANY_REQUESTS, "rate limited", "rate_limit");
    }

    let req_hook_input = serde_json::to_value(&request).unwrap_or_default();
    match state.plugins.execute(Hook::OnRequest, &req_hook_input) {
        Ok(result) if !result.allow => {
            state
                .metrics
                .requests_total
                .with_label_values(&["403"])
                .inc();
            return error_response(
                StatusCode::FORBIDDEN,
                result
                    .reject_reason
                    .as_deref()
                    .unwrap_or("request rejected by plugin"),
                "plugin_reject",
            );
        }
        Ok(result) => {
            if let Some(modified) = result.body {
                match serde_json::from_value::<ChatCompletionRequest>(modified) {
                    Ok(new_req) => request = new_req,
                    Err(err) => {
                        return error_response(
                            StatusCode::BAD_REQUEST,
                            &format!("plugin produced invalid request: {}", err),
                            "invalid_request_error",
                        );
                    }
                }
            }
        }
        Err(err) => {
            state.metrics.plugin_errors_total.inc();
            error!(error = %err, "on_request plugin error");
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "plugin request failure",
                "plugin_error",
            );
        }
    }

    let provider_result = match state.providers.execute(&request, &headers).await {
        Ok(result) => result,
        Err(err) => {
            state
                .metrics
                .provider_errors_total
                .with_label_values(&["all"])
                .inc();
            state
                .metrics
                .requests_total
                .with_label_values(&["502"])
                .inc();
            error!(error = %err, "provider request failed");
            return error_response(
                StatusCode::BAD_GATEWAY,
                "all configured providers failed",
                "provider_error",
            );
        }
    };

    state
        .metrics
        .observe_latency("chat_completions", started.elapsed());

    match provider_result {
        ProviderResult::Json(mut body, provider_model) => {
            match state.plugins.execute(Hook::OnResponse, &body) {
                Ok(result) if !result.allow => {
                    state
                        .metrics
                        .requests_total
                        .with_label_values(&["403"])
                        .inc();
                    return error_response(
                        StatusCode::FORBIDDEN,
                        result
                            .reject_reason
                            .as_deref()
                            .unwrap_or("response rejected by plugin"),
                        "plugin_reject",
                    );
                }
                Ok(result) => {
                    if let Some(modified) = result.body {
                        body = modified;
                    }
                }
                Err(err) => {
                    state.metrics.plugin_errors_total.inc();
                    error!(error = %err, "on_response plugin error");
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "plugin response failure",
                        "plugin_error",
                    );
                }
            }

            if let Some(tokens) = body
                .get("usage")
                .and_then(|u| u.get("total_tokens"))
                .and_then(|t| t.as_f64())
            {
                state
                    .metrics
                    .tokens_total
                    .with_label_values(&[&provider_model])
                    .inc_by(tokens);
                let cost = estimate_cost(&provider_model, tokens);
                state
                    .metrics
                    .cost_total_usd
                    .with_label_values(&[&provider_model])
                    .inc_by(cost);
            }

            state
                .metrics
                .requests_total
                .with_label_values(&["200"])
                .inc();

            let mut response = Json(body).into_response();
            response.headers_mut().insert(
                "x-request-id",
                HeaderValue::from_str(&request_id).unwrap_or(HeaderValue::from_static("invalid")),
            );
            response
        }
        ProviderResult::Stream(stream, _provider_model) => {
            state
                .metrics
                .requests_total
                .with_label_values(&["200"])
                .inc();

            let plugins = state.plugins.clone();
            let transformed = stream.map(move |chunk| {
                chunk.and_then(|bytes| {
                    let text = String::from_utf8_lossy(&bytes).to_string();
                    let payload = json!({"chunk": text});
                    let hooked = plugins.execute(Hook::OnStreamChunk, &payload)?;
                    if !hooked.allow {
                        return Ok(Bytes::from_static(b"event: error\ndata: [DONE]\n\n"));
                    }
                    let out = hooked
                        .body
                        .and_then(|b| {
                            b.get("chunk")
                                .and_then(|v| v.as_str())
                                .map(ToString::to_string)
                        })
                        .unwrap_or(text);
                    Ok(Bytes::from(out))
                })
            });

            let mut response = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/event-stream")
                .body(Body::from_stream(transformed))
                .expect("valid stream response");
            response.headers_mut().insert(
                "x-request-id",
                HeaderValue::from_str(&request_id).unwrap_or(HeaderValue::from_static("invalid")),
            );
            response
        }
    }
}

fn error_response(status: StatusCode, message: &str, kind: &str) -> Response {
    (status, Json(openai_error(message, kind))).into_response()
}

fn estimate_cost(model: &str, total_tokens: f64) -> f64 {
    let per_1k = if model.contains("sonnet") {
        0.003
    } else if model.contains("gpt-4") {
        0.01
    } else {
        0.001
    };

    (total_tokens / 1000.0) * per_1k
}

fn headers_to_map(headers: &HeaderMap) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in headers {
        if let Ok(value) = v.to_str() {
            map.insert(k.as_str().to_string(), json!(value));
        }
    }
    serde_json::Value::Object(map)
}

pub async fn run(config_path: &str) -> Result<()> {
    let config = Arc::new(AppConfig::from_path(config_path)?);

    let env_filter = config
        .observability
        .log_level
        .clone()
        .unwrap_or_else(|| "info".to_string());

    let _ = tracing_subscriber::fmt()
        .json()
        .with_env_filter(env_filter)
        .try_init();

    let providers = Arc::new(ProviderRegistry::new(&config)?);
    let plugins = Arc::new(PluginManager::from_config(&config.plugins)?);
    let limiter = Arc::new(RateLimiter::new(
        config.limits.global_per_minute,
        config.limits.per_key_per_minute,
    ));
    let metrics = Arc::new(Metrics::new().context("failed to initialize metrics")?);

    let app = build_app(AppState {
        config: config.clone(),
        providers,
        plugins,
        limiter,
        metrics,
    });

    let listener = tokio::net::TcpListener::bind(&config.listener.bind)
        .await
        .with_context(|| format!("failed to bind {}", config.listener.bind))?;

    info!(bind = %config.listener.bind, "aether listening");
    axum::serve(listener, app)
        .await
        .context("server terminated unexpectedly")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    use crate::{
        config::{
            AppConfig, AuthConfig, LimitsConfig, ListenerConfig, ObservabilityConfig,
            ProviderConfig, ProviderKind,
        },
        providers::ProviderRegistry,
    };

    use super::*;

    fn test_state() -> AppState {
        let mut providers = HashMap::new();
        providers.insert(
            "local".to_string(),
            ProviderConfig {
                kind: ProviderKind::OpenAiCompat,
                base_url: "http://127.0.0.1:18080".to_string(),
                api_key: None,
                api_key_env: None,
                headers: HashMap::new(),
            },
        );

        let config = Arc::new(AppConfig {
            listener: ListenerConfig {
                bind: "127.0.0.1:0".to_string(),
            },
            auth: AuthConfig {
                virtual_keys: vec!["dev-key".to_string()],
            },
            limits: LimitsConfig::default(),
            providers,
            model_aliases: HashMap::new(),
            plugins: vec![],
            observability: ObservabilityConfig::default(),
        });

        AppState {
            providers: Arc::new(ProviderRegistry::new(&config).expect("registry")),
            plugins: Arc::new(PluginManager::from_config(&[]).expect("plugins")),
            limiter: Arc::new(RateLimiter::new(100, 100)),
            metrics: Arc::new(Metrics::new().expect("metrics")),
            config,
        }
    }

    #[tokio::test]
    async fn health_endpoint_works() {
        let app = build_app(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_unauthorized_chat() {
        let app = build_app(test_state());
        let body = serde_json::to_vec(&json!({
            "model": "local:foo",
            "messages": [{"role":"user","content":"hi"}],
            "stream": false
        }))
        .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
