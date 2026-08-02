use crate::{ChaosError, InjectionHandle, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

const JOURNAL_VERSION: u32 = 1;

#[derive(Debug, Default, Serialize, Deserialize)]
struct JournalData {
    version: u32,
    #[serde(default)]
    active: Vec<InjectionHandle>,
}

pub struct RecoveryJournal {
    path: PathBuf,
    write_lock: Mutex<()>,
}

impl RecoveryJournal {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            write_lock: Mutex::new(()),
        }
    }

    pub fn default_path() -> PathBuf {
        if let Some(path) = std::env::var_os("CHAOS_JOURNAL_PATH") {
            return PathBuf::from(path);
        }

        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(".chaos-engineering")
            .join("recovery.json")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn entries(&self) -> Result<Vec<InjectionHandle>> {
        let _guard = self.write_lock.lock().await;
        Ok(self.load().await?.active)
    }

    pub async fn record(&self, handle: &InjectionHandle) -> Result<()> {
        let _guard = self.write_lock.lock().await;
        let mut data = self.load().await?;
        if let Some(existing) = data.active.iter_mut().find(|entry| entry.id == handle.id) {
            *existing = handle.clone();
        } else {
            data.active.push(handle.clone());
        }
        self.store(&data).await
    }

    pub async fn remove(&self, handle_id: &str) -> Result<bool> {
        let _guard = self.write_lock.lock().await;
        let mut data = self.load().await?;
        let previous_len = data.active.len();
        data.active.retain(|entry| entry.id != handle_id);
        let removed = data.active.len() != previous_len;
        if removed {
            self.store(&data).await?;
        }
        Ok(removed)
    }

    pub async fn clear(&self) -> Result<()> {
        let _guard = self.write_lock.lock().await;
        self.store(&JournalData {
            version: JOURNAL_VERSION,
            active: Vec::new(),
        })
        .await
    }

    async fn load(&self) -> Result<JournalData> {
        let backup_path = self.backup_path();
        let bytes = match tokio::fs::read(&self.path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match tokio::fs::read(&backup_path).await {
                    Ok(bytes) => bytes,
                    Err(backup_error) if backup_error.kind() == std::io::ErrorKind::NotFound => {
                        return Ok(JournalData {
                            version: JOURNAL_VERSION,
                            active: Vec::new(),
                        });
                    }
                    Err(backup_error) => return Err(backup_error.into()),
                }
            }
            Err(error) => return Err(error.into()),
        };

        let data: JournalData = serde_json::from_slice(&bytes)?;
        if data.version != JOURNAL_VERSION {
            return Err(ChaosError::InvalidConfig(format!(
                "Unsupported recovery journal version {}",
                data.version
            )));
        }
        Ok(data)
    }

    async fn store(&self, data: &JournalData) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let next_path = self.next_path();
        let backup_path = self.backup_path();
        let bytes = serde_json::to_vec_pretty(data)?;
        tokio::fs::write(&next_path, bytes).await?;

        remove_if_exists(&backup_path).await?;
        if tokio::fs::metadata(&self.path).await.is_ok() {
            tokio::fs::rename(&self.path, &backup_path).await?;
        }
        tokio::fs::rename(&next_path, &self.path).await?;
        remove_if_exists(&backup_path).await?;
        Ok(())
    }

    fn next_path(&self) -> PathBuf {
        self.path.with_extension("json.next")
    }

    fn backup_path(&self) -> PathBuf {
        self.path.with_extension("json.bak")
    }
}

async fn remove_if_exists(path: &Path) -> Result<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Target;

    #[tokio::test]
    async fn persists_and_removes_recovery_entries() {
        let directory =
            std::env::temp_dir().join(format!("chaos-journal-{}", uuid::Uuid::new_v4()));
        let path = directory.join("recovery.json");
        let journal = RecoveryJournal::new(&path);
        let handle = InjectionHandle::new("test", Target::System, serde_json::json!({}));

        journal.record(&handle).await.unwrap();
        assert_eq!(journal.entries().await.unwrap().len(), 1);

        let reopened = RecoveryJournal::new(&path);
        assert_eq!(reopened.entries().await.unwrap()[0].id, handle.id);
        assert!(reopened.remove(&handle.id).await.unwrap());
        assert!(reopened.entries().await.unwrap().is_empty());

        tokio::fs::remove_dir_all(directory).await.unwrap();
    }
}
