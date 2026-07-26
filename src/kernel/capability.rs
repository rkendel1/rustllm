use std::{future::Future, pin::Pin};

use anyhow::Result;

use super::context::{RequestContext, ResponseContext};

pub type CapabilityFuture<'a> = Pin<Box<dyn Future<Output = Result<CapabilityResult>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityResult {
    Continue,
    Deny {
        message: String,
        kind: String,
        status_code: u16,
    },
}

pub trait Capability: Send + Sync {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;

    fn on_request<'a>(&'a self, _ctx: &'a mut RequestContext) -> CapabilityFuture<'a> {
        Box::pin(async { Ok(CapabilityResult::Continue) })
    }

    fn on_response<'a>(&'a self, _ctx: &'a mut ResponseContext) -> CapabilityFuture<'a> {
        Box::pin(async { Ok(CapabilityResult::Continue) })
    }
}
