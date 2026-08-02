use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DnsFaultMode {
    Latency { delay: Duration },
    NxDomain,
    Spoof { fake_ip: String },
    Blackhole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsFaultConfig {
    pub domain_pattern: String,
    pub fault_mode: DnsFaultMode,
    pub failure_rate: f64, // 0.0 to 1.0
}

impl Default for DnsFaultConfig {
    fn default() -> Self {
        Self {
            domain_pattern: "*".to_string(),
            fault_mode: DnsFaultMode::Latency {
                delay: Duration::from_millis(500),
            },
            failure_rate: 1.0,
        }
    }
}

pub struct DnsFaultInjector {
    config: DnsFaultConfig,
}

impl Default for DnsFaultInjector {
    fn default() -> Self {
        Self::new(DnsFaultConfig::default())
    }
}

impl DnsFaultInjector {
    pub fn new(config: DnsFaultConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> DnsFaultBuilder {
        DnsFaultBuilder::default()
    }
}

#[async_trait]
impl Injector for DnsFaultInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        info!(
            "Injecting DNS fault on pattern '{}' mode {:?} (failure rate: {}%) for target {:?}",
            self.config.domain_pattern,
            self.config.fault_mode,
            self.config.failure_rate * 100.0,
            target
        );

        let metadata = serde_json::json!({
            "domain_pattern": self.config.domain_pattern,
            "fault_mode": format!("{:?}", self.config.fault_mode),
            "failure_rate": self.config.failure_rate,
        });

        Ok(InjectionHandle::new("dns_fault", target.clone(), metadata))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!(
            "Removing DNS fault injection for pattern '{}'",
            self.config.domain_pattern
        );
        Ok(())
    }

    fn name(&self) -> &str {
        "dns_fault"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["CAP_NET_ADMIN".to_string()]
    }
}

#[derive(Default)]
pub struct DnsFaultBuilder {
    domain_pattern: Option<String>,
    fault_mode: Option<DnsFaultMode>,
    failure_rate: Option<f64>,
}

impl DnsFaultBuilder {
    pub fn domain_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.domain_pattern = Some(pattern.into());
        self
    }

    pub fn fault_mode(mut self, mode: DnsFaultMode) -> Self {
        self.fault_mode = Some(mode);
        self
    }

    pub fn failure_rate(mut self, rate: f64) -> Self {
        self.failure_rate = Some(rate.clamp(0.0, 1.0));
        self
    }

    pub fn build(self) -> DnsFaultInjector {
        DnsFaultInjector {
            config: DnsFaultConfig {
                domain_pattern: self.domain_pattern.unwrap_or_else(|| "*".to_string()),
                fault_mode: self.fault_mode.unwrap_or(DnsFaultMode::NxDomain),
                failure_rate: self.failure_rate.unwrap_or(1.0),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_builder() {
        let injector = DnsFaultInjector::builder()
            .domain_pattern("*.api.internal")
            .fault_mode(DnsFaultMode::Spoof {
                fake_ip: "127.0.0.1".to_string(),
            })
            .failure_rate(0.8)
            .build();

        assert_eq!(injector.config.domain_pattern, "*.api.internal");
        assert_eq!(injector.config.failure_rate, 0.8);
    }
}
