use crate::collector::{Metric, MetricType};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    pub total_requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub error_rate: f64,
    pub latency_p50: Duration,
    pub latency_p95: Duration,
    pub latency_p99: Duration,
    pub latency_p999: Duration,
    pub average_latency: Duration,
    pub min_latency: Duration,
    pub max_latency: Duration,
    pub average_recovery_time: Duration,
}

pub struct MetricsAggregator;

impl MetricsAggregator {
    pub fn aggregate(metrics: &[Metric]) -> AggregatedMetrics {
        let mut latencies: Vec<Duration> = Vec::new();
        let mut recovery_times: Vec<Duration> = Vec::new();
        let mut success_count = 0;
        let mut error_count = 0;

        for metric in metrics {
            match &metric.metric_type {
                MetricType::Latency(duration) => {
                    latencies.push(*duration);
                }
                MetricType::Success => {
                    success_count += 1;
                }
                MetricType::Error { .. } => {
                    error_count += 1;
                }
                MetricType::Recovery { time } => {
                    recovery_times.push(*time);
                }
                MetricType::Custom { .. } => {}
            }
        }

        // Sort latencies for percentile calculation
        latencies.sort();

        let total_requests = success_count + error_count;
        let error_rate = if total_requests > 0 {
            error_count as f64 / total_requests as f64
        } else {
            0.0
        };

        let (p50, p95, p99, p999, avg, min, max) = if !latencies.is_empty() {
            (
                Self::percentile(&latencies, 0.50),
                Self::percentile(&latencies, 0.95),
                Self::percentile(&latencies, 0.99),
                Self::percentile(&latencies, 0.999),
                Self::average(&latencies),
                *latencies.first().unwrap(),
                *latencies.last().unwrap(),
            )
        } else {
            (
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
            )
        };

        let avg_recovery = if !recovery_times.is_empty() {
            Self::average(&recovery_times)
        } else {
            Duration::ZERO
        };

        AggregatedMetrics {
            total_requests,
            successful_requests: success_count,
            failed_requests: error_count,
            error_rate,
            latency_p50: p50,
            latency_p95: p95,
            latency_p99: p99,
            latency_p999: p999,
            average_latency: avg,
            min_latency: min,
            max_latency: max,
            average_recovery_time: avg_recovery,
        }
    }

    fn percentile(sorted: &[Duration], percentile: f64) -> Duration {
        if sorted.is_empty() {
            return Duration::ZERO;
        }

        let index = ((sorted.len() as f64) * percentile) as usize;
        let index = index.min(sorted.len() - 1);
        sorted[index]
    }

    fn average(durations: &[Duration]) -> Duration {
        if durations.is_empty() {
            return Duration::ZERO;
        }

        let sum: Duration = durations.iter().sum();
        sum / durations.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_aggregation() {
        let metrics = vec![
            Metric {
                metric_type: MetricType::Latency(Duration::from_millis(100)),
                timestamp: Utc::now(),
                labels: Default::default(),
            },
            Metric {
                metric_type: MetricType::Latency(Duration::from_millis(200)),
                timestamp: Utc::now(),
                labels: Default::default(),
            },
            Metric {
                metric_type: MetricType::Success,
                timestamp: Utc::now(),
                labels: Default::default(),
            },
            Metric {
                metric_type: MetricType::Error {
                    error_type: "timeout".to_string(),
                },
                timestamp: Utc::now(),
                labels: Default::default(),
            },
        ];

        let aggregated = MetricsAggregator::aggregate(&metrics);

        assert_eq!(aggregated.total_requests, 2);
        assert_eq!(aggregated.successful_requests, 1);
        assert_eq!(aggregated.failed_requests, 1);
        assert_eq!(aggregated.error_rate, 0.5);
    }
}
