use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityDecisionLog {
    pub capability_id: String,
    pub decision: String,
    pub inputs: serde_json::Value,
    pub outputs: serde_json::Value,
    pub duration_ms: u128,
    pub confidence: Option<f64>,
}
