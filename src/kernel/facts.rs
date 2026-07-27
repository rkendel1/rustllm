use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct RuntimeFacts {
    values: HashMap<String, serde_json::Value>,
}

impl RuntimeFacts {
    pub fn publish<V: Into<serde_json::Value>>(&mut self, key: impl Into<String>, value: V) {
        self.values.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.values.get(key)
    }

    pub fn get_str(&self, key: &str) -> Option<String> {
        self.get(key)
            .and_then(|v| v.as_str().map(ToString::to_string))
    }

    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(serde_json::Value::as_f64)
    }

    pub fn all(&self) -> &HashMap<String, serde_json::Value> {
        &self.values
    }
}
