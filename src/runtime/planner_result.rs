use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::decision::CapabilityDecisionLog;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdentityDecision {
    pub authenticated: bool,
    pub api_key_present: bool,
    pub plan: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicyDecision {
    pub allowed: bool,
    #[serde(default)]
    pub matched_rules: Vec<String>,
    pub approval_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BudgetEstimate {
    pub estimated_tokens: u64,
    pub estimated_cost_usd: f64,
    pub remaining_budget: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntentClassification {
    pub intent: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderSelectionCandidates {
    #[serde(default)]
    pub providers: Vec<String>,
    pub selected_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequiredTools {
    #[serde(default)]
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApprovalRequest {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CapabilityPlan {
    pub capability_id: String,
    pub identity: Option<IdentityDecision>,
    pub policy: Option<PolicyDecision>,
    pub budget: Option<BudgetEstimate>,
    pub intent: Option<IntentClassification>,
    pub providers: Option<ProviderSelectionCandidates>,
    pub required_tools: Option<RequiredTools>,
    pub approval: Option<ApprovalRequest>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PlannerResult {
    #[serde(default)]
    pub plans: Vec<CapabilityPlan>,
    #[serde(default)]
    pub decision_log: Vec<CapabilityDecisionLog>,
}
