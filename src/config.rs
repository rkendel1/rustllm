use std::{collections::HashMap, env, fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub listener: ListenerConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub limits: LimitsConfig,
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub model_aliases: HashMap<String, Vec<ModelRoute>>,
    #[serde(default)]
    pub plugins: Vec<PluginConfig>,
    #[serde(default)]
    pub observability: ObservabilityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
}

fn default_bind() -> String {
    "0.0.0.0:3000".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub virtual_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    #[serde(default = "default_global")]
    pub global_per_minute: u64,
    #[serde(default = "default_per_key")]
    pub per_key_per_minute: u64,
    #[serde(default = "default_retries")]
    pub retries: usize,
    #[serde(default = "default_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    #[serde(default = "default_total_timeout_ms")]
    pub total_timeout_ms: u64,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            global_per_minute: default_global(),
            per_key_per_minute: default_per_key(),
            retries: default_retries(),
            connect_timeout_ms: default_connect_timeout_ms(),
            total_timeout_ms: default_total_timeout_ms(),
        }
    }
}

fn default_global() -> u64 {
    5_000
}
fn default_per_key() -> u64 {
    500
}
fn default_retries() -> usize {
    2
}
fn default_connect_timeout_ms() -> u64 {
    2_500
}
fn default_total_timeout_ms() -> u64 {
    60_000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl ProviderConfig {
    pub fn resolved_api_key(&self) -> Option<String> {
        if let Some(env_key) = &self.api_key_env {
            if let Ok(v) = env::var(env_key) {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        self.api_key.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    OpenAiCompat,
    Anthropic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRoute {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    #[serde(default)]
    pub log_level: Option<String>,
}

impl AppConfig {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;
        let config: Self = serde_yaml::from_str(&raw)
            .with_context(|| format!("failed to parse yaml config at {}", path.display()))?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aliases_and_defaults() {
        let yaml = r#"
listener:
  bind: "127.0.0.1:3001"
providers:
  openai:
    kind: open_ai_compat
    base_url: "https://api.openai.com"
model_aliases:
  gpt-4o:
    - provider: openai
      model: gpt-4o-mini
"#;
        let config: AppConfig = serde_yaml::from_str(yaml).expect("valid config");
        assert_eq!(config.listener.bind, "127.0.0.1:3001");
        assert_eq!(config.limits.retries, 2);
        assert_eq!(config.model_aliases["gpt-4o"][0].model, "gpt-4o-mini");
    }
}
