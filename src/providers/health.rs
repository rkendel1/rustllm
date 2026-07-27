use std::{
    collections::{HashMap, VecDeque},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::runtime::observation::ExecutionObservation;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderHealth {
    pub availability: f64,
    pub latency_p50: u64,
    pub latency_p95: u64,
    pub success_rate: f64,
    pub timeout_rate: f64,
    pub last_failure: Option<u64>,
    pub rolling_window: usize,
    pub average_cost: f64,
    #[serde(default)]
    pub status_distribution: HashMap<u16, u64>,
    #[serde(default)]
    pub total_requests: u64,
    #[serde(default)]
    pub total_retries: u64,
    #[serde(default)]
    pub stream_count: u64,
    #[serde(default)]
    pub token_throughput: f64,
    #[serde(default)]
    pub sample_count: u64,
    #[serde(default)]
    pub failure_count: u64,
    #[serde(default)]
    pub timeout_count: u64,
    #[serde(skip)]
    latencies: VecDeque<u64>,
    #[serde(skip)]
    sampled_cost_total: f64,
    #[serde(skip)]
    observed_tokens: u64,
    #[serde(skip)]
    observed_latency_ms: u64,
}

impl ProviderHealth {
    pub fn new(rolling_window: usize) -> Self {
        Self {
            availability: 1.0,
            latency_p50: 0,
            latency_p95: 0,
            success_rate: 1.0,
            timeout_rate: 0.0,
            last_failure: None,
            rolling_window,
            average_cost: 0.0,
            status_distribution: HashMap::new(),
            total_requests: 0,
            total_retries: 0,
            stream_count: 0,
            token_throughput: 0.0,
            sample_count: 0,
            failure_count: 0,
            timeout_count: 0,
            latencies: VecDeque::new(),
            sampled_cost_total: 0.0,
            observed_tokens: 0,
            observed_latency_ms: 0,
        }
    }

    pub fn observe(&mut self, observation: &ExecutionObservation) {
        self.total_requests += 1;
        self.total_retries += observation.retries as u64;
        self.sample_count += 1;

        if observation.streamed {
            self.stream_count += 1;
        }
        if !observation.success {
            self.failure_count += 1;
            self.last_failure = Some(now_ms());
        }
        if observation
            .error
            .as_deref()
            .map(|msg| msg.to_ascii_lowercase().contains("timeout"))
            .unwrap_or(false)
        {
            self.timeout_count += 1;
        }
        if let Some(status) = observation.http_status {
            *self.status_distribution.entry(status).or_insert(0) += 1;
        }
        if let Some(cost) = observation.estimated_cost {
            self.sampled_cost_total += cost;
        }
        if let Some(tokens) = observation.tokens {
            self.observed_tokens += tokens;
        }
        self.observed_latency_ms += observation.latency_ms;

        self.latencies.push_back(observation.latency_ms);
        while self.latencies.len() > self.rolling_window {
            self.latencies.pop_front();
        }

        self.latency_p50 = percentile(&self.latencies, 0.50);
        self.latency_p95 = percentile(&self.latencies, 0.95);
        self.success_rate = if self.sample_count == 0 {
            1.0
        } else {
            (self.sample_count - self.failure_count) as f64 / self.sample_count as f64
        };
        self.timeout_rate = if self.sample_count == 0 {
            0.0
        } else {
            self.timeout_count as f64 / self.sample_count as f64
        };
        self.availability = self.success_rate;
        self.average_cost = if self.sample_count == 0 {
            0.0
        } else {
            self.sampled_cost_total / self.sample_count as f64
        };
        self.token_throughput = if self.observed_latency_ms == 0 {
            0.0
        } else {
            self.observed_tokens as f64 / (self.observed_latency_ms as f64 / 1000.0)
        };
    }
}

fn percentile(latencies: &VecDeque<u64>, quantile: f64) -> u64 {
    if latencies.is_empty() {
        return 0;
    }

    let mut ordered = latencies.iter().copied().collect::<Vec<_>>();
    ordered.sort_unstable();
    let idx = (((ordered.len() - 1) as f64) * quantile).round() as usize;
    ordered[idx.min(ordered.len() - 1)]
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(latency_ms: u64) -> ExecutionObservation {
        ExecutionObservation::new(
            "openai".to_string(),
            "gpt-4o-mini".to_string(),
            latency_ms,
            false,
            0,
            true,
            Some(100),
            Some(0.01),
            None,
            Some(200),
        )
    }

    #[test]
    fn updates_latency_percentiles_in_window() {
        let mut health = ProviderHealth::new(5);
        for latency in [100, 120, 130, 400, 500] {
            health.observe(&observation(latency));
        }

        assert_eq!(health.latency_p50, 130);
        assert_eq!(health.latency_p95, 500);
    }

    #[test]
    fn tracks_failure_and_timeout_rates() {
        let mut health = ProviderHealth::new(5);
        health.observe(&observation(120));
        health.observe(&ExecutionObservation::new(
            "openai".to_string(),
            "gpt-4o-mini".to_string(),
            700,
            false,
            1,
            false,
            None,
            None,
            Some("request timeout".to_string()),
            Some(504),
        ));

        assert_eq!(health.success_rate, 0.5);
        assert_eq!(health.timeout_rate, 0.5);
        assert_eq!(health.status_distribution.get(&504), Some(&1));
        assert!(health.last_failure.is_some());
    }
}
