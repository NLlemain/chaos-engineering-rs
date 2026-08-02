use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskFillConfig {
    pub target_path: PathBuf,
    pub fill_size_bytes: u64,
    pub block_size_bytes: usize,
}

impl Default for DiskFillConfig {
    fn default() -> Self {
        Self {
            target_path: std::env::temp_dir(),
            fill_size_bytes: 100 * 1024 * 1024, // 100MB
            block_size_bytes: 1024 * 1024,      // 1MB
        }
    }
}

pub struct DiskFillInjector {
    config: DiskFillConfig,
}

impl Default for DiskFillInjector {
    fn default() -> Self {
        Self::new(DiskFillConfig::default())
    }
}

impl DiskFillInjector {
    pub fn new(config: DiskFillConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> DiskFillBuilder {
        DiskFillBuilder::default()
    }
}

#[async_trait]
impl Injector for DiskFillInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        let ballast_path = self.config.target_path.join(format!(
            "chaos_ballast_{}.bin",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ));

        info!(
            "Injecting Disk Fill (ENOSPC test): writing {} bytes to {:?}",
            self.config.fill_size_bytes, ballast_path
        );

        let fill_size = self.config.fill_size_bytes;
        let block_size = self.config.block_size_bytes;
        let path_clone = ballast_path.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut file = File::create(&path_clone).map_err(|e| {
                ChaosError::InjectionFailed(format!("Failed to create ballast file: {}", e))
            })?;
            let buffer = vec![0u8; block_size];
            let mut written = 0u64;

            while written < fill_size {
                let to_write = (fill_size - written).min(block_size as u64) as usize;
                file.write_all(&buffer[..to_write]).map_err(|e| {
                    ChaosError::InjectionFailed(format!("Write error on ballast file: {}", e))
                })?;
                written += to_write as u64;
            }
            file.flush().ok();
            Ok(())
        })
        .await
        .map_err(|e| ChaosError::InjectionFailed(format!("Disk fill task join error: {}", e)))??;

        let metadata = serde_json::json!({
            "ballast_path": ballast_path.to_string_lossy(),
            "fill_size_bytes": self.config.fill_size_bytes,
        });

        Ok(InjectionHandle::new("disk_fill", target.clone(), metadata))
    }

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        if let Some(path_str) = handle.metadata.get("ballast_path").and_then(|v| v.as_str()) {
            info!("Removing Disk Fill ballast file: {}", path_str);
            let path = PathBuf::from(path_str);
            if path.exists() {
                fs::remove_file(path).map_err(|e| {
                    ChaosError::CleanupFailed(format!("Failed to remove ballast file: {}", e))
                })?;
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "disk_fill"
    }

    async fn validate(&self) -> Result<()> {
        if self.config.fill_size_bytes == 0 || self.config.block_size_bytes == 0 {
            return Err(ChaosError::InvalidConfig(
                "Disk fill size and block size must be greater than zero".to_string(),
            ));
        }
        if !self.config.target_path.is_dir() {
            return Err(ChaosError::InvalidConfig(format!(
                "Disk fill target directory does not exist: {}",
                self.config.target_path.display()
            )));
        }
        Ok(())
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["Write access to the target directory".to_string()]
    }
}

#[derive(Default)]
pub struct DiskFillBuilder {
    target_path: Option<PathBuf>,
    fill_size_bytes: Option<u64>,
    block_size_bytes: Option<usize>,
}

impl DiskFillBuilder {
    pub fn target_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.target_path = Some(path.into());
        self
    }

    pub fn fill_size_bytes(mut self, size: u64) -> Self {
        self.fill_size_bytes = Some(size);
        self
    }

    pub fn build(self) -> DiskFillInjector {
        DiskFillInjector::new(DiskFillConfig {
            target_path: self.target_path.unwrap_or_else(std::env::temp_dir),
            fill_size_bytes: self.fill_size_bytes.unwrap_or(10 * 1024 * 1024),
            block_size_bytes: self.block_size_bytes.unwrap_or(1024 * 1024),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_disk_fill_lifecycle() {
        let injector = DiskFillInjector::builder()
            .fill_size_bytes(1024 * 1024) // 1MB
            .build();

        let target = Target::System;
        let handle = injector.inject(&target).await.unwrap();
        assert_eq!(handle.injector_name, "disk_fill");

        let path_str = handle.metadata["ballast_path"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(std::path::Path::new(&path_str).exists());

        injector.remove(handle).await.unwrap();
        assert!(!std::path::Path::new(&path_str).exists());
    }

    #[tokio::test]
    async fn zero_byte_fill_is_rejected_as_no_effect() {
        let injector = DiskFillInjector::new(DiskFillConfig {
            target_path: std::env::temp_dir(),
            fill_size_bytes: 0,
            block_size_bytes: 1024,
        });
        assert!(injector.validate().await.is_err());
    }
}
