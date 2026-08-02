use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs::File, path::PathBuf, sync::Arc};
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

struct HeldFiles {
    files: Vec<File>,
    directory: PathBuf,
}

pub struct FdExhaustionInjector {
    config: FdExhaustionConfig,
    held_files: Arc<RwLock<HashMap<String, HeldFiles>>>,
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
            held_files: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn builder() -> FdExhaustionBuilder {
        FdExhaustionBuilder::default()
    }
}

#[async_trait]
impl Injector for FdExhaustionInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        let directory = std::env::temp_dir().join(format!("chaos-fd-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory)?;
        let mut opened = Vec::new();

        for index in 0..self.config.target_open_files {
            let path = directory.join(format!("handle-{}.tmp", index));
            match File::create(path) {
                Ok(file) => opened.push(file),
                Err(error) if !opened.is_empty() => {
                    info!(
                        "File descriptor limit reached after {} handles: {}",
                        opened.len(),
                        error
                    );
                    break;
                }
                Err(error) => {
                    let _ = std::fs::remove_dir_all(&directory);
                    return Err(error.into());
                }
            }
        }

        if opened.is_empty() {
            let _ = std::fs::remove_dir_all(&directory);
            return Err(ChaosError::InjectionFailed(
                "File descriptor pressure did not open any handles".to_string(),
            ));
        }

        let total_opened = opened.len();
        let metadata = serde_json::json!({
            "target_open_files": self.config.target_open_files,
            "actual_opened_files": total_opened,
            "temporary_directory": directory,
        });
        let handle = InjectionHandle::new("fd_exhaustion", target.clone(), metadata);
        self.held_files.write().await.insert(
            handle.id.clone(),
            HeldFiles {
                files: opened,
                directory,
            },
        );
        Ok(handle)
    }

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        let held = self.held_files.write().await.remove(&handle.id);
        let directory = held
            .as_ref()
            .map(|held| held.directory.clone())
            .or_else(|| {
                handle
                    .metadata
                    .get("temporary_directory")
                    .and_then(|value| value.as_str())
                    .map(PathBuf::from)
            });
        if let Some(held) = held {
            info!("Closing {} held file handles", held.files.len());
            drop(held.files);
        }
        if let Some(directory) = directory {
            if directory.is_dir() {
                std::fs::remove_dir_all(directory)?;
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "fd_exhaustion"
    }

    async fn validate(&self) -> Result<()> {
        if self.config.target_open_files == 0 || self.config.target_open_files > 100_000 {
            return Err(ChaosError::InvalidConfig(
                "File descriptor target must be between 1 and 100000".to_string(),
            ));
        }
        if !self.config.close_on_remove {
            return Err(ChaosError::InvalidConfig(
                "close_on_remove=false is unsafe and no longer supported".to_string(),
            ));
        }
        Ok(())
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
    async fn opens_real_handles_and_removes_every_artifact() {
        let injector = FdExhaustionInjector::builder()
            .target_open_files(10)
            .build();
        let handle = injector.inject(&Target::System).await.unwrap();
        assert_eq!(handle.metadata["actual_opened_files"], 10);
        let directory = PathBuf::from(handle.metadata["temporary_directory"].as_str().unwrap());
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 10);

        injector.remove(handle).await.unwrap();
        assert!(!directory.exists());
    }

    #[tokio::test]
    async fn metadata_recovery_removes_interrupted_artifacts() {
        let injector = FdExhaustionInjector::builder().target_open_files(4).build();
        let handle = injector.inject(&Target::System).await.unwrap();
        let directory = PathBuf::from(handle.metadata["temporary_directory"].as_str().unwrap());
        drop(injector);
        assert!(directory.is_dir());

        FdExhaustionInjector::default()
            .remove(handle)
            .await
            .unwrap();
        assert!(!directory.exists());
    }
}
