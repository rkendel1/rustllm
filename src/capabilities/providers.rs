use crate::kernel::{
    capability::{Capability, CapabilityFuture, CapabilityResult},
    context::RequestContext,
};

#[derive(Clone, Default)]
pub struct ProviderRoutingCapability;

impl Capability for ProviderRoutingCapability {
    fn id(&self) -> &'static str {
        "provider_router"
    }

    fn version(&self) -> &'static str {
        "v1"
    }

    fn on_request<'a>(&'a self, ctx: &'a mut RequestContext) -> CapabilityFuture<'a> {
        Box::pin(async move {
            if let Some((provider, _)) = ctx.model.model.split_once(':') {
                ctx.metadata.values.insert(
                    "provider_hint".to_string(),
                    serde_json::json!(provider.to_string()),
                );
            }
            Ok(CapabilityResult::Continue)
        })
    }
}
