use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub uptime_seconds: u64,
}

impl HealthStatus {
    pub fn healthy(uptime_seconds: u64) -> Self {
        Self {
            status: "healthy".to_string(),
            timestamp: chrono::Utc::now(),
            uptime_seconds,
        }
    }
}
