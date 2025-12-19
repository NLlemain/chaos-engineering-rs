use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPressureConfig {
    pub target_usage: f64,      // 0.0 - 1.0, target memory usage percentage
    pub failure_rate: f64,      // 0.0 - 1.0, probability of allocation failure
    pub leak_rate: Option<u64>, // Bytes per second to leak
}

impl Default for MemoryPressureConfig {
    fn default() -> Self {
        Self {
            target_usage: 0.90,
            failure_rate: 0.0,
            leak_rate: None,
        }
    }
}

pub struct MemoryPressureInjector {
    config: MemoryPressureConfig,
    allocated_blocks: Arc<tokio::sync::Mutex<Vec<Vec<u8>>>>,
}

impl Default for MemoryPressureInjector {
    fn default() -> Self {
        Self {
            config: MemoryPressureConfig::default(),
            allocated_blocks: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }
}

impl MemoryPressureInjector {
    pub fn new(config: MemoryPressureConfig) -> Self {
        Self {
            config,
            allocated_blocks: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn builder() -> MemoryPressureBuilder {
        MemoryPressureBuilder::default()
    }

    async fn get_system_memory_info(&self) -> Result<(u64, u64)> {
        use sysinfo::System;
        let mut sys = System::new_all();
        sys.refresh_memory();

        let total = sys.total_memory();
        let used = sys.used_memory();

        Ok((total, used)) // sysinfo v0.30 returns bytes directly
    }

    async fn allocate_memory(&self, target_bytes: u64) -> Result<()> {
        info!("Allocating {} MB of memory", target_bytes / 1024 / 1024);

        let mut blocks = self.allocated_blocks.lock().await;

        // Allocate in chunks to avoid single huge allocation
        let chunk_size = 100 * 1024 * 1024; // 100 MB chunks
        let num_chunks = (target_bytes / chunk_size) as usize;
        let remainder = (target_bytes % chunk_size) as usize;

        for _ in 0..num_chunks {
            let block = vec![0u8; chunk_size as usize];
            blocks.push(block);
        }

        if remainder > 0 {
            let block = vec![0u8; remainder];
            blocks.push(block);
        }

        Ok(())
    }

    async fn calculate_bytes_to_allocate(&self) -> Result<u64> {
        let (total, used) = self.get_system_memory_info().await?;
        let target_used = (total as f64 * self.config.target_usage) as u64;
        let bytes_to_allocate = target_used.saturating_sub(used);

        info!(
            "Memory: total={}MB, used={}MB, target={}MB, will_allocate={}MB",
            total / 1024 / 1024,
            used / 1024 / 1024,
            target_used / 1024 / 1024,
            bytes_to_allocate / 1024 / 1024
        );

        Ok(bytes_to_allocate)
    }
}

#[async_trait]
impl Injector for MemoryPressureInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        let bytes_to_allocate = self.calculate_bytes_to_allocate().await?;

        if bytes_to_allocate > 0 {
            self.allocate_memory(bytes_to_allocate).await?;
        }

        let metadata = serde_json::json!({
            "bytes_allocated": bytes_to_allocate,
            "target_usage": self.config.target_usage,
        });

        Ok(InjectionHandle::new(
            "memory_pressure",
            target.clone(),
            metadata,
        ))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!("Releasing allocated memory");
        let mut blocks = self.allocated_blocks.lock().await;
        blocks.clear();
        Ok(())
    }

    fn name(&self) -> &str {
        "memory_pressure"
    }
}

#[derive(Default)]
pub struct MemoryPressureBuilder {
    target_usage: Option<f64>,
    failure_rate: Option<f64>,
    leak_rate: Option<u64>,
}

impl MemoryPressureBuilder {
    pub fn target_usage(mut self, target_usage: f64) -> Self {
        self.target_usage = Some(target_usage.clamp(0.0, 1.0));
        self
    }

    pub fn failure_rate(mut self, failure_rate: f64) -> Self {
        self.failure_rate = Some(failure_rate.clamp(0.0, 1.0));
        self
    }

    pub fn leak_rate(mut self, bytes_per_second: u64) -> Self {
        self.leak_rate = Some(bytes_per_second);
        self
    }

