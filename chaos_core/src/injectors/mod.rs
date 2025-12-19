pub mod cpu;
pub mod disk;
pub mod memory;
pub mod network;
pub mod process;

use crate::{error::Result, handle::InjectionHandle, target::Target};
use async_trait::async_trait;
use std::sync::Arc;

pub use cpu::*;
pub use disk::*;
pub use memory::*;
pub use network::*;
pub use process::*;

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

        // Register default injectors
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

        registry
    }
}
