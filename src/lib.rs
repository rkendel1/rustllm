pub mod auth;
pub mod capabilities;
pub mod config;
pub mod kernel;
pub mod metrics;
pub mod models;
pub mod plugins;
pub mod providers;
pub mod rate_limit;
pub mod runtime;
pub mod wasm;

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
use serde_json::json;
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    capabilities::{
        auth::IdentityCapability, budget::BudgetCapability, guardrails::GuardrailCapability,
        policy::PolicyCapability, providers::ProviderRoutingCapability, routing::RoutingCapability,
        tools::ToolMcpCapability,
    },
    config::AppConfig,
    kernel::{
        capability::CapabilityResult,
        context::{Identity, Metadata, RequestContext, ResponseContext},
        runtime::CapabilityRuntime,
    },
    metrics::Metrics,
    models::{ChatCompletionRequest, openai_error},
    providers::{ProviderRegistry, ProviderResult},
    rate_limit::RateLimiter,
    wasm::loader::WasmCapability,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub providers: Arc<ProviderRegistry>,
    pub runtime: Arc<CapabilityRuntime>,
    pub limiter: Arc<RateLimiter>,
    pub metrics: Arc<Metrics>,
}

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/health", get(health))
        .route("/debug/plan", get(debug_plan))
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

async fn debug_plan(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.runtime.diagnostics().clone())
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

    let mut request_ctx = RequestContext {
        request_id: request_id.clone(),
        identity: Identity {
            api_key: None,
            authenticated: false,
            plan: "free".to_string(),
        },
        model: request.clone(),
        metadata: Metadata::default(),
        budget: Default::default(),
        policy: Default::default(),
        headers: headers_to_map(&headers),
    };

    match state.runtime.on_request(&mut request_ctx).await {
        Ok((CapabilityResult::Continue | CapabilityResult::Modify, _events)) => {
            request = request_ctx.model.clone();
        }
        Ok((result, _events)) => {
            let (status_code, message, kind) = capability_failure_details(result);
            state
                .metrics
                .requests_total
                .with_label_values(&[&status_code.to_string()])
                .inc();
            return error_response(
                StatusCode::from_u16(status_code).unwrap_or(StatusCode::FORBIDDEN),
                &message,
                &kind,
            );
        }
        Err(err) => {
            state.metrics.plugin_errors_total.inc();
            error!(error = %err, "capability runtime request failure");
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "capability runtime request failure",
                "capability_error",
            );
        }
    }

    if !state.limiter.check(request_ctx.identity.api_key.as_deref()) {
        state
            .metrics
            .requests_total
            .with_label_values(&["429"])
            .inc();
        return error_response(StatusCode::TOO_MANY_REQUESTS, "rate limited", "rate_limit");
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
            let mut response_ctx = ResponseContext {
                request_id: request_id.clone(),
                identity: request_ctx.identity.clone(),
                metadata: request_ctx.metadata.clone(),
                policy: request_ctx.policy.clone(),
                provider_model: Some(provider_model.clone()),
                body: body.clone(),
            };
            match state.runtime.on_response(&mut response_ctx).await {
                Ok((CapabilityResult::Continue | CapabilityResult::Modify, _events)) => {
                    body = response_ctx.body;
                }
                Ok((result, _events)) => {
                    let (status_code, message, kind) = capability_failure_details(result);
                    state
                        .metrics
                        .requests_total
                        .with_label_values(&[&status_code.to_string()])
                        .inc();
                    return error_response(
                        StatusCode::from_u16(status_code).unwrap_or(StatusCode::FORBIDDEN),
                        &message,
                        &kind,
                    );
                }
                Err(err) => {
                    state.metrics.plugin_errors_total.inc();
                    error!(error = %err, "capability runtime response failure");
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "capability runtime response failure",
                        "capability_error",
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

            let mut response = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/event-stream")
                .body(Body::from_stream(stream))
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

fn capability_failure_details(result: CapabilityResult) -> (u16, String, String) {
    match result {
        CapabilityResult::Stop {
            message,
            kind,
            status_code,
        }
        | CapabilityResult::RequireApproval {
            message,
            kind,
            status_code,
        }
        | CapabilityResult::Fail {
            message,
            kind,
            status_code,
        } => (status_code, message, kind),
        CapabilityResult::Retry { reason } => (503, reason, "capability_retry".to_string()),
        CapabilityResult::Suspend { reason } => (503, reason, "capability_suspend".to_string()),
        CapabilityResult::Redirect { target } => (307, target, "capability_redirect".to_string()),
        CapabilityResult::Continue | CapabilityResult::Modify => (
            500,
            "unexpected capability state".to_string(),
            "capability_error".to_string(),
        ),
    }
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

fn headers_to_map(headers: &HeaderMap) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for (k, v) in headers {
        if let Ok(value) = v.to_str() {
            map.insert(k.as_str().to_string(), value.to_string());
        }
    }
    map
}

fn build_runtime(config: &AppConfig) -> Result<CapabilityRuntime> {
    let wasm_plugins = plugins::PluginManager::from_config(&config.plugins)?;
    let capabilities: Vec<Box<dyn kernel::capability::Capability>> = vec![
        Box::new(IdentityCapability::new(config.auth.clone())),
        Box::new(PolicyCapability::new(config.policies.clone())),
        Box::new(BudgetCapability::new(8_192)),
        Box::new(GuardrailCapability::new(Vec::new())),
        Box::new(RoutingCapability),
        Box::new(ToolMcpCapability),
        Box::new(ProviderRoutingCapability),
        Box::new(WasmCapability::new(wasm_plugins)),
    ];
    let runtime = CapabilityRuntime::new(capabilities, &config.capabilities.pipeline)?;
    runtime.ensure_contains("identity")?;
    runtime.ensure_contains("provider_router")?;
    Ok(runtime)
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
    let runtime = Arc::new(build_runtime(&config)?);
    let limiter = Arc::new(RateLimiter::new(
        config.limits.global_per_minute,
        config.limits.per_key_per_minute,
    ));
    let metrics = Arc::new(Metrics::new().context("failed to initialize metrics")?);

    let app = build_app(AppState {
        config: config.clone(),
        providers,
        runtime,
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
            AppConfig, AuthConfig, CapabilityConfig, LimitsConfig, ListenerConfig,
            ObservabilityConfig, ProviderConfig, ProviderKind,
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
            capabilities: CapabilityConfig::default(),
            policies: vec![],
            observability: ObservabilityConfig::default(),
        });

        AppState {
            providers: Arc::new(ProviderRegistry::new(&config).expect("registry")),
            runtime: Arc::new(build_runtime(&config).expect("runtime")),
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
    async fn debug_plan_endpoint_works() {
        let app = build_app(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/debug/plan")
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
