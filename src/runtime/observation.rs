use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionObservation {
    pub provider: String,
    pub model: String,
    pub latency_ms: u64,
    pub streamed: bool,
    pub retries: u32,
    pub success: bool,
    pub tokens: Option<u64>,
    pub estimated_cost: Option<f64>,
    pub error: Option<String>,
    pub http_status: Option<u16>,
    pub timestamp_ms: u64,
}

impl ExecutionObservation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider: String,
        model: String,
        latency_ms: u64,
        streamed: bool,
        retries: u32,
        success: bool,
        tokens: Option<u64>,
        estimated_cost: Option<f64>,
        error: Option<String>,
        http_status: Option<u16>,
    ) -> Self {
        Self {
            provider,
            model,
            latency_ms,
            streamed,
            retries,
            success,
            tokens,
            estimated_cost,
            error,
            http_status,
            timestamp_ms: now_ms(),
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
