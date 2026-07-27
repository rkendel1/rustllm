use serde::{Deserialize, Serialize};

use crate::providers::health::ProviderHealth;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RoutingStrategy {
    #[default]
    Adaptive,
    FirstAvailable,
    Cheapest,
    Fastest,
    LowestLatency,
    HighestSuccess,
    Balanced,
}

impl RoutingStrategy {
    pub fn from_str(raw: &str) -> Self {
        match raw {
            "first_available" => Self::FirstAvailable,
            "cheapest" => Self::Cheapest,
            "fastest" => Self::Fastest,
            "lowest_latency" => Self::LowestLatency,
            "highest_success" => Self::HighestSuccess,
            "balanced" => Self::Balanced,
            _ => Self::Adaptive,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Adaptive => "adaptive",
            Self::FirstAvailable => "first_available",
            Self::Cheapest => "cheapest",
            Self::Fastest => "fastest",
            Self::LowestLatency => "lowest_latency",
            Self::HighestSuccess => "highest_success",
            Self::Balanced => "balanced",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderScoreBreakdown {
    pub latency: f64,
    pub availability: f64,
    pub cost: f64,
    pub policy: f64,
    pub intent_fit: f64,
    pub success: f64,
    pub final_score: f64,
}

impl ProviderScoreBreakdown {
    pub fn fallback() -> Self {
        Self {
            latency: 0.5,
            availability: 0.5,
            cost: 0.5,
            policy: 1.0,
            intent_fit: 0.7,
            success: 0.5,
            final_score: 0.62,
        }
    }
}

pub fn score_provider(
    health: Option<&ProviderHealth>,
    configured_rank: usize,
    intent: Option<&str>,
    remaining_budget: Option<u64>,
) -> ProviderScoreBreakdown {
    let default = ProviderScoreBreakdown::fallback();
    let Some(health) = health else {
        return default;
    };

    let latency = latency_score(health.latency_p50);
    let availability = clamp(health.availability);
    let success = clamp(health.success_rate);
    let cost = cost_score(health.average_cost);
    let policy = policy_score(configured_rank);
    let intent_fit = intent_score(intent, health.token_throughput, remaining_budget);
    let final_score = clamp(
        latency * 0.24
            + availability * 0.24
            + cost * 0.18
            + policy * 0.12
            + intent_fit * 0.12
            + success * 0.10,
    );

    ProviderScoreBreakdown {
        latency,
        availability,
        cost,
        policy,
        intent_fit,
        success,
        final_score,
    }
}

fn latency_score(latency_ms: u64) -> f64 {
    if latency_ms == 0 {
        return 0.7;
    }
    clamp(1.0 - (latency_ms as f64 / 2000.0))
}

fn cost_score(avg_cost: f64) -> f64 {
    if avg_cost <= 0.0 {
        return 0.7;
    }
    clamp(1.0 - (avg_cost / 0.05))
}

fn policy_score(configured_rank: usize) -> f64 {
    match configured_rank {
        0 => 1.0,
        1 => 0.95,
        2 => 0.9,
        _ => 0.85,
    }
}

fn intent_score(intent: Option<&str>, throughput: f64, remaining_budget: Option<u64>) -> f64 {
    let semantic = match intent {
        Some("tools") => 0.92,
        Some("chat") => 0.88,
        _ => 0.8,
    };
    let throughput_component = if throughput <= 0.0 {
        0.75
    } else {
        clamp(throughput / 200.0)
    };
    let budget_component = match remaining_budget {
        Some(b) if b < 500 => 0.6,
        Some(b) if b < 2_000 => 0.8,
        _ => 1.0,
    };
    clamp((semantic * 0.6) + (throughput_component * 0.2) + (budget_component * 0.2))
}

fn clamp(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use crate::{providers::health::ProviderHealth, runtime::observation::ExecutionObservation};

    use super::*;

    #[test]
    fn scoring_is_deterministic() {
        let mut health = ProviderHealth::new(50);
        health.observe(&ExecutionObservation::new(
            "openai".to_string(),
            "gpt-4o-mini".to_string(),
            220,
            false,
            0,
            true,
            Some(300),
            Some(0.01),
            None,
            Some(200),
        ));

        let a = score_provider(Some(&health), 0, Some("chat"), Some(2000));
        let b = score_provider(Some(&health), 0, Some("chat"), Some(2000));
        assert_eq!(a, b);
    }
}