    pub fn build(self) -> MemoryPressureInjector {
        MemoryPressureInjector {
            config: MemoryPressureConfig {
                target_usage: self.target_usage.unwrap_or(0.90),
                failure_rate: self.failure_rate.unwrap_or(0.0),
                leak_rate: self.leak_rate,
            },
            allocated_blocks: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }
}

// Memory Leak Injector
#[derive(Debug, Clone)]
pub struct MemoryLeakInjector {
    leak_rate: u64, // Bytes per second
    allocated_blocks: Arc<tokio::sync::Mutex<Vec<Vec<u8>>>>,
    stop_signal: Arc<AtomicBool>,
}

impl MemoryLeakInjector {
    pub fn new(leak_rate: u64) -> Self {
        Self {
            leak_rate,
            allocated_blocks: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            stop_signal: Arc::new(AtomicBool::new(false)),
        }
    }

    async fn start_leaking(&self) -> tokio::task::JoinHandle<()> {
        let leak_rate = self.leak_rate;
        let blocks = self.allocated_blocks.clone();
        let stop_signal = self.stop_signal.clone();

        tokio::spawn(async move {
            info!("Starting memory leak: {} bytes/sec", leak_rate);

            while !stop_signal.load(Ordering::Relaxed) {
                // Allocate memory
                let block = vec![0u8; leak_rate as usize];
                blocks.lock().await.push(block);

                // Wait 1 second
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }

            info!("Memory leak stopped");
        })
    }
}

#[async_trait]
impl Injector for MemoryLeakInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        self.stop_signal.store(false, Ordering::Relaxed);
        self.start_leaking().await;

        let metadata = serde_json::json!({
            "leak_rate": self.leak_rate,
        });

        Ok(InjectionHandle::new(
            "memory_leak",
            target.clone(),
            metadata,
        ))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!("Stopping memory leak and freeing memory");
        self.stop_signal.store(true, Ordering::Relaxed);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        self.allocated_blocks.lock().await.clear();
        Ok(())
    }

    fn name(&self) -> &str {
        "memory_leak"
    }
}

// OOM Killer Injector
#[derive(Debug, Clone)]
pub struct OomKillerInjector {
    #[allow(dead_code)]
    target_pid: Option<u32>,
}

impl OomKillerInjector {
    pub fn new(target_pid: Option<u32>) -> Self {
        Self { target_pid }
    }

    #[cfg(target_os = "linux")]
    async fn trigger_oom(&self, pid: u32) -> Result<()> {
        info!("Triggering OOM condition for PID {}", pid);

        // Adjust OOM score to make process more likely to be killed
        let oom_score_adj_path = format!("/proc/{}/oom_score_adj", pid);
        tokio::fs::write(&oom_score_adj_path, "1000")
            .await
            .map_err(|e| {
                ChaosError::InjectionFailed(format!("Failed to adjust OOM score: {}", e))
            })?;

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    async fn trigger_oom(&self, _pid: u32) -> Result<()> {
        Err(ChaosError::SystemError(
            "OOM killer only supported on Linux".to_string(),
        ))
    }
}

#[async_trait]
impl Injector for OomKillerInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        let Target::Process { pid } = target else {
            return Err(ChaosError::InvalidConfig(
                "OOM killer requires Process target".to_string(),
            ));
        };

        self.trigger_oom(*pid).await?;

        let metadata = serde_json::json!({
            "pid": pid,
        });

        Ok(InjectionHandle::new("oom_killer", target.clone(), metadata))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            let pid = _handle
                .metadata
                .get("pid")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| ChaosError::CleanupFailed("Missing pid metadata".to_string()))?;

            // Reset OOM score
            let oom_score_adj_path = format!("/proc/{}/oom_score_adj", pid);
            tokio::fs::write(&oom_score_adj_path, "0").await.ok();
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "oom_killer"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["CAP_SYS_ADMIN".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_pressure_builder() {
        let injector = MemoryPressureInjector::builder()
            .target_usage(0.8)
            .failure_rate(0.1)
            .build();

        assert_eq!(injector.config.target_usage, 0.8);
        assert_eq!(injector.config.failure_rate, 0.1);
    }

    #[test]
    fn test_memory_leak_injector() {
        let injector = MemoryLeakInjector::new(1024 * 1024); // 1 MB/sec
        assert_eq!(injector.leak_rate, 1024 * 1024);
    }
}
