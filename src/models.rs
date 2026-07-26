use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub tools: Option<serde_json::Value>,
    #[serde(default)]
    pub stream: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayErrorBody {
    pub error: GatewayErrorMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayErrorMessage {
    pub message: String,
    #[serde(rename = "type")]
    pub kind: String,
}

pub fn openai_error(message: impl Into<String>, kind: impl Into<String>) -> GatewayErrorBody {
    GatewayErrorBody {
        error: GatewayErrorMessage {
            message: message.into(),
            kind: kind.into(),
        },
    }
}
