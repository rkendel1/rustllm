use std::time::{Duration, Instant};

use dashmap::DashMap;

#[derive(Debug)]
pub struct RateLimiter {
    global_per_minute: u64,
    per_key_per_minute: u64,
    global: DashMap<String, CounterWindow>,
    per_key: DashMap<String, CounterWindow>,
}

#[derive(Debug, Clone)]
struct CounterWindow {
    started_at: Instant,
    count: u64,
}

impl RateLimiter {
    pub fn new(global_per_minute: u64, per_key_per_minute: u64) -> Self {
        Self {
            global_per_minute,
            per_key_per_minute,
            global: DashMap::new(),
            per_key: DashMap::new(),
        }
    }

    pub fn check(&self, key: Option<&str>) -> bool {
        let global_ok = self.bump(&self.global, "global", self.global_per_minute);
        if !global_ok {
            return false;
        }

        if let Some(k) = key {
            return self.bump(&self.per_key, k, self.per_key_per_minute);
        }

        true
    }

    fn bump(&self, map: &DashMap<String, CounterWindow>, id: &str, limit: u64) -> bool {
        if limit == 0 {
            return true;
        }

        let now = Instant::now();
        let window = Duration::from_secs(60);

        let mut entry = map.entry(id.to_string()).or_insert(CounterWindow {
            started_at: now,
            count: 0,
        });

        if now.duration_since(entry.started_at) >= window {
            entry.started_at = now;
            entry.count = 0;
        }

        if entry.count >= limit {
            return false;
        }
        entry.count += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_per_key_limit() {
        let limiter = RateLimiter::new(100, 1);
        assert!(limiter.check(Some("a")));
        assert!(!limiter.check(Some("a")));
        assert!(limiter.check(Some("b")));
    }
}
