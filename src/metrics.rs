use std::time::Duration;

use anyhow::{Context, Result};
use prometheus::{
    CounterVec, Encoder, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, Registry,
    TextEncoder,
};

#[derive(Clone)]
pub struct Metrics {
    pub registry: Registry,
    pub requests_total: IntCounterVec,
    pub provider_errors_total: IntCounterVec,
    pub plugin_errors_total: IntCounter,
    pub latency_seconds: HistogramVec,
    pub tokens_total: CounterVec,
    pub cost_total_usd: CounterVec,
}

impl Metrics {
    pub fn new() -> Result<Self> {
        let registry = Registry::new();

        let requests_total = IntCounterVec::new(
            prometheus::Opts::new("aether_requests_total", "Total requests"),
            &["status"],
        )?;
        let provider_errors_total = IntCounterVec::new(
            prometheus::Opts::new("aether_provider_errors_total", "Provider errors"),
            &["provider"],
        )?;
        let plugin_errors_total = IntCounter::new("aether_plugin_errors_total", "Plugin errors")?;
        let latency_seconds = HistogramVec::new(
            HistogramOpts::new("aether_request_latency_seconds", "Request latency seconds"),
            &["route"],
        )?;
        let tokens_total = CounterVec::new(
            prometheus::Opts::new("aether_tokens_total", "Total output tokens by model"),
            &["model"],
        )?;
        let cost_total_usd = CounterVec::new(
            prometheus::Opts::new(
                "aether_cost_total_usd",
                "Total estimated cost in USD by model",
            ),
            &["model"],
        )?;

        registry.register(Box::new(requests_total.clone()))?;
        registry.register(Box::new(provider_errors_total.clone()))?;
        registry.register(Box::new(plugin_errors_total.clone()))?;
        registry.register(Box::new(latency_seconds.clone()))?;
        registry.register(Box::new(tokens_total.clone()))?;
        registry.register(Box::new(cost_total_usd.clone()))?;

        Ok(Self {
            registry,
            requests_total,
            provider_errors_total,
            plugin_errors_total,
            latency_seconds,
            tokens_total,
            cost_total_usd,
        })
    }

    pub fn observe_latency(&self, route: &str, duration: Duration) {
        self.latency_seconds
            .with_label_values(&[route])
            .observe(duration.as_secs_f64());
    }

    pub fn render(&self) -> Result<String> {
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        TextEncoder::new()
            .encode(&metric_families, &mut buffer)
            .context("failed to encode metrics")?;
        String::from_utf8(buffer).context("metrics are not valid utf8")
    }
}
