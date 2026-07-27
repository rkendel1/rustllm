use crate::kernel::{
    capability::{Capability, CapabilityFuture, CapabilityResult},
    context::{CapabilityState, RequestContext},
    manifest::CapabilityManifest,
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

    fn manifest(&self) -> CapabilityManifest {
        CapabilityManifest {
            id: self.id().to_string(),
            version: self.version().to_string(),
            provides: vec!["guardrails".to_string()],
            requires: vec!["semantic.intent".to_string()],
            before: vec![],
            after: vec![],
            tags: vec!["guardrails".to_string()],
            permissions: vec!["guardrails.scan".to_string()],
            cost: 1,
        }
    }

    fn on_request<'a>(
        &'a self,
        ctx: &'a mut RequestContext,
        _state: &'a mut CapabilityState,
    ) -> CapabilityFuture<'a> {
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
                    return Ok(CapabilityResult::Stop {
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
