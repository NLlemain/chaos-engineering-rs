use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AzureServiceFault {
    ArmRateLimit429 { retry_after_secs: u32 },
    CosmosDbRuExhaustion,
    KeyVaultAccessDenied,
    BlobStorageServerBusy503,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureFaultConfig {
    pub service_fault: AzureServiceFault,
    pub resource_group: String,
    pub rate: f64,
}

impl Default for AzureFaultConfig {
    fn default() -> Self {
        Self {
            service_fault: AzureServiceFault::ArmRateLimit429 { retry_after_secs: 30 },
            resource_group: "rg-prod".to_string(),
            rate: 1.0,
        }
    }
}

pub struct AzureFaultInjector {
    config: AzureFaultConfig,
}

impl Default for AzureFaultInjector {
    fn default() -> Self {
        Self::new(AzureFaultConfig::default())
    }
}

impl AzureFaultInjector {
    pub fn new(config: AzureFaultConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> AzureFaultBuilder {
        AzureFaultBuilder::default()
    }
}

#[async_trait]
impl Injector for AzureFaultInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        info!(
            "Injecting Azure Fault {:?} for resource group '{}' (rate: {})",
            self.config.service_fault, self.config.resource_group, self.config.rate
        );

        let metadata = serde_json::json!({
            "service_fault": format!("{:?}", self.config.service_fault),
            "resource_group": self.config.resource_group,
            "rate": self.config.rate,
        });

        Ok(InjectionHandle::new("azure_fault", target.clone(), metadata))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!(
            "Removing Azure Fault injection for resource group '{}'",
            self.config.resource_group
        );
        Ok(())
    }

    fn name(&self) -> &str {
        "azure_fault"
    }
}

#[derive(Default)]
pub struct AzureFaultBuilder {
    service_fault: Option<AzureServiceFault>,
    resource_group: Option<String>,
    rate: Option<f64>,
}

impl AzureFaultBuilder {
    pub fn service_fault(mut self, fault: AzureServiceFault) -> Self {
        self.service_fault = Some(fault);
        self
    }

    pub fn resource_group(mut self, rg: impl Into<String>) -> Self {
        self.resource_group = Some(rg.into());
        self
    }

    pub fn rate(mut self, rate: f64) -> Self {
        self.rate = Some(rate.clamp(0.0, 1.0));
        self
    }

    pub fn build(self) -> AzureFaultInjector {
        AzureFaultInjector::new(AzureFaultConfig {
            service_fault: self
                .service_fault
                .unwrap_or(AzureServiceFault::CosmosDbRuExhaustion),
            resource_group: self
                .resource_group
                .unwrap_or_else(|| "default-rg".to_string()),
            rate: self.rate.unwrap_or(1.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_azure_fault_builder() {
        let injector = AzureFaultInjector::builder()
            .service_fault(AzureServiceFault::KeyVaultAccessDenied)
            .resource_group("rg-core")
            .rate(0.8)
            .build();

        assert_eq!(injector.config.resource_group, "rg-core");
        let target = Target::System;
        let handle = injector.inject(&target).await.unwrap();
        assert_eq!(handle.injector_name, "azure_fault");
        injector.remove(handle).await.unwrap();
    }
}
