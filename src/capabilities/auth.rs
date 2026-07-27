use crate::{
    auth::{extract_bearer, is_authorized},
    config::AuthConfig,
    kernel::{
        capability::{Capability, CapabilityFuture, CapabilityResult},
        context::{CapabilityState, RequestContext},
        manifest::CapabilityManifest,
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

    fn manifest(&self) -> CapabilityManifest {
        CapabilityManifest {
            id: self.id().to_string(),
            version: self.version().to_string(),
            provides: vec!["identity".to_string()],
            requires: vec![],
            before: vec![],
            after: vec![],
            tags: vec!["auth".to_string()],
            permissions: vec!["identity.read".to_string()],
            cost: 1,
        }
    }

    fn on_request<'a>(
        &'a self,
        ctx: &'a mut RequestContext,
        state: &'a mut CapabilityState,
    ) -> CapabilityFuture<'a> {
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
                return Ok(CapabilityResult::Fail {
                    message: "unauthorized".to_string(),
                    kind: "invalid_api_key".to_string(),
                    status_code: 401,
                });
            }
            state.facts.publish(
                "identity.user",
                ctx.identity.api_key.clone().unwrap_or_default(),
            );
            state
                .facts
                .publish("identity.plan", ctx.identity.plan.clone());
            Ok(CapabilityResult::Continue)
        })
    }
}
