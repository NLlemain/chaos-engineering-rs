use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HttpFaultType {
    Status { code: u16, body: String },
    Latency { delay: Duration },
    StripHeaders { headers: Vec<String> },
    Slowloris { chunk_delay: Duration },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpFaultConfig {
    pub path_pattern: String,
    pub fault_type: HttpFaultType,
    pub rate: f64,
}

impl Default for HttpFaultConfig {
    fn default() -> Self {
        Self {
            path_pattern: "/api/*".to_string(),
            fault_type: HttpFaultType::Status {
                code: 500,
                body: "Chaos Injected Internal Server Error".to_string(),
            },
            rate: 1.0,
        }
    }
}

pub struct HttpFaultInjector {
    config: HttpFaultConfig,
}

impl Default for HttpFaultInjector {
    fn default() -> Self {
        Self::new(HttpFaultConfig::default())
    }
}

impl HttpFaultInjector {
    pub fn new(config: HttpFaultConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> HttpFaultBuilder {
        HttpFaultBuilder::default()
    }
}

#[async_trait]
impl Injector for HttpFaultInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        info!(
            "Injecting HTTP Fault on path pattern '{}' with type {:?} (rate: {})",
            self.config.path_pattern, self.config.fault_type, self.config.rate
        );

        let metadata = serde_json::json!({
            "path_pattern": self.config.path_pattern,
            "fault_type": format!("{:?}", self.config.fault_type),
            "rate": self.config.rate,
        });

        Ok(InjectionHandle::new("http_fault", target.clone(), metadata))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!("Removing HTTP Fault on path pattern '{}'", self.config.path_pattern);
        Ok(())
    }

    fn name(&self) -> &str {
        "http_fault"
    }
}

#[derive(Default)]
pub struct HttpFaultBuilder {
    path_pattern: Option<String>,
    fault_type: Option<HttpFaultType>,
    rate: Option<f64>,
}

impl HttpFaultBuilder {
    pub fn path_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.path_pattern = Some(pattern.into());
        self
    }

    pub fn status(mut self, code: u16, body: impl Into<String>) -> Self {
        self.fault_type = Some(HttpFaultType::Status {
            code,
            body: body.into(),
        });
        self
    }

    pub fn latency(mut self, delay: Duration) -> Self {
        self.fault_type = Some(HttpFaultType::Latency { delay });
        self
    }

    pub fn rate(mut self, rate: f64) -> Self {
        self.rate = Some(rate.clamp(0.0, 1.0));
        self
    }

    pub fn build(self) -> HttpFaultInjector {
        HttpFaultInjector::new(HttpFaultConfig {
            path_pattern: self.path_pattern.unwrap_or_else(|| "/*".to_string()),
            fault_type: self.fault_type.unwrap_or(HttpFaultType::Status {
                code: 503,
                body: "Service Unavailable".to_string(),
            }),
            rate: self.rate.unwrap_or(1.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_fault_builder() {
        let injector = HttpFaultInjector::builder()
            .path_pattern("/v1/checkout")
            .status(429, "Too Many Requests")
            .rate(0.5)
            .build();

        assert_eq!(injector.config.path_pattern, "/v1/checkout");
        assert_eq!(injector.config.rate, 0.5);
    }
}
