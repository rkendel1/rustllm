use std::{future::Future, pin::Pin};

use anyhow::Result;

use super::{
    context::{CapabilityState, RequestContext, ResponseContext},
    manifest::CapabilityManifest,
};

pub type CapabilityFuture<'a> = Pin<Box<dyn Future<Output = Result<CapabilityResult>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityResult {
    Continue,
    Modify,
    Stop {
        message: String,
        kind: String,
        status_code: u16,
    },
    Retry {
        reason: String,
    },
    Suspend {
        reason: String,
    },
    RequireApproval {
        message: String,
        kind: String,
        status_code: u16,
    },
    Redirect {
        target: String,
    },
    Fail {
        message: String,
        kind: String,
        status_code: u16,
    },
}

impl CapabilityResult {
    pub fn should_stop(&self) -> bool {
        matches!(
            self,
            CapabilityResult::Stop { .. }
                | CapabilityResult::Retry { .. }
                | CapabilityResult::Suspend { .. }
                | CapabilityResult::RequireApproval { .. }
                | CapabilityResult::Redirect { .. }
                | CapabilityResult::Fail { .. }
        )
    }
}

pub trait Capability: Send + Sync {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn manifest(&self) -> CapabilityManifest;

    fn on_request<'a>(
        &'a self,
        _ctx: &'a mut RequestContext,
        _state: &'a mut CapabilityState,
    ) -> CapabilityFuture<'a> {
        Box::pin(async { Ok(CapabilityResult::Continue) })
    }

    fn on_response<'a>(
        &'a self,
        _ctx: &'a mut ResponseContext,
        _state: &'a mut CapabilityState,
    ) -> CapabilityFuture<'a> {
        Box::pin(async { Ok(CapabilityResult::Continue) })
    }
}
