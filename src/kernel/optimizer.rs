use std::collections::{BTreeSet, HashMap};

use crate::runtime::{
    execution_intent::ExecutionIntent,
    execution_plan::ExecutionPlan,
    planner_result::{ApprovalRequest, PlannerResult},
};

use super::planning::PlanningContext;

pub struct IntentOptimizer;

impl IntentOptimizer {
    pub fn optimize(
        planning_ctx: &PlanningContext,
        graph: &ExecutionPlan,
        planner_result: PlannerResult,
    ) -> ExecutionIntent {
        let mut selected_provider = planning_ctx
            .request
            .model
            .split_once(':')
            .map(|(provider, _)| provider.to_string());
        let mut selected_model = planning_ctx.request.model.clone();
        let mut estimated_tokens = planning_ctx.request.messages.len() as u64 * 256;
        let mut estimated_cost = estimate_cost(&selected_model, estimated_tokens as f64);
        let mut required_tools = BTreeSet::new();
        let mut approvals = Vec::<ApprovalRequest>::new();
        let mut policies = Vec::<String>::new();
        let mut metadata = HashMap::<String, serde_json::Value>::new();
        let mut seen_intent: Option<String> = None;

        for plan in &planner_result.plans {
            if let Some(policy) = &plan.policy {
                policies.extend(policy.matched_rules.iter().cloned());
                if policy.approval_required && plan.approval.is_none() {
                    approvals.push(ApprovalRequest {
                        reason: "policy requires approval".to_string(),
                    });
                }
            }
            if let Some(approval) = &plan.approval {
                approvals.push(approval.clone());
            }
            if let Some(budget) = &plan.budget {
                estimated_tokens = budget.estimated_tokens;
                estimated_cost = budget.estimated_cost_usd;
                metadata.insert(
                    "remaining_budget".to_string(),
                    serde_json::json!(budget.remaining_budget),
                );
            }
            if let Some(intent) = &plan.intent {
                if let Some(previous) = &seen_intent
                    && previous != &intent.intent
                {
                    metadata
                        .entry("conflicting_intents".to_string())
                        .or_insert_with(|| serde_json::json!([]));
                }
                seen_intent = Some(intent.intent.clone());
                metadata.insert(
                    "semantic_intent".to_string(),
                    serde_json::json!(intent.intent.clone()),
                );
                metadata.insert(
                    "semantic_confidence".to_string(),
                    serde_json::json!(intent.confidence),
                );
            }
            if let Some(providers) = &plan.providers {
                if let Some(first) = providers.providers.first() {
                    selected_provider = Some(first.clone());
                }
                if let Some(model) = &providers.selected_model {
                    selected_model = model.clone();
                }
            }
            if let Some(tools) = &plan.required_tools {
                for tool in &tools.tools {
                    required_tools.insert(tool.clone());
                }
            }
            metadata.extend(plan.metadata.clone());
        }

        ExecutionIntent::new(
            planning_ctx.request.clone(),
            selected_provider,
            selected_model,
            estimated_cost,
            estimated_tokens,
            required_tools.into_iter().collect(),
            approvals,
            policies,
            graph.parallel_groups.clone(),
            metadata,
            planner_result.decision_log,
        )
    }
}

fn estimate_cost(model: &str, total_tokens: f64) -> f64 {
    let per_1k = if model.contains("sonnet") {
        0.003
    } else if model.contains("gpt-4") {
        0.01
    } else {
        0.001
    };

    (total_tokens / 1000.0) * per_1k
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        kernel::planning::PlanningContext,
        models::{ChatCompletionRequest, ChatMessage},
        runtime::{
            execution_plan::ExecutionPlan,
            planner_result::{
                BudgetEstimate, CapabilityPlan, IntentClassification, PlannerResult,
                ProviderSelectionCandidates,
            },
        },
    };

    use super::IntentOptimizer;

    #[test]
    fn merges_planner_outputs_and_selects_provider() {
        let planning_ctx = PlanningContext {
            request_id: "r1".to_string(),
            request: ChatCompletionRequest {
                model: "local:model-a".to_string(),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: serde_json::json!("hello"),
                }],
                tools: None,
                stream: false,
                extra: HashMap::new(),
            },
            identity: Default::default(),
            metadata: Default::default(),
            budget: Default::default(),
            policy: Default::default(),
            headers: HashMap::new(),
        };

        let graph = ExecutionPlan {
            capabilities: vec![],
            execution_order: vec!["a".to_string(), "b".to_string()],
            missing_dependencies: vec![],
            parallel_groups: vec![vec!["a".to_string()], vec!["b".to_string()]],
        };
        let result = PlannerResult {
            plans: vec![
                CapabilityPlan {
                    capability_id: "budget".to_string(),
                    budget: Some(BudgetEstimate {
                        estimated_tokens: 2048,
                        estimated_cost_usd: 0.05,
                        remaining_budget: 6000,
                    }),
                    ..Default::default()
                },
                CapabilityPlan {
                    capability_id: "provider".to_string(),
                    providers: Some(ProviderSelectionCandidates {
                        providers: vec!["anthropic".to_string()],
                        selected_model: Some("anthropic:sonnet".to_string()),
                    }),
                    intent: Some(IntentClassification {
                        intent: "chat".to_string(),
                        confidence: 0.9,
                    }),
                    ..Default::default()
                },
            ],
            decision_log: vec![],
        };

        let intent = IntentOptimizer::optimize(&planning_ctx, &graph, result);
        assert_eq!(intent.selected_provider(), Some("anthropic"));
        assert_eq!(intent.model(), "anthropic:sonnet");
        assert_eq!(intent.estimated_tokens(), 2048);
        assert_eq!(intent.execution_graph().len(), 2);
    }

    #[test]
    fn conflicting_intents_are_captured_in_metadata() {
        let planning_ctx = PlanningContext {
            request_id: "r1".to_string(),
            request: ChatCompletionRequest {
                model: "local:model-a".to_string(),
                messages: vec![],
                tools: None,
                stream: false,
                extra: HashMap::new(),
            },
            identity: Default::default(),
            metadata: Default::default(),
            budget: Default::default(),
            policy: Default::default(),
            headers: HashMap::new(),
        };
        let graph = ExecutionPlan::default();
        let result = PlannerResult {
            plans: vec![
                CapabilityPlan {
                    capability_id: "r1".to_string(),
                    intent: Some(IntentClassification {
                        intent: "chat".to_string(),
                        confidence: 0.9,
                    }),
                    ..Default::default()
                },
                CapabilityPlan {
                    capability_id: "r2".to_string(),
                    intent: Some(IntentClassification {
                        intent: "tools".to_string(),
                        confidence: 0.7,
                    }),
                    ..Default::default()
                },
            ],
            decision_log: vec![],
        };

        let intent = IntentOptimizer::optimize(&planning_ctx, &graph, result);
        assert!(intent.metadata().contains_key("conflicting_intents"));
    }
}
