use crate::runtime::{execution_intent::ExecutionIntent, planner_result::ApprovalRequest};

pub struct ApprovalEngine;

impl ApprovalEngine {
    pub fn required(intent: &ExecutionIntent) -> Option<ApprovalRequest> {
        intent.approvals().first().cloned()
    }
}
