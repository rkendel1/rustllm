use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::models::ChatCompletionRequest;

use super::{decision::CapabilityDecisionLog, planner_result::ApprovalRequest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionIntent {
    request: ChatCompletionRequest,
    selected_provider: Option<String>,
    model: String,
    estimated_cost: f64,
    estimated_tokens: u64,
    required_tools: Vec<String>,
    approvals: Vec<ApprovalRequest>,
    policies: Vec<String>,
    execution_graph: Vec<Vec<String>>,
    metadata: HashMap<String, serde_json::Value>,
    decision_log: Vec<CapabilityDecisionLog>,
}

impl ExecutionIntent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        request: ChatCompletionRequest,
        selected_provider: Option<String>,
        model: String,
        estimated_cost: f64,
        estimated_tokens: u64,
        required_tools: Vec<String>,
        approvals: Vec<ApprovalRequest>,
        policies: Vec<String>,
        execution_graph: Vec<Vec<String>>,
        metadata: HashMap<String, serde_json::Value>,
        decision_log: Vec<CapabilityDecisionLog>,
    ) -> Self {
        Self {
            request,
            selected_provider,
            model,
            estimated_cost,
            estimated_tokens,
            required_tools,
            approvals,
            policies,
            execution_graph,
            metadata,
            decision_log,
        }
    }

    pub fn request(&self) -> &ChatCompletionRequest {
        &self.request
    }

    pub fn selected_provider(&self) -> Option<&str> {
        self.selected_provider.as_deref()
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn estimated_cost(&self) -> f64 {
        self.estimated_cost
    }

    pub fn estimated_tokens(&self) -> u64 {
        self.estimated_tokens
    }

    pub fn required_tools(&self) -> &[String] {
        &self.required_tools
    }

    pub fn approvals(&self) -> &[ApprovalRequest] {
        &self.approvals
    }

    pub fn policies(&self) -> &[String] {
        &self.policies
    }

    pub fn execution_graph(&self) -> &[Vec<String>] {
        &self.execution_graph
    }

    pub fn metadata(&self) -> &HashMap<String, serde_json::Value> {
        &self.metadata
    }

    pub fn decision_log(&self) -> &[CapabilityDecisionLog] {
        &self.decision_log
    }
}
