use anyhow::{Result, anyhow};

use crate::runtime::execution_plan::ExecutionPlan;

use super::{
    capability::CapabilityResult,
    context::{CapabilityState, RequestContext, ResponseContext},
    lifecycle::{LifecycleEvent, LifecycleHook},
    registry::CapabilityRegistry,
};

pub struct CapabilityScheduler;

impl CapabilityScheduler {
    pub async fn execute_request(
        registry: &CapabilityRegistry,
        plan: &ExecutionPlan,
        ctx: &mut RequestContext,
        state: &mut CapabilityState,
    ) -> Result<(CapabilityResult, Vec<LifecycleEvent>)> {
        let mut events = Vec::new();
        for group in &plan.parallel_groups {
            for capability_id in group {
                let capability = registry.capability(capability_id).ok_or_else(|| {
                    anyhow!("planned capability '{}' is not registered", capability_id)
                })?;
                events.push(LifecycleEvent {
                    capability_id: capability_id.clone(),
                    hook: LifecycleHook::OnRequest,
                });
                let result = capability.on_request(ctx, state).await?;
                if result.should_stop() {
                    return Ok((result, events));
                }
            }
        }
        Ok((CapabilityResult::Continue, events))
    }

    pub async fn execute_response(
        registry: &CapabilityRegistry,
        plan: &ExecutionPlan,
        ctx: &mut ResponseContext,
        state: &mut CapabilityState,
    ) -> Result<(CapabilityResult, Vec<LifecycleEvent>)> {
        let mut events = Vec::new();
        for group in plan.parallel_groups.iter().rev() {
            for capability_id in group.iter().rev() {
                let capability = registry.capability(capability_id).ok_or_else(|| {
                    anyhow!("planned capability '{}' is not registered", capability_id)
                })?;
                events.push(LifecycleEvent {
                    capability_id: capability_id.clone(),
                    hook: LifecycleHook::OnResponse,
                });
                let result = capability.on_response(ctx, state).await?;
                if result.should_stop() {
                    return Ok((result, events));
                }
            }
        }
        Ok((CapabilityResult::Continue, events))
    }
}
