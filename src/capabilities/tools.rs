use crate::kernel::{
    capability::{Capability, CapabilityFuture, CapabilityResult},
    context::RequestContext,
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

    fn on_request<'a>(&'a self, ctx: &'a mut RequestContext) -> CapabilityFuture<'a> {
        Box::pin(async move {
            if let Some(tools) = &ctx.model.tools {
                ctx.metadata
                    .values
                    .insert("tools".to_string(), tools.clone());
            }
            Ok(CapabilityResult::Continue)
        })
    }
}
