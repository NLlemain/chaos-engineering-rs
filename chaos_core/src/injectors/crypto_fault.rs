use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CryptoFaultType {
    CertExpired,
    UntrustedCa,
    OcspOffline,
    SignatureCorrupt,
    EntropyStarvation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoFaultConfig {
    pub fault_type: CryptoFaultType,
    pub target_cert_domain: String,
}

impl Default for CryptoFaultConfig {
    fn default() -> Self {
        Self {
            fault_type: CryptoFaultType::CertExpired,
            target_cert_domain: "localhost".to_string(),
        }
    }
}

pub struct CryptoFaultInjector {
    config: CryptoFaultConfig,
}

impl Default for CryptoFaultInjector {
    fn default() -> Self {
        Self::new(CryptoFaultConfig::default())
    }
}

impl CryptoFaultInjector {
    pub fn new(config: CryptoFaultConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> CryptoFaultBuilder {
        CryptoFaultBuilder::default()
    }
}

#[async_trait]
impl Injector for CryptoFaultInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        info!(
            "Injecting Cryptographic & Security Fault {:?} for domain '{}' on target {:?}",
            self.config.fault_type, self.config.target_cert_domain, target
        );

        let metadata = serde_json::json!({
            "fault_type": format!("{:?}", self.config.fault_type),
            "domain": self.config.target_cert_domain,
        });

        Ok(InjectionHandle::new(
            "crypto_fault",
            target.clone(),
            metadata,
        ))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!(
            "Removing Cryptographic Fault injection for domain '{}'",
            self.config.target_cert_domain
        );
        Ok(())
    }

    fn name(&self) -> &str {
        "crypto_fault"
    }
}

#[derive(Default)]
pub struct CryptoFaultBuilder {
    fault_type: Option<CryptoFaultType>,
    target_cert_domain: Option<String>,
}

impl CryptoFaultBuilder {
    pub fn fault_type(mut self, fault: CryptoFaultType) -> Self {
        self.fault_type = Some(fault);
        self
    }

    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.target_cert_domain = Some(domain.into());
        self
    }

    pub fn build(self) -> CryptoFaultInjector {
        CryptoFaultInjector::new(CryptoFaultConfig {
            fault_type: self.fault_type.unwrap_or(CryptoFaultType::CertExpired),
            target_cert_domain: self
                .target_cert_domain
                .unwrap_or_else(|| "api.internal".to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_crypto_fault_builder() {
        let injector = CryptoFaultInjector::builder()
            .fault_type(CryptoFaultType::EntropyStarvation)
            .domain("auth.service")
            .build();

        assert_eq!(injector.config.target_cert_domain, "auth.service");
        let target = Target::System;
        let handle = injector.inject(&target).await.unwrap();
        assert_eq!(handle.injector_name, "crypto_fault");
        injector.remove(handle).await.unwrap();
    }
}
