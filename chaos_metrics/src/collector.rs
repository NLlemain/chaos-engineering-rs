use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    Latency(Duration),
    Error { error_type: String },
    Success,
    Recovery { time: Duration },
    Custom { name: String, value: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub metric_type: MetricType,
    pub timestamp: DateTime<Utc>,
    pub labels: std::collections::HashMap<String, String>,
}

pub struct MetricsCollector {
    metrics: Arc<RwLock<Vec<Metric>>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn record(&self, metric: Metric) {
        self.metrics.write().await.push(metric);
    }

    pub async fn record_latency(&self, latency: Duration) {
        self.record(Metric {
            metric_type: MetricType::Latency(latency),
            timestamp: Utc::now(),
            labels: std::collections::HashMap::new(),
        })
        .await;
    }

    pub async fn record_error(&self, error_type: impl Into<String>) {
        self.record(Metric {
            metric_type: MetricType::Error {
                error_type: error_type.into(),
            },
            timestamp: Utc::now(),
            labels: std::collections::HashMap::new(),
        })
        .await;
    }

    pub async fn record_success(&self) {
        self.record(Metric {
            metric_type: MetricType::Success,
            timestamp: Utc::now(),
            labels: std::collections::HashMap::new(),
        })
        .await;
    }

    pub async fn record_recovery(&self, time: Duration) {
        self.record(Metric {
            metric_type: MetricType::Recovery { time },
            timestamp: Utc::now(),
            labels: std::collections::HashMap::new(),
        })
        .await;
    }

    pub async fn get_metrics(&self) -> Vec<Metric> {
        self.metrics.read().await.clone()
    }

    pub async fn clear(&self) {
        self.metrics.write().await.clear();
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}
