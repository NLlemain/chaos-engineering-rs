use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AwsServiceFault {
    ImdsMetadataBlock,
    S3SlowDown503,
    DynamoDbProvisionedThroughputExceeded,
    IamAccessDenied,
    RegionBlackhole { region: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsFaultConfig {
    pub service_fault: AwsServiceFault,
    pub rate: f64,
}

impl Default for AwsFaultConfig {
    fn default() -> Self {
        Self {
            service_fault: AwsServiceFault::ImdsMetadataBlock,
            rate: 1.0,
        }
    }
}

pub struct AwsFaultInjector {
    config: AwsFaultConfig,
}

impl Default for AwsFaultInjector {
    fn default() -> Self {
        Self::new(AwsFaultConfig::default())
    }
}

impl AwsFaultInjector {
    pub fn new(config: AwsFaultConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> AwsFaultBuilder {
        AwsFaultBuilder::default()
    }
}

#[async_trait]
impl Injector for AwsFaultInjector {
    async fn inject(&self, _target: &Target) -> Result<InjectionHandle> {
        Err(ChaosError::InvalidConfig(
            "aws_fault is planned; use AWS FIS directly until the adapter is implemented".into(),
        ))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!(
            "Removing AWS Cloud Fault injection {:?}",
            self.config.service_fault
        );
        Ok(())
    }

    fn name(&self) -> &str {
        "aws_fault"
    }

    fn status(&self) -> crate::injectors::InjectorStatus {
        crate::injectors::InjectorStatus::Planned
    }
}

#[derive(Default)]
pub struct AwsFaultBuilder {
    service_fault: Option<AwsServiceFault>,
    rate: Option<f64>,
}

impl AwsFaultBuilder {
    pub fn service_fault(mut self, fault: AwsServiceFault) -> Self {
        self.service_fault = Some(fault);
        self
    }

    pub fn rate(mut self, rate: f64) -> Self {
        self.rate = Some(rate.clamp(0.0, 1.0));
        self
    }

    pub fn build(self) -> AwsFaultInjector {
        AwsFaultInjector::new(AwsFaultConfig {
            service_fault: self.service_fault.unwrap_or(AwsServiceFault::S3SlowDown503),
            rate: self.rate.unwrap_or(1.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_aws_fault_builder() {
        let injector = AwsFaultInjector::builder()
            .service_fault(AwsServiceFault::DynamoDbProvisionedThroughputExceeded)
            .rate(0.9)
            .build();

        assert_eq!(injector.config.rate, 0.9);
        let target = Target::System;
        assert!(injector.inject(&target).await.is_err());
    }
}
