use std::{collections::HashMap, sync::Arc};

use anyhow::{Result, anyhow};

use super::{capability::Capability, manifest::CapabilityManifest};

pub struct CapabilityRegistry {
    capabilities: HashMap<String, Arc<dyn Capability>>,
    manifests: HashMap<String, CapabilityManifest>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self {
            capabilities: HashMap::new(),
            manifests: HashMap::new(),
        }
    }

    pub fn register(&mut self, capability: Box<dyn Capability>) -> Result<()> {
        let manifest = capability.manifest();
        manifest.validate()?;

        if let Some(existing) = self.manifests.get(&manifest.id) {
            if existing.version != manifest.version {
                return Err(anyhow!(
                    "incompatible versions for capability '{}': '{}' and '{}'",
                    manifest.id,
                    existing.version,
                    manifest.version
                ));
            }
            return Err(anyhow!("duplicate capability id '{}'", manifest.id));
        }

        let id = manifest.id.clone();
        let capability = Arc::<dyn Capability>::from(capability);
        self.capabilities.insert(id.clone(), capability);
        self.manifests.insert(id, manifest);
        Ok(())
    }

    pub fn register_many(&mut self, capabilities: Vec<Box<dyn Capability>>) -> Result<()> {
        for capability in capabilities {
            self.register(capability)?;
        }
        Ok(())
    }

    pub fn capability(&self, id: &str) -> Option<Arc<dyn Capability>> {
        self.capabilities.get(id).cloned()
    }

    pub fn manifest(&self, id: &str) -> Option<&CapabilityManifest> {
        self.manifests.get(id)
    }

    pub fn version(&self, id: &str) -> Option<&str> {
        self.manifests.get(id).map(|m| m.version.as_str())
    }

    pub fn manifests_for_pipeline(&self, pipeline: &[String]) -> Result<Vec<CapabilityManifest>> {
        if pipeline.is_empty() {
            return Ok(self
                .manifests
                .values()
                .cloned()
                .collect::<Vec<CapabilityManifest>>());
        }

        let mut manifests = Vec::with_capacity(pipeline.len());
        for id in pipeline {
            let Some(manifest) = self.manifests.get(id) else {
                return Err(anyhow!("unknown capability '{}'", id));
            };
            manifests.push(manifest.clone());
        }
        Ok(manifests)
    }
}
