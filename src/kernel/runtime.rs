use anyhow::{Result, anyhow};

use crate::runtime::execution_plan::ExecutionPlan;

use super::{
    capability::{Capability, CapabilityResult},
    context::{CapabilityState, RequestContext, ResponseContext},
    lifecycle::LifecycleEvent,
    planner::ExecutionPlanner,
    registry::CapabilityRegistry,
    scheduler::CapabilityScheduler,
};

pub struct CapabilityRuntime {
    registry: CapabilityRegistry,
    plan: ExecutionPlan,
}

impl CapabilityRuntime {
    pub fn new(capabilities: Vec<Box<dyn Capability>>, pipeline: &[String]) -> Result<Self> {
        let mut registry = CapabilityRegistry::new();
        registry.register_many(capabilities)?;

        let manifests = registry.manifests_for_pipeline(pipeline)?;
        let plan = ExecutionPlanner::build_plan(manifests)?;
        if !plan.missing_dependencies.is_empty() {
            return Err(anyhow!(
                "missing capability dependencies: {}",
                plan.missing_dependencies.join(", ")
            ));
        }

        Ok(Self { registry, plan })
    }

    pub fn describe(&self) -> Vec<(&str, &str)> {
        self.plan
            .execution_order
            .iter()
            .filter_map(|id| {
                self.registry
                    .manifest(id)
                    .map(|manifest| (manifest.id.as_str(), manifest.version.as_str()))
            })
            .collect()
    }

    pub fn diagnostics(&self) -> &ExecutionPlan {
        &self.plan
    }

    pub async fn on_request(
        &self,
        ctx: &mut RequestContext,
    ) -> Result<(CapabilityResult, Vec<LifecycleEvent>)> {
        let mut state = CapabilityState::from_request(ctx);
        let result =
            CapabilityScheduler::execute_request(&self.registry, &self.plan, ctx, &mut state).await;
        state.apply_to_request(ctx);
        result
    }

    pub async fn on_response(
        &self,
        ctx: &mut ResponseContext,
    ) -> Result<(CapabilityResult, Vec<LifecycleEvent>)> {
        let mut state = CapabilityState::from_response(ctx);
        let result =
            CapabilityScheduler::execute_response(&self.registry, &self.plan, ctx, &mut state)
                .await;
        state.apply_to_response(ctx);
        result
    }

    pub fn ensure_contains(&self, id: &str) -> Result<()> {
        if self.plan.execution_order.iter().any(|cap_id| cap_id == id) {
            return Ok(());
        }
        Err(anyhow!("required capability '{}' not configured", id))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        kernel::{
            capability::{Capability, CapabilityFuture, CapabilityResult},
            context::{CapabilityState, Identity, Metadata, RequestContext},
            manifest::CapabilityManifest,
        },
        models::{ChatCompletionRequest, ChatMessage},
    };

    use super::*;

    #[derive(Clone)]
    struct DenyCapability;

    impl Capability for DenyCapability {
        fn id(&self) -> &'static str {
            "deny"
        }

        fn version(&self) -> &'static str {
            "v1"
        }

        fn manifest(&self) -> CapabilityManifest {
            CapabilityManifest {
                id: self.id().to_string(),
                version: self.version().to_string(),
                provides: vec!["deny".to_string()],
                requires: vec![],
                before: vec![],
                after: vec![],
                tags: vec![],
                permissions: vec![],
                cost: 1,
            }
        }

        fn on_request<'a>(
            &'a self,
            _ctx: &'a mut RequestContext,
            _state: &'a mut CapabilityState,
        ) -> CapabilityFuture<'a> {
            Box::pin(async {
                Ok(CapabilityResult::Fail {
                    message: "blocked".to_string(),
                    kind: "deny".to_string(),
                    status_code: 403,
                })
            })
        }
    }

    #[tokio::test]
    async fn stops_pipeline_on_deny() {
        let runtime = CapabilityRuntime::new(vec![Box::new(DenyCapability)], &[]).expect("runtime");
        let mut ctx = RequestContext {
            request_id: "r1".to_string(),
            identity: Identity::default(),
            model: ChatCompletionRequest {
                model: "local:foo".to_string(),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: serde_json::json!("hi"),
                }],
                tools: None,
                stream: false,
                extra: Default::default(),
            },
            metadata: Metadata::default(),
            budget: Default::default(),
            policy: Default::default(),
            headers: Default::default(),
        };

        let (result, events) = runtime.on_request(&mut ctx).await.expect("runtime");
        assert!(matches!(result, CapabilityResult::Fail { .. }));
        assert_eq!(events.len(), 1);
    }
}
