use crate::kernel::{
    capability::{Capability, CapabilityFuture, CapabilityResult},
    context::{CapabilityState, RequestContext},
    manifest::CapabilityManifest,
};

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
}
