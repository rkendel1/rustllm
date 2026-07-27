use crate::{
    auth::{extract_bearer, is_authorized},
    config::AuthConfig,
    kernel::{
        capability::{Capability, CapabilityFuture, CapabilityResult},
        context::RequestContext,
    },
};

#[derive(Clone)]
pub struct IdentityCapability {
    auth: AuthConfig,
}

impl IdentityCapability {
    pub fn new(auth: AuthConfig) -> Self {
        Self { auth }
    }
}

impl Capability for IdentityCapability {
    fn id(&self) -> &'static str {
        "identity"
    }

    fn version(&self) -> &'static str {
        "v1"
    }

    fn on_request<'a>(&'a self, ctx: &'a mut RequestContext) -> CapabilityFuture<'a> {
        Box::pin(async move {
            let mut headers = axum::http::HeaderMap::new();
            for (k, v) in &ctx.headers {
                if let Ok(name) = axum::http::header::HeaderName::from_bytes(k.as_bytes())
                    && let Ok(value) = axum::http::HeaderValue::from_str(v)
                {
                    headers.insert(name, value);
                }
            }
            ctx.identity.api_key = extract_bearer(&headers);
            ctx.identity.authenticated = is_authorized(&headers, &self.auth);
            if !ctx.identity.authenticated {
                return Ok(CapabilityResult::Deny {
                    message: "unauthorized".to_string(),
                    kind: "invalid_api_key".to_string(),
                    status_code: 401,
                });
            }
            Ok(CapabilityResult::Continue)
        })
    }
}
