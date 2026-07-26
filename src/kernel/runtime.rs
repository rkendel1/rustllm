use anyhow::{Result, anyhow};

use super::{
    capability::{Capability, CapabilityResult},
    context::{RequestContext, ResponseContext},
    lifecycle::{LifecycleEvent, LifecycleHook},
};

pub struct CapabilityRuntime {
    pipeline: Vec<Box<dyn Capability>>,
}

impl CapabilityRuntime {
    pub fn new(mut capabilities: Vec<Box<dyn Capability>>, pipeline: &[String]) -> Self {
        if !pipeline.is_empty() {
            capabilities.sort_by_key(|cap| {
                pipeline
                    .iter()
                    .position(|id| id == cap.id())
                    .unwrap_or(usize::MAX)
            });
        }
        Self {
            pipeline: capabilities,
        }
    }

    pub fn describe(&self) -> Vec<(&'static str, &'static str)> {
        self.pipeline
            .iter()
            .map(|cap| (cap.id(), cap.version()))
            .collect()
    }

    pub async fn on_request(
        &self,
        ctx: &mut RequestContext,
    ) -> Result<(CapabilityResult, Vec<LifecycleEvent>)> {
        let mut events = Vec::with_capacity(self.pipeline.len());
        for capability in &self.pipeline {
            events.push(LifecycleEvent {
                capability_id: capability.id().to_string(),
                hook: LifecycleHook::OnRequest,
            });
            match capability.on_request(ctx).await? {
                CapabilityResult::Continue => {}
                denied => return Ok((denied, events)),
            }
        }
        Ok((CapabilityResult::Continue, events))
    }

    pub async fn on_response(
        &self,
        ctx: &mut ResponseContext,
    ) -> Result<(CapabilityResult, Vec<LifecycleEvent>)> {
        let mut events = Vec::with_capacity(self.pipeline.len());
        for capability in self.pipeline.iter().rev() {
            events.push(LifecycleEvent {
                capability_id: capability.id().to_string(),
                hook: LifecycleHook::OnResponse,
            });
            match capability.on_response(ctx).await? {
                CapabilityResult::Continue => {}
                denied => return Ok((denied, events)),
            }
        }
        Ok((CapabilityResult::Continue, events))
    }

    pub fn ensure_contains(&self, id: &str) -> Result<()> {
        if self.pipeline.iter().any(|cap| cap.id() == id) {
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
            context::{Identity, Metadata, RequestContext},
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

        fn on_request<'a>(&'a self, _ctx: &'a mut RequestContext) -> CapabilityFuture<'a> {
            Box::pin(async {
                Ok(CapabilityResult::Deny {
                    message: "blocked".to_string(),
                    kind: "deny".to_string(),
                    status_code: 403,
                })
            })
        }
    }

    #[tokio::test]
    async fn stops_pipeline_on_deny() {
        let runtime = CapabilityRuntime::new(vec![Box::new(DenyCapability)], &[]);
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
        assert!(matches!(result, CapabilityResult::Deny { .. }));
        assert_eq!(events.len(), 1);
    }
}
