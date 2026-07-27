use std::collections::HashMap;

use crate::models::ChatCompletionRequest;

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
}

impl Default for BudgetContext {
    fn default() -> Self {
        Self {
            max_input_tokens: 8_192,
            estimated_cost_usd: 0.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PolicyContext {
    pub denied_by: Option<String>,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub intent: Option<String>,
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
