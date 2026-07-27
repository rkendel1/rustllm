use std::time::Instant;

use anyhow::{Result, anyhow};

use crate::runtime::{decision::CapabilityDecisionLog, planner_result::PlannerResult};

use super::{
    context::{BudgetContext, Identity, Metadata, PolicyContext, RequestContext},
    registry::CapabilityRegistry,
};

#[derive(Debug, Clone)]
pub struct PlanningContext {
    pub request_id: String,
    pub request: crate::models::ChatCompletionRequest,
    pub identity: Identity,
    pub metadata: Metadata,
    pub budget: BudgetContext,
    pub policy: PolicyContext,
    pub headers: std::collections::HashMap<String, String>,
}

impl From<&RequestContext> for PlanningContext {
    fn from(ctx: &RequestContext) -> Self {
        Self {
            request_id: ctx.request_id.clone(),
            request: ctx.model.clone(),
            identity: ctx.identity.clone(),
            metadata: ctx.metadata.clone(),
            budget: ctx.budget.clone(),
            policy: ctx.policy.clone(),
            headers: ctx.headers.clone(),
        }
    }
}

pub struct PlanningEngine;

impl PlanningEngine {
    pub async fn collect(
        registry: &CapabilityRegistry,
        execution_order: &[String],
        ctx: &PlanningContext,
    ) -> Result<PlannerResult> {
        let mut result = PlannerResult::default();

        for capability_id in execution_order {
            let capability = registry.capability(capability_id).ok_or_else(|| {
                anyhow!("planned capability '{}' is not registered", capability_id)
            })?;

            let started = Instant::now();
            let mut plan = capability.plan(ctx).await?;
            if plan.capability_id.is_empty() {
                plan.capability_id = capability_id.clone();
            }
            let duration_ms = started.elapsed().as_millis();
            let decision = if plan.approval.is_some() {
                "approval_required"
            } else if plan.policy.is_some() {
                "policy"
            } else if plan.providers.is_some() {
                "provider_selection"
            } else if plan.intent.is_some() {
                "intent"
            } else if plan.budget.is_some() {
                "budget"
            } else if plan.required_tools.is_some() {
                "tools"
            } else if plan.identity.is_some() {
                "identity"
            } else {
                "noop"
            };
            result.decision_log.push(CapabilityDecisionLog {
                capability_id: capability_id.clone(),
                decision: decision.to_string(),
                inputs: serde_json::json!({
                    "request_id": ctx.request_id,
                    "model": ctx.request.model,
                    "message_count": ctx.request.messages.len(),
                }),
                outputs: serde_json::to_value(&plan).unwrap_or_else(|_| serde_json::json!({})),
                duration_ms,
                confidence: plan.confidence,
            });
            result.plans.push(plan);
        }

        Ok(result)
    }
}
