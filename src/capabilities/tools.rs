use crate::kernel::{
    capability::{Capability, CapabilityFuture, CapabilityResult, PlanningFuture},
    context::{CapabilityState, RequestContext},
    manifest::CapabilityManifest,
    planning::PlanningContext,
};
use crate::runtime::planner_result::{CapabilityPlan, RequiredTools};

#[derive(Clone, Default)]
pub struct ToolMcpCapability;

impl Capability for ToolMcpCapability {
    fn id(&self) -> &'static str {
        "tool_mcp"
    }

    fn version(&self) -> &'static str {
        "v1"
    }

    fn manifest(&self) -> CapabilityManifest {
        CapabilityManifest {
            id: self.id().to_string(),
            version: self.version().to_string(),
            provides: vec!["tools".to_string()],
            requires: vec!["semantic.intent".to_string()],
            before: vec![],
            after: vec![],
            tags: vec!["tools".to_string()],
            permissions: vec!["tools.use".to_string()],
            cost: 1,
        }
    }

    fn on_request<'a>(
        &'a self,
        ctx: &'a mut RequestContext,
        state: &'a mut CapabilityState,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            if let Some(tools) = &ctx.model.tools {
                ctx.metadata
                    .values
                    .insert("tools".to_string(), tools.clone());
                state
                    .facts
                    .publish("tools.available", serde_json::json!(tools.clone()));
            }
            Ok(CapabilityResult::Continue)
        })
    }

    fn plan<'a>(&'a self, ctx: &'a PlanningContext) -> PlanningFuture<'a> {
        Box::pin(async move {
            let tools = ctx
                .request
                .tools
                .as_ref()
                .and_then(|value| value.as_array().cloned())
                .map(|entries| {
                    entries
                        .into_iter()
                        .filter_map(|entry| {
                            entry
                                .get("function")
                                .and_then(|func| func.get("name"))
                                .and_then(|name| name.as_str())
                                .map(ToString::to_string)
                        })
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();
            Ok(CapabilityPlan {
                capability_id: self.id().to_string(),
                required_tools: Some(RequiredTools { tools }),
                confidence: Some(0.75),
                ..Default::default()
            })
        })
    }
}
