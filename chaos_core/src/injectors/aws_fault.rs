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
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        info!(
            "Injecting AWS Cloud Fault {:?} (rate: {}) for target {:?}",
            self.config.service_fault, self.config.rate, target
        );

        let metadata = serde_json::json!({
            "service_fault": format!("{:?}", self.config.service_fault),
            "rate": self.config.rate,
        });

        Ok(InjectionHandle::new("aws_fault", target.clone(), metadata))
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
            service_fault: self
                .service_fault
                .unwrap_or(AwsServiceFault::S3SlowDown503),
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
        let handle = injector.inject(&target).await.unwrap();
        assert_eq!(handle.injector_name, "aws_fault");
        injector.remove(handle).await.unwrap();
    }
}
