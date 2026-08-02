pub mod aws_fault;
pub mod azure_fault;
pub mod clock_skew;
pub mod cloudflare_fault;
pub mod cpu;
pub mod crypto_fault;
pub mod disk;
pub mod disk_fill;
pub mod dns;
pub mod fd_exhaustion;
pub mod http_fault;
pub mod media_streaming_fault;
pub mod memory;
pub mod network;
pub mod nginx_fault;
pub mod process;
pub mod process_freeze;
pub mod socket_corrupt;

use crate::{error::Result, handle::InjectionHandle, target::Target};
use async_trait::async_trait;
use std::sync::Arc;

pub use aws_fault::*;
pub use azure_fault::*;
pub use clock_skew::*;
pub use cloudflare_fault::*;
pub use cpu::*;
pub use crypto_fault::*;
pub use disk::*;
pub use disk_fill::*;
pub use dns::*;
pub use fd_exhaustion::*;
pub use http_fault::*;
pub use media_streaming_fault::*;
pub use memory::*;
pub use network::*;
pub use nginx_fault::*;
pub use process::*;
pub use process_freeze::*;
pub use socket_corrupt::*;

/// Core trait for all fault injectors
#[async_trait]
pub trait Injector: Send + Sync {
    /// Apply the fault injection to the target
    async fn inject(&self, target: &Target) -> Result<InjectionHandle>;

    /// Remove the fault injection
    async fn remove(&self, handle: InjectionHandle) -> Result<()>;

    /// Get the name of this injector
    fn name(&self) -> &str;

    /// Validate the injector can run on this system
    async fn validate(&self) -> Result<()> {
        Ok(())
    }

    /// Get required system capabilities
    fn required_capabilities(&self) -> Vec<String> {
        vec![]
    }
}

pub type DynInjector = Arc<dyn Injector>;

#[derive(Default)]
pub struct InjectorRegistry {
    injectors: std::collections::HashMap<String, DynInjector>,
}

impl InjectorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: impl Into<String>, injector: DynInjector) {
        self.injectors.insert(name.into(), injector);
    }

    pub fn get(&self, name: &str) -> Option<&DynInjector> {
        self.injectors.get(name)
    }

    pub fn list(&self) -> Vec<String> {
        self.injectors.keys().cloned().collect()
    }

    pub fn with_defaults() -> Self {
        let mut registry = Self::new();

        // Register default baseline injectors
        registry.register(
            "network_latency",
            Arc::new(NetworkLatencyInjector::default()),
        );
        registry.register("packet_loss", Arc::new(PacketLossInjector::default()));
        registry.register("tcp_reset", Arc::new(TcpResetInjector::default()));
        registry.register("cpu_starvation", Arc::new(CpuStarvationInjector::default()));
        registry.register("disk_slow", Arc::new(DiskSlowInjector::default()));
        registry.register(
            "memory_pressure",
            Arc::new(MemoryPressureInjector::default()),
        );
        registry.register("process_kill", Arc::new(ProcessKillInjector::default()));

        // Register expanded niche & cloud injectors (20 total)
        registry.register("dns_fault", Arc::new(DnsFaultInjector::default()));
        registry.register("fd_exhaustion", Arc::new(FdExhaustionInjector::default()));
        registry.register("process_freeze", Arc::new(ProcessFreezeInjector::default()));
        registry.register("clock_skew", Arc::new(ClockSkewInjector::default()));
        registry.register("disk_fill", Arc::new(DiskFillInjector::default()));
        registry.register("socket_corrupt", Arc::new(SocketCorruptInjector::default()));
        registry.register("http_fault", Arc::new(HttpFaultInjector::default()));
        registry.register("nginx_fault", Arc::new(NginxFaultInjector::default()));
        registry.register("aws_fault", Arc::new(AwsFaultInjector::default()));
        registry.register("crypto_fault", Arc::new(CryptoFaultInjector::default()));
        registry.register("azure_fault", Arc::new(AzureFaultInjector::default()));
        registry.register(
            "cloudflare_fault",
            Arc::new(CloudflareFaultInjector::default()),
        );
        registry.register(
            "media_streaming_fault",
            Arc::new(MediaStreamingFaultInjector::default()),
        );

        registry
    }
}
