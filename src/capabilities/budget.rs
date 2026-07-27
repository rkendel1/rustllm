use crate::kernel::{
    capability::{Capability, CapabilityFuture, CapabilityResult},
    context::RequestContext,
};

#[derive(Clone)]
pub struct BudgetCapability {
    default_max_input_tokens: u64,
}

impl BudgetCapability {
    pub fn new(default_max_input_tokens: u64) -> Self {
        Self {
            default_max_input_tokens,
        }
    }
}

impl Capability for BudgetCapability {
    fn id(&self) -> &'static str {
        "budget_guard"
    }

    fn version(&self) -> &'static str {
        "v1"
    }

    fn on_request<'a>(&'a self, ctx: &'a mut RequestContext) -> CapabilityFuture<'a> {
        Box::pin(async move {
            ctx.budget.max_input_tokens = self.default_max_input_tokens;
            let estimated_input = ctx.model.messages.len() as u64 * 256;
            if estimated_input > ctx.budget.max_input_tokens {
                return Ok(CapabilityResult::Deny {
                    message: format!(
                        "request exceeds budget estimate ({} > {})",
                        estimated_input, ctx.budget.max_input_tokens
                    ),
                    kind: "budget_exceeded".to_string(),
                    status_code: 429,
                });
            }
            Ok(CapabilityResult::Continue)
        })
    }
}
