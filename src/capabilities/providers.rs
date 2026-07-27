use std::collections::HashMap;

use crate::config::ModelRoute;
use crate::kernel::{
    capability::{Capability, CapabilityFuture, CapabilityResult, PlanningFuture},
    context::{CapabilityState, RequestContext},
    manifest::CapabilityManifest,
    planning::PlanningContext,
    scoring::RoutingStrategy,
};
use crate::runtime::planner_result::{CapabilityPlan, ProviderSelectionCandidates};

#[derive(Clone)]
pub struct ProviderRoutingCapability {
    aliases: HashMap<String, Vec<ModelRoute>>,
    strategy: RoutingStrategy,
}

impl ProviderRoutingCapability {
    pub fn new(aliases: HashMap<String, Vec<ModelRoute>>, strategy: RoutingStrategy) -> Self {
        Self { aliases, strategy }
    }
}

impl Default for ProviderRoutingCapability {
    fn default() -> Self {
        Self::new(HashMap::new(), RoutingStrategy::Adaptive)
    }
}

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
            let mut providers = Vec::new();
            let mut selected_model = None;
            let mut provider_models = HashMap::new();

            if let Some(routes) = self.aliases.get(&ctx.request.model) {
                for route in routes {
                    providers.push(route.provider.clone());
                    provider_models.insert(route.provider.clone(), route.model.clone());
                }
            } else if let Some((provider, model)) = ctx.request.model.split_once(':') {
                providers.push(provider.to_string());
                let mapped = model.to_string();
                provider_models.insert(provider.to_string(), mapped.clone());
                selected_model = Some(format!("{provider}:{mapped}"));
            } else {
                providers.push("default".to_string());
                selected_model = Some(ctx.request.model.clone());
            }

            Ok(CapabilityPlan {
                capability_id: self.id().to_string(),
                providers: Some(ProviderSelectionCandidates {
                    providers,
                    selected_model,
                    strategy: Some(self.strategy.as_str().to_string()),
                    provider_models,
                }),
                confidence: Some(0.8),
                ..Default::default()
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        config::ModelRoute,
        kernel::{context::Identity, planning::PlanningContext, scoring::RoutingStrategy},
        models::{ChatCompletionRequest, ChatMessage},
    };

    use super::*;

    #[tokio::test]
    async fn plan_uses_alias_routes_with_strategy() {
        let mut aliases = HashMap::new();
        aliases.insert(
            "gpt".to_string(),
            vec![
                ModelRoute {
                    provider: "openai".to_string(),
                    model: "gpt-4o-mini".to_string(),
                },
                ModelRoute {
                    provider: "local".to_string(),
                    model: "llama3".to_string(),
                },
            ],
        );
        let capability = ProviderRoutingCapability::new(aliases, RoutingStrategy::Adaptive);
        let ctx = PlanningContext {
            request_id: "r1".to_string(),
            request: ChatCompletionRequest {
                model: "gpt".to_string(),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: serde_json::json!("hi"),
                }],
                tools: None,
                stream: false,
                extra: HashMap::new(),
            },
            identity: Identity::default(),
            metadata: Default::default(),
            budget: Default::default(),
            policy: Default::default(),
            headers: HashMap::new(),
        };

        let plan = capability.plan(&ctx).await.expect("plan");
        let providers = plan.providers.expect("providers");
        assert_eq!(providers.providers, vec!["openai", "local"]);
        assert_eq!(providers.strategy.as_deref(), Some("adaptive"));
        assert_eq!(
            providers.provider_models.get("openai"),
            Some(&"gpt-4o-mini".to_string())
        );
    }
}
