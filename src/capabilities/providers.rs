use crate::kernel::{
    capability::{Capability, CapabilityFuture, CapabilityResult, PlanningFuture},
    context::{CapabilityState, RequestContext},
    manifest::CapabilityManifest,
    planning::PlanningContext,
};
use crate::runtime::planner_result::{CapabilityPlan, ProviderSelectionCandidates};

#[derive(Clone, Default)]
pub struct ProviderRoutingCapability;

impl Capability for ProviderRoutingCapability {
    fn id(&self) -> &'static str {
        "provider_router"
    }

    fn version(&self) -> &'static str {
        "v1"
    }

    fn manifest(&self) -> CapabilityManifest {
        CapabilityManifest {
            id: self.id().to_string(),
            version: self.version().to_string(),
            provides: vec!["provider.selection".to_string()],
            requires: vec![
                "policy".to_string(),
                "semantic.intent".to_string(),
                "budget".to_string(),
            ],
            before: vec![],
            after: vec![],
            tags: vec!["provider".to_string()],
            permissions: vec!["provider.route".to_string()],
            cost: 1,
        }
    }

    fn on_request<'a>(
        &'a self,
        ctx: &'a mut RequestContext,
        state: &'a mut CapabilityState,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            let intent = state.facts.get_str("semantic.intent");
            let remaining_budget = state.facts.get_f64("budget.remaining_budget");
            if let Some((provider, _)) = ctx.model.model.split_once(':') {
                ctx.metadata.values.insert(
                    "provider_hint".to_string(),
                    serde_json::json!(provider.to_string()),
                );
            }
            if let Some(intent) = intent {
                ctx.metadata
                    .values
                    .insert("intent".to_string(), serde_json::json!(intent));
            }
            if let Some(remaining_budget) = remaining_budget {
                ctx.metadata.values.insert(
                    "remaining_budget".to_string(),
                    serde_json::json!(remaining_budget),
                );
            }
            Ok(CapabilityResult::Continue)
        })
    }

    fn plan<'a>(&'a self, ctx: &'a PlanningContext) -> PlanningFuture<'a> {
        Box::pin(async move {
            let (provider, model) = match ctx.request.model.split_once(':') {
                Some((provider, model)) => (provider.to_string(), format!("{provider}:{model}")),
                None => ("default".to_string(), ctx.request.model.clone()),
            };
            Ok(CapabilityPlan {
                capability_id: self.id().to_string(),
                providers: Some(ProviderSelectionCandidates {
                    providers: vec![provider],
                    selected_model: Some(model),
                }),
                confidence: Some(0.8),
                ..Default::default()
            })
        })
    }
}
