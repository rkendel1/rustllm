use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub enum Hook {
    OnRequest,
    OnResponse,
    OnStreamChunk,
    OnAuth,
}

impl Hook {
    pub fn export_name(self) -> &'static str {
        match self {
            Hook::OnRequest => "on_request",
            Hook::OnResponse => "on_response",
            Hook::OnStreamChunk => "on_stream_chunk",
            Hook::OnAuth => "on_auth",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HookResult {
    #[serde(default = "default_true")]
    pub allow: bool,
    #[serde(default)]
    pub reject_reason: Option<String>,
    #[serde(default)]
    pub body: Option<serde_json::Value>,
}

fn default_true() -> bool {
    true
}
