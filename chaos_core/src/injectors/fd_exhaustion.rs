use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FdExhaustionConfig {
    pub target_open_files: usize,
    pub close_on_remove: bool,
}

impl Default for FdExhaustionConfig {
    fn default() -> Self {
        Self {
            target_open_files: 1024,
            close_on_remove: true,
        }
    }
}

pub struct FdExhaustionInjector {
    config: FdExhaustionConfig,
    held_files: Arc<RwLock<Vec<File>>>,
}

impl Default for FdExhaustionInjector {
    fn default() -> Self {
        Self::new(FdExhaustionConfig::default())
    }
}

impl FdExhaustionInjector {
    pub fn new(config: FdExhaustionConfig) -> Self {
        Self {
            config,
            held_files: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn builder() -> FdExhaustionBuilder {
        FdExhaustionBuilder::default()
    }
}

#[async_trait]
impl Injector for FdExhaustionInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        info!(
            "Injecting FD Exhaustion: attempting to open {} file descriptors for target {:?}",
            self.config.target_open_files, target
        );

        let mut opened = Vec::new();
        let temp_dir = std::env::temp_dir();

        for i in 0..self.config.target_open_files {
            let path = temp_dir.join(format!("chaos_fd_hold_{}.tmp", i));
            match File::create(&path) {
                Ok(f) => opened.push(f),
                Err(e) => {
                    info!("FD Exhaustion limit reached after opening {} files: {}", opened.len(), e);
                    break;
                }
            }
        }

        let total_opened = opened.len();
        *self.held_files.write().await = opened;

        let metadata = serde_json::json!({
            "target_open_files": self.config.target_open_files,
            "actual_opened_files": total_opened,
        });

        Ok(InjectionHandle::new("fd_exhaustion", target.clone(), metadata))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!("Removing FD Exhaustion injection");
        let mut held = self.held_files.write().await;
        let count = held.len();
        held.clear(); // Closes all file handles automatically on drop
        info!("Closed {} file handles", count);
        Ok(())
    }

    fn name(&self) -> &str {
        "fd_exhaustion"
    }
}

#[derive(Default)]
pub struct FdExhaustionBuilder {
    target_open_files: Option<usize>,
    close_on_remove: Option<bool>,
}

impl FdExhaustionBuilder {
    pub fn target_open_files(mut self, count: usize) -> Self {
        self.target_open_files = Some(count);
        self
    }

    pub fn close_on_remove(mut self, close: bool) -> Self {
        self.close_on_remove = Some(close);
        self
    }

    pub fn build(self) -> FdExhaustionInjector {
        FdExhaustionInjector::new(FdExhaustionConfig {
            target_open_files: self.target_open_files.unwrap_or(1024),
            close_on_remove: self.close_on_remove.unwrap_or(true),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fd_exhaustion_injector() {
        let injector = FdExhaustionInjector::builder()
            .target_open_files(10)
            .build();

        let target = Target::System;
        let handle = injector.inject(&target).await.unwrap();
        assert_eq!(handle.injector_name, "fd_exhaustion");

        injector.remove(handle).await.unwrap();
    }
}
