use std::collections::HashMap;

use crate::models::ChatCompletionRequest;

use super::facts::RuntimeFacts;

#[derive(Debug, Clone, Default)]
pub struct Identity {
    pub api_key: Option<String>,
    pub authenticated: bool,
    pub plan: String,
}

#[derive(Debug, Clone)]
pub struct BudgetContext {
    pub max_input_tokens: u64,
    pub estimated_cost_usd: f64,
    pub remaining_budget: u64,
    pub tokens_used: u64,
}

impl Default for BudgetContext {
    fn default() -> Self {
        Self {
            max_input_tokens: 8_192,
            estimated_cost_usd: 0.0,
            remaining_budget: 8_192,
            tokens_used: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PolicyContext {
    pub denied_by: Option<String>,
    pub requires_approval: bool,
    pub matched_rules: Vec<String>,
    pub approval_required: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub intent: Option<String>,
    pub confidence: Option<f64>,
    pub priority: Option<String>,
    pub sensitivity: Option<String>,
    pub values: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct RequestContext {
    pub request_id: String,
    pub identity: Identity,
    pub model: ChatCompletionRequest,
    pub metadata: Metadata,
    pub budget: BudgetContext,
    pub policy: PolicyContext,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ResponseContext {
    pub request_id: String,
    pub identity: Identity,
    pub metadata: Metadata,
    pub policy: PolicyContext,
    pub provider_model: Option<String>,
    pub body: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct BudgetState {
    pub remaining_budget: u64,
    pub tokens_used: u64,
    pub estimated_cost_usd: f64,
}

impl Default for BudgetState {
    fn default() -> Self {
        Self {
            remaining_budget: 8_192,
            tokens_used: 0,
            estimated_cost_usd: 0.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PolicyState {
    pub matched_rules: Vec<String>,
    pub approval_required: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SemanticState {
    pub intent: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct CapabilityState {
    pub facts: RuntimeFacts,
    pub budget: BudgetState,
    pub policy: PolicyState,
    pub semantic: SemanticState,
}

impl CapabilityState {
    pub fn from_request(ctx: &RequestContext) -> Self {
        Self {
            facts: RuntimeFacts::default(),
            budget: BudgetState {
                remaining_budget: ctx.budget.remaining_budget,
                tokens_used: ctx.budget.tokens_used,
                estimated_cost_usd: ctx.budget.estimated_cost_usd,
            },
            policy: PolicyState {
                matched_rules: ctx.policy.matched_rules.clone(),
                approval_required: ctx.policy.approval_required || ctx.policy.requires_approval,
            },
            semantic: SemanticState {
                intent: ctx.metadata.intent.clone(),
                confidence: ctx.metadata.confidence,
            },
        }
    }

    pub fn from_response(ctx: &ResponseContext) -> Self {
        Self {
            facts: RuntimeFacts::default(),
            budget: BudgetState::default(),
            policy: PolicyState {
                matched_rules: ctx.policy.matched_rules.clone(),
                approval_required: ctx.policy.approval_required || ctx.policy.requires_approval,
            },
            semantic: SemanticState {
                intent: ctx.metadata.intent.clone(),
                confidence: ctx.metadata.confidence,
            },
        }
    }

    pub fn apply_to_request(&self, ctx: &mut RequestContext) {
        ctx.budget.remaining_budget = self.budget.remaining_budget;
        ctx.budget.tokens_used = self.budget.tokens_used;
        ctx.budget.estimated_cost_usd = self.budget.estimated_cost_usd;
        ctx.policy.matched_rules = self.policy.matched_rules.clone();
        ctx.policy.approval_required = self.policy.approval_required;
        ctx.policy.requires_approval = self.policy.approval_required;
        ctx.metadata.intent = self.semantic.intent.clone();
        ctx.metadata.confidence = self.semantic.confidence;
    }

    pub fn apply_to_response(&self, ctx: &mut ResponseContext) {
        ctx.policy.matched_rules = self.policy.matched_rules.clone();
        ctx.policy.approval_required = self.policy.approval_required;
        ctx.policy.requires_approval = self.policy.approval_required;
        ctx.metadata.intent = self.semantic.intent.clone();
        ctx.metadata.confidence = self.semantic.confidence;
    }
}
