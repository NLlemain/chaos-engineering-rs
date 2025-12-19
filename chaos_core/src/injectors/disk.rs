use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskSlowConfig {
    pub latency: Duration,
    pub operations: Vec<DiskOperation>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiskOperation {
    Read,
    Write,
    Fsync,
    Open,
    All,
}

impl Default for DiskSlowConfig {
    fn default() -> Self {
        Self {
            latency: Duration::from_millis(100),
            operations: vec![DiskOperation::All],
        }
    }
}

pub struct DiskSlowInjector {
    #[allow(dead_code)]
    config: DiskSlowConfig,
}

impl Default for DiskSlowInjector {
    fn default() -> Self {
        Self {
            config: DiskSlowConfig::default(),
        }
    }
}

impl DiskSlowInjector {
    pub fn new(config: DiskSlowConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> DiskSlowBuilder {
        DiskSlowBuilder::default()
    }

    #[cfg(target_os = "linux")]
    async fn inject_linux(&self, target: &Target) -> Result<InjectionHandle> {
        info!(
            "Injecting disk I/O slowdown: latency={}ms",
            self.config.latency.as_millis()
        );

        // For process targets, we would inject via LD_PRELOAD
        // For simplicity, we'll use a marker file approach
        let marker_file = "/tmp/chaos_disk_slow.json";
        let config_json = serde_json::to_string(&self.config)?;
        tokio::fs::write(marker_file, config_json).await?;

        let metadata = serde_json::json!({
            "marker_file": marker_file,
            "latency_ms": self.config.latency.as_millis(),
        });

        Ok(InjectionHandle::new("disk_slow", target.clone(), metadata))
    }

    #[cfg(not(target_os = "linux"))]
    async fn inject_linux(&self, _target: &Target) -> Result<InjectionHandle> {
        Err(ChaosError::SystemError(
            "Disk slowdown injection only supported on Linux".to_string(),
        ))
    }
}

#[async_trait]
impl Injector for DiskSlowInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        self.inject_linux(target).await
    }

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        let marker_file = handle
            .metadata
            .get("marker_file")
            .and_then(|v| v.as_str())
            .unwrap_or("/tmp/chaos_disk_slow.json");

        info!("Removing disk I/O slowdown");
        tokio::fs::remove_file(marker_file).await.ok();
        Ok(())
    }

    fn name(&self) -> &str {
        "disk_slow"
    }
}

#[derive(Default)]
pub struct DiskSlowBuilder {
    latency: Option<Duration>,
    operations: Option<Vec<DiskOperation>>,
}

impl DiskSlowBuilder {
    pub fn latency(mut self, latency: Duration) -> Self {
        self.latency = Some(latency);
        self
    }

    pub fn operations(mut self, operations: Vec<DiskOperation>) -> Self {
        self.operations = Some(operations);
        self
    }

    pub fn build(self) -> DiskSlowInjector {
        DiskSlowInjector {
            config: DiskSlowConfig {
                latency: self.latency.unwrap_or(Duration::from_millis(100)),
                operations: self.operations.unwrap_or(vec![DiskOperation::All]),
            },
        }
    }
}

// Disk Failure Injector
#[derive(Debug, Clone)]
pub struct DiskFailureInjector {
    failure_rate: f64, // 0.0 - 1.0
}

impl Default for DiskFailureInjector {
    fn default() -> Self {
        Self { failure_rate: 0.1 }
    }
}

impl DiskFailureInjector {
    pub fn new(failure_rate: f64) -> Self {
        Self {
            failure_rate: failure_rate.clamp(0.0, 1.0),
        }
    }
}

#[async_trait]
impl Injector for DiskFailureInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        info!("Injecting disk write failures: rate={}", self.failure_rate);

        let metadata = serde_json::json!({
            "failure_rate": self.failure_rate,
        });

        Ok(InjectionHandle::new(
            "disk_failure",
            target.clone(),
            metadata,
        ))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!("Removing disk write failures");
        Ok(())
    }

    fn name(&self) -> &str {
        "disk_failure"
    }
}

// Disk Space Exhaustion Injector
#[derive(Debug, Clone)]
pub struct DiskSpaceInjector {
    target_usage: f64, // 0.0 - 1.0, target disk usage percentage
    path: String,
}

impl DiskSpaceInjector {
    pub fn new(path: impl Into<String>, target_usage: f64) -> Self {
        Self {
            path: path.into(),
            target_usage: target_usage.clamp(0.0, 1.0),
        }
    }

    async fn fill_disk(&self, bytes_to_fill: u64) -> Result<String> {
        let temp_file = format!("{}/chaos_disk_fill_{}.tmp", self.path, uuid::Uuid::new_v4());

        info!("Filling disk with {} bytes at {}", bytes_to_fill, temp_file);

        // Create large file
        let file = tokio::fs::File::create(&temp_file).await?;
        file.set_len(bytes_to_fill).await?;

        Ok(temp_file)
    }

    async fn calculate_bytes_to_fill(&self) -> Result<u64> {
        // Get filesystem statistics
        #[cfg(unix)]
        {
            use nix::sys::statvfs::statvfs;
            let stats = statvfs(self.path.as_str()).map_err(|e| {
                ChaosError::SystemError(format!("Failed to stat filesystem: {}", e))
            })?;

            let total_space = stats.blocks() * stats.block_size();
            let free_space = stats.blocks_free() * stats.block_size();
            let target_free = total_space as f64 * (1.0 - self.target_usage);
            let bytes_to_fill = (free_space as f64 - target_free).max(0.0) as u64;

            Ok(bytes_to_fill)
        }

        #[cfg(not(unix))]
        {
            // Simplified for non-Unix
            Ok((1024 * 1024 * 1024) as u64) // 1GB
        }
    }
}

#[async_trait]
impl Injector for DiskSpaceInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        let bytes_to_fill = self.calculate_bytes_to_fill().await?;
        let temp_file = self.fill_disk(bytes_to_fill).await?;

        let metadata = serde_json::json!({
            "temp_file": temp_file,
            "bytes_filled": bytes_to_fill,
            "target_usage": self.target_usage,
        });

        Ok(InjectionHandle::new("disk_space", target.clone(), metadata))
    }

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        let temp_file = handle
            .metadata
            .get("temp_file")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChaosError::CleanupFailed("Missing temp_file metadata".to_string()))?;

        info!("Removing disk fill file: {}", temp_file);
        tokio::fs::remove_file(temp_file)
            .await
            .map_err(|e| ChaosError::CleanupFailed(format!("Failed to remove temp file: {}", e)))?;

        Ok(())
    }

    fn name(&self) -> &str {
        "disk_space"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disk_slow_builder() {
        let injector = DiskSlowInjector::builder()
            .latency(Duration::from_millis(200))
            .operations(vec![DiskOperation::Write, DiskOperation::Fsync])
            .build();

        assert_eq!(injector.config.latency, Duration::from_millis(200));
        assert_eq!(injector.config.operations.len(), 2);
    }

    #[test]
    fn test_disk_failure_rate_clamping() {
        let injector = DiskFailureInjector::new(1.5);
        assert_eq!(injector.failure_rate, 1.0);

        let injector = DiskFailureInjector::new(-0.5);
        assert_eq!(injector.failure_rate, 0.0);
    }
}
