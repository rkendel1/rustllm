use crate::kernel::{
    capability::{Capability, CapabilityFuture, CapabilityResult},
    context::RequestContext,
};

#[derive(Clone)]
pub struct GuardrailCapability {
    blocked_terms: Vec<String>,
}

impl GuardrailCapability {
    pub fn new(blocked_terms: Vec<String>) -> Self {
        Self { blocked_terms }
    }
}

impl Capability for GuardrailCapability {
    fn id(&self) -> &'static str {
        "pii_filter"
    }

    fn version(&self) -> &'static str {
        "v1"
    }

    fn on_request<'a>(&'a self, ctx: &'a mut RequestContext) -> CapabilityFuture<'a> {
        Box::pin(async move {
            if self.blocked_terms.is_empty() {
                return Ok(CapabilityResult::Continue);
            }
            for message in &ctx.model.messages {
                let raw = message.content.to_string().to_lowercase();
                if let Some(term) = self
                    .blocked_terms
                    .iter()
                    .find(|term| raw.contains(&term.to_lowercase()))
                {
                    return Ok(CapabilityResult::Deny {
                        message: format!("request contains blocked term '{}'", term),
                        kind: "guardrail_reject".to_string(),
                        status_code: 403,
                    });
                }
            }
            Ok(CapabilityResult::Continue)
        })
    }
}
