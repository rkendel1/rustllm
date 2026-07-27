use crate::kernel::{
    capability::{Capability, CapabilityFuture, CapabilityResult},
    context::{CapabilityState, RequestContext},
    manifest::CapabilityManifest,
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

    fn manifest(&self) -> CapabilityManifest {
        CapabilityManifest {
            id: self.id().to_string(),
            version: self.version().to_string(),
            provides: vec!["semantic.intent".to_string()],
            requires: vec!["policy".to_string()],
            before: vec![],
            after: vec![],
            tags: vec!["routing".to_string()],
            permissions: vec!["metadata.write".to_string()],
            cost: 1,
        }
    }

    fn on_request<'a>(
        &'a self,
        ctx: &'a mut RequestContext,
        state: &'a mut CapabilityState,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            if ctx.metadata.intent.is_none() {
                let intent = if ctx.model.tools.is_some() {
                    "tools"
                } else {
                    "chat"
                };
                ctx.metadata.intent = Some(intent.to_string());
            }
            let confidence = if ctx.model.tools.is_some() {
                0.94
            } else {
                0.82
            };
            ctx.metadata.confidence = Some(confidence);
            state.semantic.intent = ctx.metadata.intent.clone();
            state.semantic.confidence = ctx.metadata.confidence;
            if let Some(intent) = &state.semantic.intent {
                state.facts.publish("semantic.intent", intent.clone());
            }
            state.facts.publish("semantic.confidence", confidence);
            Ok(CapabilityResult::Modify)
        })
    }
}
