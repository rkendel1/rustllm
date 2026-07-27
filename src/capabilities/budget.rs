use crate::kernel::{
    capability::{Capability, CapabilityFuture, CapabilityResult, PlanningFuture},
    context::{CapabilityState, RequestContext},
    manifest::CapabilityManifest,
    planning::PlanningContext,
};
use crate::runtime::planner_result::{BudgetEstimate, CapabilityPlan};

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

    fn manifest(&self) -> CapabilityManifest {
        CapabilityManifest {
            id: self.id().to_string(),
            version: self.version().to_string(),
            provides: vec!["budget".to_string()],
            requires: vec!["identity".to_string()],
            before: vec![],
            after: vec![],
            tags: vec!["budget".to_string()],
            permissions: vec!["budget.manage".to_string()],
            cost: 1,
        }
    }

    fn on_request<'a>(
        &'a self,
        ctx: &'a mut RequestContext,
        state: &'a mut CapabilityState,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            ctx.budget.max_input_tokens = self.default_max_input_tokens;
            let estimated_input = ctx.model.messages.len() as u64 * 256;
            state.budget.tokens_used = estimated_input;
            state.budget.remaining_budget = self
                .default_max_input_tokens
                .saturating_sub(estimated_input);
            state.facts.publish(
                "budget.remaining_budget",
                serde_json::json!(state.budget.remaining_budget),
            );
            state.facts.publish(
                "budget.tokens_used",
                serde_json::json!(state.budget.tokens_used),
            );
            if estimated_input > ctx.budget.max_input_tokens {
                return Ok(CapabilityResult::Fail {
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

    fn plan<'a>(&'a self, ctx: &'a PlanningContext) -> PlanningFuture<'a> {
        Box::pin(async move {
            let estimated_tokens = ctx.request.messages.len() as u64 * 256;
            let estimated_cost_usd = (estimated_tokens as f64 / 1000.0) * 0.001;
            let remaining_budget = self
                .default_max_input_tokens
                .saturating_sub(estimated_tokens);
            Ok(CapabilityPlan {
                capability_id: self.id().to_string(),
                budget: Some(BudgetEstimate {
                    estimated_tokens,
                    estimated_cost_usd,
                    remaining_budget,
                }),
                confidence: Some(0.75),
                ..Default::default()
            })
        })
    }
}
