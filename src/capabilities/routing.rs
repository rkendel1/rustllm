use crate::kernel::{
    capability::{Capability, CapabilityFuture, CapabilityResult},
    context::RequestContext,
};

#[derive(Clone, Default)]
pub struct RoutingCapability;

impl Capability for RoutingCapability {
    fn id(&self) -> &'static str {
        "semantic_router"
    }

    fn version(&self) -> &'static str {
        "v1"
    }

    fn on_request<'a>(&'a self, ctx: &'a mut RequestContext) -> CapabilityFuture<'a> {
        Box::pin(async move {
            if ctx.metadata.intent.is_none() {
                let intent = if ctx.model.tools.is_some() {
                    "tools"
                } else {
                    "chat"
                };
                ctx.metadata.intent = Some(intent.to_string());
            }
            Ok(CapabilityResult::Continue)
        })
    }
}
