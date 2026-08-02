use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NginxFaultMode {
    UpstreamReset,
    GatewayTimeout { duration: Duration },
    BadGateway502,
    HeaderBufferOverflow,
    SslHandshakeDrop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NginxFaultConfig {
    pub upstream_name: String,
    pub fault_mode: NginxFaultMode,
}

impl Default for NginxFaultConfig {
    fn default() -> Self {
        Self {
            upstream_name: "backend_app".to_string(),
            fault_mode: NginxFaultMode::UpstreamReset,
        }
    }
}

pub struct NginxFaultInjector {
    config: NginxFaultConfig,
}

impl Default for NginxFaultInjector {
    fn default() -> Self {
        Self::new(NginxFaultConfig::default())
    }
}

impl NginxFaultInjector {
    pub fn new(config: NginxFaultConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> NginxFaultBuilder {
        NginxFaultBuilder::default()
    }
}

#[async_trait]
impl Injector for NginxFaultInjector {
    async fn inject(&self, _target: &Target) -> Result<InjectionHandle> {
        Err(ChaosError::InvalidConfig(
            "nginx_fault is planned and does not modify Nginx configuration yet".into(),
        ))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!(
            "Removing Nginx Fault injection on upstream '{}'",
            self.config.upstream_name
        );
        Ok(())
    }

    fn name(&self) -> &str {
        "nginx_fault"
    }

    fn status(&self) -> crate::injectors::InjectorStatus {
        crate::injectors::InjectorStatus::Planned
    }
}

#[derive(Default)]
pub struct NginxFaultBuilder {
    upstream_name: Option<String>,
    fault_mode: Option<NginxFaultMode>,
}

impl NginxFaultBuilder {
    pub fn upstream_name(mut self, name: impl Into<String>) -> Self {
        self.upstream_name = Some(name.into());
        self
    }

    pub fn fault_mode(mut self, mode: NginxFaultMode) -> Self {
        self.fault_mode = Some(mode);
        self
    }

    pub fn build(self) -> NginxFaultInjector {
        NginxFaultInjector::new(NginxFaultConfig {
            upstream_name: self.upstream_name.unwrap_or_else(|| "backend".to_string()),
            fault_mode: self.fault_mode.unwrap_or(NginxFaultMode::BadGateway502),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_nginx_fault_builder() {
        let injector = NginxFaultInjector::builder()
            .upstream_name("payment_service")
            .fault_mode(NginxFaultMode::SslHandshakeDrop)
            .build();

        assert_eq!(injector.config.upstream_name, "payment_service");
        let target = Target::System;
        assert!(injector.inject(&target).await.is_err());
    }
}
