use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CloudflareErrorCode {
    Error520Unknown,
    Error522ConnectionTimedOut,
    Error524TimeoutOccurred,
    WorkerCpuLimitExceeded,
    WafChallenge403,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareFaultConfig {
    pub error_code: CloudflareErrorCode,
    pub zone_name: String,
    pub rate: f64,
}

impl Default for CloudflareFaultConfig {
    fn default() -> Self {
        Self {
            error_code: CloudflareErrorCode::Error522ConnectionTimedOut,
            zone_name: "example.com".to_string(),
            rate: 1.0,
        }
    }
}

pub struct CloudflareFaultInjector {
    config: CloudflareFaultConfig,
}

impl Default for CloudflareFaultInjector {
    fn default() -> Self {
        Self::new(CloudflareFaultConfig::default())
    }
}

impl CloudflareFaultInjector {
    pub fn new(config: CloudflareFaultConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> CloudflareFaultBuilder {
        CloudflareFaultBuilder::default()
    }
}

#[async_trait]
impl Injector for CloudflareFaultInjector {
    async fn inject(&self, _target: &Target) -> Result<InjectionHandle> {
        Err(ChaosError::InvalidConfig(
            "cloudflare_fault is planned and does not modify edge traffic yet".into(),
        ))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!(
            "Removing Cloudflare Edge Fault injection for zone '{}'",
            self.config.zone_name
        );
        Ok(())
    }

    fn name(&self) -> &str {
        "cloudflare_fault"
    }

    fn status(&self) -> crate::injectors::InjectorStatus {
        crate::injectors::InjectorStatus::Planned
    }
}

#[derive(Default)]
pub struct CloudflareFaultBuilder {
    error_code: Option<CloudflareErrorCode>,
    zone_name: Option<String>,
    rate: Option<f64>,
}

impl CloudflareFaultBuilder {
    pub fn error_code(mut self, code: CloudflareErrorCode) -> Self {
        self.error_code = Some(code);
        self
    }

    pub fn zone_name(mut self, zone: impl Into<String>) -> Self {
        self.zone_name = Some(zone.into());
        self
    }

    pub fn rate(mut self, rate: f64) -> Self {
        self.rate = Some(rate.clamp(0.0, 1.0));
        self
    }

    pub fn build(self) -> CloudflareFaultInjector {
        CloudflareFaultInjector::new(CloudflareFaultConfig {
            error_code: self
                .error_code
                .unwrap_or(CloudflareErrorCode::Error524TimeoutOccurred),
            zone_name: self.zone_name.unwrap_or_else(|| "cdn.app".to_string()),
            rate: self.rate.unwrap_or(1.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cloudflare_fault_builder() {
        let injector = CloudflareFaultInjector::builder()
            .error_code(CloudflareErrorCode::WafChallenge403)
            .zone_name("video.cdn.com")
            .rate(0.5)
            .build();

        assert_eq!(injector.config.zone_name, "video.cdn.com");
        let target = Target::System;
        assert!(injector.inject(&target).await.is_err());
    }
}
