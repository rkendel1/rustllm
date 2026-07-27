use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};

use crate::{providers::health::ProviderHealth, runtime::observation::ExecutionObservation};

pub trait KnowledgeStore: Send + Sync {
    fn load(&self) -> RuntimeKnowledgeSnapshot;
    fn save(&self, snapshot: RuntimeKnowledgeSnapshot);
    fn observe(&self, observation: ExecutionObservation);
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RuntimeKnowledgeSnapshot {
    #[serde(default)]
    pub providers: HashMap<String, ProviderHealth>,
    #[serde(default)]
    pub observations: u64,
}

#[derive(Clone)]
pub struct MemoryKnowledgeStore {
    state: Arc<RwLock<RuntimeKnowledgeSnapshot>>,
    rolling_window: usize,
}

impl MemoryKnowledgeStore {
    pub fn new(rolling_window: usize) -> Self {
        Self {
            state: Arc::new(RwLock::new(RuntimeKnowledgeSnapshot::default())),
            rolling_window,
        }
    }
}

impl Default for MemoryKnowledgeStore {
    fn default() -> Self {
        Self::new(256)
    }
}

impl KnowledgeStore for MemoryKnowledgeStore {
    fn load(&self) -> RuntimeKnowledgeSnapshot {
        self.state.read().map(|s| s.clone()).unwrap_or_default()
    }

    fn save(&self, snapshot: RuntimeKnowledgeSnapshot) {
        if let Ok(mut state) = self.state.write() {
            *state = snapshot;
        }
    }

    fn observe(&self, observation: ExecutionObservation) {
        if let Ok(mut state) = self.state.write() {
            state.observations += 1;
            let provider = state
                .providers
                .entry(observation.provider.clone())
                .or_insert_with(|| ProviderHealth::new(self.rolling_window));
            provider.observe(&observation);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_observations_per_provider() {
        let store = MemoryKnowledgeStore::new(20);
        store.observe(ExecutionObservation::new(
            "openai".to_string(),
            "gpt-4o-mini".to_string(),
            120,
            false,
            0,
            true,
            Some(100),
            Some(0.01),
            None,
            Some(200),
        ));
        store.observe(ExecutionObservation::new(
            "openai".to_string(),
            "gpt-4o-mini".to_string(),
            300,
            true,
            1,
            false,
            None,
            None,
            Some("timeout".to_string()),
            Some(504),
        ));

        let snapshot = store.load();
        assert_eq!(snapshot.observations, 2);
        let health = snapshot.providers.get("openai").expect("provider exists");
        assert_eq!(health.total_requests, 2);
        assert_eq!(health.success_rate, 0.5);
        assert_eq!(health.latency_p95, 300);
    }
}
