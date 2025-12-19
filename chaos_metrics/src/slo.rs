use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SloViolation {
    pub slo_name: String,
    pub threshold: Duration,
    pub actual: Duration,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct SloTracker {
    slos: Vec<Slo>,
    violations: Vec<SloViolation>,
}

#[derive(Debug, Clone)]
struct Slo {
    name: String,
    threshold: Duration,
}

impl SloTracker {
    pub fn new() -> Self {
        Self {
            slos: Vec::new(),
            violations: Vec::new(),
        }
    }

    pub fn add_slo(&mut self, name: impl Into<String>, threshold: Duration) {
        self.slos.push(Slo {
            name: name.into(),
            threshold,
        });
    }

    pub fn check_latency(&mut self, latency: Duration) {
        for slo in &self.slos {
            if latency > slo.threshold {
                self.violations.push(SloViolation {
                    slo_name: slo.name.clone(),
                    threshold: slo.threshold,
                    actual: latency,
                    timestamp: chrono::Utc::now(),
                });
            }
        }
    }

    pub fn violations(&self) -> &[SloViolation] {
        &self.violations
    }

    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }

    pub fn violation_rate(&self, total_requests: usize) -> f64 {
        if total_requests == 0 {
            return 0.0;
        }
        self.violations.len() as f64 / total_requests as f64
    }
}

impl Default for SloTracker {
    fn default() -> Self {
        Self::new()
    }
}
