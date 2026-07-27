use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityManifest {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub before: Vec<String>,
    #[serde(default)]
    pub after: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub cost: u32,
}

impl CapabilityManifest {
    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            return Err(anyhow!("capability manifest id cannot be empty"));
        }
        if self.version.trim().is_empty() {
            return Err(anyhow!("capability '{}' version cannot be empty", self.id));
        }
        Ok(())
    }
}
