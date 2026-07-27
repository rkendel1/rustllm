use std::collections::{BTreeSet, HashMap};

use crate::runtime::{
    execution_intent::ExecutionIntent,
    execution_plan::ExecutionPlan,
    planner_result::{ApprovalRequest, PlannerResult},
};

use super::{
    knowledge::RuntimeKnowledgeSnapshot,
    planning::PlanningContext,
    scoring::{ProviderScoreBreakdown, RoutingStrategy, score_provider},
};

pub struct IntentOptimizer;

impl IntentOptimizer {
    pub fn optimize(
        planning_ctx: &PlanningContext,
        graph: &ExecutionPlan,
        planner_result: PlannerResult,
        knowledge: &RuntimeKnowledgeSnapshot,
    ) -> ExecutionIntent {
        let mut request = planning_ctx.request.clone();
        let mut selected_provider = request
            .model
            .split_once(':')
            .map(|(provider, _)| provider.to_string());
        let mut selected_model = request.model.clone();
        let mut estimated_tokens = request.messages.len() as u64 * 256;
        let mut estimated_cost = estimate_cost(&selected_model, estimated_tokens as f64);
        let mut required_tools = BTreeSet::new();
        let mut approvals = Vec::<ApprovalRequest>::new();
        let mut policies = Vec::<String>::new();
        let mut metadata = HashMap::<String, serde_json::Value>::new();
        let mut seen_intent: Option<String> = None;
        let mut remaining_budget: Option<u64> = None;
        let mut candidate_providers = Vec::<String>::new();
        let mut candidate_models = HashMap::<String, String>::new();
        let mut routing_strategy = RoutingStrategy::Adaptive;

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
                remaining_budget = Some(budget.remaining_budget);
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
                if !providers.providers.is_empty() {
                    candidate_providers = providers.providers.clone();
                }
                candidate_models.extend(providers.provider_models.clone());
                if let Some(model) = &providers.selected_model {
                    selected_model = model.clone();
                }
                if let Some(strategy) = &providers.strategy {
                    routing_strategy = RoutingStrategy::from_str(strategy);
                }
            }
            if let Some(tools) = &plan.required_tools {
                for tool in &tools.tools {
                    required_tools.insert(tool.clone());
                }
            }
            metadata.extend(plan.metadata.clone());
        }

        let provider_scores = Self::score_candidates(
            &candidate_providers,
            knowledge,
            seen_intent.as_deref(),
            remaining_budget,
        );
        if let Some(provider) = Self::select_provider(
            &candidate_providers,
            &routing_strategy,
            &provider_scores,
            knowledge,
        ) {
            selected_provider = Some(provider.clone());
            if let Some(model) = candidate_models.get(&provider) {
                selected_model = format!("{provider}:{model}");
            }
            if let Some(score) = provider_scores.get(&provider) {
                metadata.insert(
                    "provider_explainability".to_string(),
                    serde_json::json!({
                        "selected_provider": provider,
                        "strategy": routing_strategy.as_str(),
                        "latency_score": score.latency,
                        "success_score": score.success,
                        "budget_score": score.cost,
                        "semantic_score": score.intent_fit,
                        "configured_preference": score.policy,
                        "final_score": score.final_score
                    }),
                );
            }
        }
        if !provider_scores.is_empty() {
            metadata.insert(
                "provider_scores".to_string(),
                serde_json::to_value(&provider_scores).unwrap_or_else(|_| serde_json::json!({})),
            );
        }
        metadata.insert(
            "routing_strategy".to_string(),
            serde_json::json!(routing_strategy.as_str()),
        );

        request.model = selected_model.clone();

        ExecutionIntent::new(
            request,
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

    fn score_candidates(
        candidates: &[String],
        knowledge: &RuntimeKnowledgeSnapshot,
        intent: Option<&str>,
        remaining_budget: Option<u64>,
    ) -> HashMap<String, ProviderScoreBreakdown> {
        let mut scores = HashMap::new();
        for (idx, provider) in candidates.iter().enumerate() {
            let score = score_provider(
                knowledge.providers.get(provider),
                idx,
                intent,
                remaining_budget,
            );
            scores.insert(provider.clone(), score);
        }
        scores
    }

    fn select_provider(
        candidates: &[String],
        strategy: &RoutingStrategy,
        scores: &HashMap<String, ProviderScoreBreakdown>,
        knowledge: &RuntimeKnowledgeSnapshot,
    ) -> Option<String> {
        if candidates.is_empty() {
            return None;
        }

        match strategy {
            RoutingStrategy::FirstAvailable => candidates
                .iter()
                .find(|p| {
                    knowledge
                        .providers
                        .get(*p)
                        .map(|h| h.availability > 0.0)
                        .unwrap_or(true)
                })
                .cloned()
                .or_else(|| candidates.first().cloned()),
            RoutingStrategy::Cheapest => candidates
                .iter()
                .min_by(|a, b| {
                    let a_cost = knowledge
                        .providers
                        .get(*a)
                        .map(|h| h.average_cost)
                        .unwrap_or(0.0);
                    let b_cost = knowledge
                        .providers
                        .get(*b)
                        .map(|h| h.average_cost)
                        .unwrap_or(0.0);
                    a_cost.total_cmp(&b_cost)
                })
                .cloned(),
            RoutingStrategy::Fastest | RoutingStrategy::LowestLatency => candidates
                .iter()
                .min_by_key(|provider| {
                    knowledge
                        .providers
                        .get(*provider)
                        .map(|h| h.latency_p50)
                        .unwrap_or(u64::MAX)
                })
                .cloned(),
            RoutingStrategy::HighestSuccess => candidates
                .iter()
                .max_by(|a, b| {
                    let a_score = knowledge
                        .providers
                        .get(*a)
                        .map(|h| h.success_rate)
                        .unwrap_or(0.0);
                    let b_score = knowledge
                        .providers
                        .get(*b)
                        .map(|h| h.success_rate)
                        .unwrap_or(0.0);
                    a_score.total_cmp(&b_score)
                })
                .cloned(),
            RoutingStrategy::Balanced | RoutingStrategy::Adaptive => candidates
                .iter()
                .max_by(|a, b| {
                    let a_score = scores.get(*a).map(|v| v.final_score).unwrap_or(0.0);
                    let b_score = scores.get(*b).map(|v| v.final_score).unwrap_or(0.0);
                    a_score.total_cmp(&b_score)
                })
                .cloned(),
        }
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
        kernel::knowledge::RuntimeKnowledgeSnapshot,
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
                        strategy: None,
                        provider_models: HashMap::new(),
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

        let intent = IntentOptimizer::optimize(
            &planning_ctx,
            &graph,
            result,
            &RuntimeKnowledgeSnapshot::default(),
        );
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

        let intent = IntentOptimizer::optimize(
            &planning_ctx,
            &graph,
            result,
            &RuntimeKnowledgeSnapshot::default(),
        );
        assert!(intent.metadata().contains_key("conflicting_intents"));
    }
}
