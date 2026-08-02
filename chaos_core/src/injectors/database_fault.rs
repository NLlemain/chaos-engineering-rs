use crate::{
    error::{ChaosError, Result},
    handle::InjectionHandle,
    injectors::{Injector, InjectorStatus},
    target::Target,
};
use async_trait::async_trait;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{sync::Mutex, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalDatabaseEngine {
    DuckDb,
    Sqlite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DatabaseFaultMode {
    Unavailable,
    ReadOnly,
    Lock,
    IoPressure {
        bytes_per_cycle: usize,
        cycle_delay: Duration,
    },
    InodePressure {
        files: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseFaultConfig {
    pub engine: LocalDatabaseEngine,
    pub mode: DatabaseFaultMode,
}

impl Default for DatabaseFaultConfig {
    fn default() -> Self {
        Self {
            engine: LocalDatabaseEngine::DuckDb,
            mode: DatabaseFaultMode::Unavailable,
        }
    }
}

impl DatabaseFaultConfig {
    pub fn validate(&self) -> Result<()> {
        match self.mode {
            DatabaseFaultMode::IoPressure {
                bytes_per_cycle,
                cycle_delay,
            } => {
                if bytes_per_cycle == 0 || bytes_per_cycle > 64 * 1024 * 1024 {
                    return Err(ChaosError::InvalidConfig(
                        "Database I/O pressure cycle must be between 1 byte and 64 MiB".to_string(),
                    ));
                }
                if cycle_delay.is_zero() {
                    return Err(ChaosError::InvalidConfig(
                        "Database I/O pressure delay must be greater than zero".to_string(),
                    ));
                }
            }
            DatabaseFaultMode::InodePressure { files } if files == 0 || files > 1_000_000 => {
                return Err(ChaosError::InvalidConfig(
                    "Inode pressure file count must be between 1 and 1,000,000".to_string(),
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

enum ActiveDatabaseFault {
    Lock(File),
    IoPressure {
        cancellation: CancellationToken,
        task: JoinHandle<std::io::Result<()>>,
        bytes_written: Arc<AtomicU64>,
    },
}

pub struct DatabaseFaultInjector {
    config: DatabaseFaultConfig,
    active: Arc<Mutex<HashMap<String, ActiveDatabaseFault>>>,
}

impl DatabaseFaultInjector {
    pub fn new(config: DatabaseFaultConfig) -> Self {
        Self {
            config,
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn pressure_bytes(&self, handle_id: &str) -> Option<u64> {
        self.active.lock().await.get(handle_id).and_then(|active| {
            if let ActiveDatabaseFault::IoPressure { bytes_written, .. } = active {
                Some(bytes_written.load(Ordering::Relaxed))
            } else {
                None
            }
        })
    }

    async fn inject_unavailable(&self, path: &Path, target: &Target) -> Result<InjectionHandle> {
        let backup = owned_sibling(path, "database-backup")?;
        tokio::fs::rename(path, &backup).await?;
        if let Err(error) = tokio::fs::create_dir(path).await {
            let _ = tokio::fs::rename(&backup, path).await;
            return Err(error.into());
        }
        Ok(InjectionHandle::new(
            self.name(),
            target.clone(),
            serde_json::json!({
                "mode": "unavailable",
                "path": path,
                "backup": backup,
                "engine": self.config.engine,
            }),
        ))
    }

    async fn inject_read_only(&self, path: &Path, target: &Target) -> Result<InjectionHandle> {
        let metadata = tokio::fs::metadata(path).await?;
        let permission_state = permission_state(&metadata);
        set_read_only(path, &metadata).await?;
        Ok(InjectionHandle::new(
            self.name(),
            target.clone(),
            serde_json::json!({
                "mode": "read_only",
                "path": path,
                "permission_state": permission_state,
                "engine": self.config.engine,
            }),
        ))
    }

    async fn inject_lock(&self, path: &Path, target: &Target) -> Result<InjectionHandle> {
        let lock_path = path.to_path_buf();
        let file = tokio::task::spawn_blocking(move || -> Result<File> {
            let file = OpenOptions::new().read(true).write(true).open(&lock_path)?;
            file.try_lock_exclusive().map_err(|error| {
                ChaosError::InjectionFailed(format!("Could not lock database: {}", error))
            })?;
            Ok(file)
        })
        .await
        .map_err(|error| ChaosError::InjectionFailed(error.to_string()))??;
        let handle = InjectionHandle::new(
            self.name(),
            target.clone(),
            serde_json::json!({
                "mode": "lock",
                "path": path,
                "engine": self.config.engine,
            }),
        );
        self.active
            .lock()
            .await
            .insert(handle.id.clone(), ActiveDatabaseFault::Lock(file));
        Ok(handle)
    }

    async fn inject_io_pressure(
        &self,
        path: &Path,
        target: &Target,
        bytes_per_cycle: usize,
        cycle_delay: Duration,
    ) -> Result<InjectionHandle> {
        let pressure_path = owned_sibling(path, "io-pressure")?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&pressure_path)?;
        let cancellation = CancellationToken::new();
        let shutdown = cancellation.clone();
        let bytes_written = Arc::new(AtomicU64::new(0));
        let task_bytes = bytes_written.clone();
        let task = tokio::task::spawn_blocking(move || {
            let buffer = vec![0x5au8; bytes_per_cycle];
            let cycles = ((64 * 1024 * 1024) / bytes_per_cycle).clamp(1, 16);
            let working_set_size = (bytes_per_cycle * cycles) as u64;
            let mut offset = 0;
            while !shutdown.is_cancelled() {
                file.seek(SeekFrom::Start(offset))?;
                file.write_all(&buffer)?;
                file.flush()?;
                task_bytes.fetch_add(buffer.len() as u64, Ordering::Relaxed);
                offset = (offset + buffer.len() as u64) % working_set_size;
                if wait_for_cycle_or_cancelled(&shutdown, cycle_delay) {
                    break;
                }
            }
            Ok(())
        });
        let handle = InjectionHandle::new(
            self.name(),
            target.clone(),
            serde_json::json!({
                "mode": "io_pressure",
                "path": path,
                "pressure_path": pressure_path,
                "engine": self.config.engine,
            }),
        );
        self.active.lock().await.insert(
            handle.id.clone(),
            ActiveDatabaseFault::IoPressure {
                cancellation,
                task,
                bytes_written,
            },
        );
        Ok(handle)
    }

    async fn inject_inode_pressure(
        &self,
        path: &Path,
        target: &Target,
        files: usize,
    ) -> Result<InjectionHandle> {
        let directory = owned_sibling(path, "inode-pressure")?;
        tokio::fs::create_dir(&directory).await?;
        for index in 0..files {
            if let Err(error) =
                tokio::fs::write(directory.join(format!("inode-{index:08}")), []).await
            {
                let _ = tokio::fs::remove_dir_all(&directory).await;
                return Err(error.into());
            }
        }
        Ok(InjectionHandle::new(
            self.name(),
            target.clone(),
            serde_json::json!({
                "mode": "inode_pressure",
                "path": path,
                "directory": directory,
                "files_created": files,
                "engine": self.config.engine,
            }),
        ))
    }
}

fn wait_for_cycle_or_cancelled(shutdown: &CancellationToken, delay: Duration) -> bool {
    let started = Instant::now();
    loop {
        if shutdown.is_cancelled() {
            return true;
        }
        let Some(remaining) = delay.checked_sub(started.elapsed()) else {
            return false;
        };
        std::thread::sleep(remaining.min(Duration::from_millis(10)));
    }
}

impl Default for DatabaseFaultInjector {
    fn default() -> Self {
        Self::new(DatabaseFaultConfig::default())
    }
}

#[async_trait]
impl Injector for DatabaseFaultInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        self.config.validate()?;
        let Target::File { path } = target else {
            return Err(ChaosError::InvalidConfig(
                "database_fault requires a file target".to_string(),
            ));
        };
        if !path.is_file() {
            return Err(ChaosError::TargetNotFound(path.display().to_string()));
        }
        match self.config.mode {
            DatabaseFaultMode::Unavailable => self.inject_unavailable(path, target).await,
            DatabaseFaultMode::ReadOnly => self.inject_read_only(path, target).await,
            DatabaseFaultMode::Lock => self.inject_lock(path, target).await,
            DatabaseFaultMode::IoPressure {
                bytes_per_cycle,
                cycle_delay,
            } => {
                self.inject_io_pressure(path, target, bytes_per_cycle, cycle_delay)
                    .await
            }
            DatabaseFaultMode::InodePressure { files } => {
                self.inject_inode_pressure(path, target, files).await
            }
        }
    }

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        if let Some(active) = self.active.lock().await.remove(&handle.id) {
            match active {
                ActiveDatabaseFault::Lock(file) => {
                    FileExt::unlock(&file).map_err(|error| {
                        ChaosError::CleanupFailed(format!("Could not unlock database: {}", error))
                    })?;
                }
                ActiveDatabaseFault::IoPressure {
                    cancellation, task, ..
                } => {
                    cancellation.cancel();
                    task.await
                        .map_err(|error| ChaosError::CleanupFailed(error.to_string()))?
                        .map_err(|error| ChaosError::CleanupFailed(error.to_string()))?;
                }
            }
        }

        let mode = metadata_string(&handle, "mode")?;
        let path = metadata_path(&handle, "path")?;
        match mode {
            "unavailable" => restore_unavailable(&handle, &path).await?,
            "read_only" => {
                let state = handle.metadata.get("permission_state").ok_or_else(|| {
                    ChaosError::CleanupFailed("Missing prior permission state".to_string())
                })?;
                restore_permissions(&path, state).await?;
            }
            "io_pressure" => {
                remove_owned_file(&path, metadata_path(&handle, "pressure_path")?).await?;
            }
            "inode_pressure" => {
                remove_owned_directory(&path, metadata_path(&handle, "directory")?).await?;
            }
            "lock" => {}
            value => {
                return Err(ChaosError::CleanupFailed(format!(
                    "Unknown database recovery mode '{}'",
                    value
                )))
            }
        }
        info!("Restored local database fault {}", handle.id);
        Ok(())
    }

    fn name(&self) -> &str {
        "database_fault"
    }

    fn status(&self) -> InjectorStatus {
        match self.config.mode {
            DatabaseFaultMode::Unavailable | DatabaseFaultMode::ReadOnly => InjectorStatus::Stable,
            DatabaseFaultMode::Lock
            | DatabaseFaultMode::IoPressure { .. }
            | DatabaseFaultMode::InodePressure { .. } => InjectorStatus::Experimental,
        }
    }

    async fn validate(&self) -> Result<()> {
        self.config.validate()
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["Read/write access to the database directory".to_string()]
    }
}

async fn restore_unavailable(handle: &InjectionHandle, path: &Path) -> Result<()> {
    let backup = metadata_path(handle, "backup")?;
    let metadata = tokio::fs::metadata(path).await.map_err(|error| {
        ChaosError::CleanupFailed(format!("Database placeholder missing: {}", error))
    })?;
    if !metadata.is_dir() {
        return Err(ChaosError::CleanupFailed(format!(
            "Refusing to replace non-directory path {}",
            path.display()
        )));
    }
    let mut entries = tokio::fs::read_dir(path).await?;
    if entries.next_entry().await?.is_some() {
        return Err(ChaosError::CleanupFailed(format!(
            "Refusing to remove non-empty placeholder {}",
            path.display()
        )));
    }
    tokio::fs::remove_dir(path).await?;
    tokio::fs::rename(backup, path).await?;
    Ok(())
}

#[cfg(unix)]
fn permission_state(metadata: &std::fs::Metadata) -> serde_json::Value {
    use std::os::unix::fs::PermissionsExt;
    serde_json::json!({ "mode": metadata.permissions().mode() })
}

#[cfg(windows)]
fn permission_state(metadata: &std::fs::Metadata) -> serde_json::Value {
    serde_json::json!({ "readonly": metadata.permissions().readonly() })
}

#[cfg(unix)]
async fn set_read_only(path: &Path, metadata: &std::fs::Metadata) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = metadata.permissions().mode() & !0o222;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).await?;
    Ok(())
}

#[cfg(windows)]
async fn set_read_only(path: &Path, metadata: &std::fs::Metadata) -> Result<()> {
    let mut permissions = metadata.permissions();
    permissions.set_readonly(true);
    tokio::fs::set_permissions(path, permissions).await?;
    Ok(())
}

#[cfg(unix)]
async fn restore_permissions(path: &Path, state: &serde_json::Value) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = state
        .get("mode")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            ChaosError::CleanupFailed("Missing Unix database permission mode".to_string())
        })?;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode as u32)).await?;
    Ok(())
}

#[cfg(windows)]
async fn restore_permissions(path: &Path, state: &serde_json::Value) -> Result<()> {
    let readonly = state
        .get("readonly")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| {
            ChaosError::CleanupFailed("Missing Windows database permission state".to_string())
        })?;
    let mut permissions = tokio::fs::metadata(path).await?.permissions();
    permissions.set_readonly(readonly);
    tokio::fs::set_permissions(path, permissions).await?;
    Ok(())
}

fn owned_sibling(path: &Path, kind: &str) -> Result<PathBuf> {
    let parent = path.parent().ok_or_else(|| {
        ChaosError::InvalidConfig("Database path must have a parent directory".to_string())
    })?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ChaosError::InvalidConfig("Database filename is invalid".to_string()))?;
    Ok(parent.join(format!(".chaos-{}-{}-{}", kind, name, uuid::Uuid::new_v4())))
}

fn metadata_string<'a>(handle: &'a InjectionHandle, key: &str) -> Result<&'a str> {
    handle
        .metadata
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ChaosError::CleanupFailed(format!("Missing database metadata '{}'", key)))
}

fn metadata_path(handle: &InjectionHandle, key: &str) -> Result<PathBuf> {
    metadata_string(handle, key).map(PathBuf::from)
}

fn verify_owned_path(database: &Path, owned: &Path, marker: &str) -> Result<()> {
    if owned.parent() != database.parent()
        || !owned
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(marker))
    {
        return Err(ChaosError::CleanupFailed(format!(
            "Refusing to remove unrecognized chaos path {}",
            owned.display()
        )));
    }
    Ok(())
}

async fn remove_owned_file(database: &Path, owned: PathBuf) -> Result<()> {
    verify_owned_path(database, &owned, ".chaos-io-pressure-")?;
    match tokio::fs::remove_file(owned).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

async fn remove_owned_directory(database: &Path, owned: PathBuf) -> Result<()> {
    verify_owned_path(database, &owned, ".chaos-inode-pressure-")?;
    match tokio::fs::remove_dir_all(owned).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_database() -> (PathBuf, PathBuf) {
        let directory = std::env::temp_dir().join(format!("chaos-db-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let database = directory.join("test.duckdb");
        std::fs::write(&database, b"real database bytes").unwrap();
        (directory, database)
    }

    #[tokio::test]
    async fn unavailable_database_is_disrupted_and_restored_byte_for_byte() {
        let (directory, database) = temporary_database();
        let injector = DatabaseFaultInjector::new(DatabaseFaultConfig::default());
        let handle = injector.inject(&Target::file(&database)).await.unwrap();

        assert!(database.is_dir());
        assert!(!database.is_file());
        assert!(std::fs::read(&database).is_err());
        injector.remove(handle).await.unwrap();
        assert_eq!(std::fs::read(&database).unwrap(), b"real database bytes");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn io_pressure_writes_bytes_and_cleans_its_file() {
        let (directory, database) = temporary_database();
        let injector = DatabaseFaultInjector::new(DatabaseFaultConfig {
            engine: LocalDatabaseEngine::DuckDb,
            mode: DatabaseFaultMode::IoPressure {
                bytes_per_cycle: 4096,
                cycle_delay: Duration::from_secs(3600),
            },
        });
        let handle = injector.inject(&Target::file(&database)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(injector.pressure_bytes(&handle.id).await.unwrap() >= 4096);
        let pressure = metadata_path(&handle, "pressure_path").unwrap();
        tokio::time::timeout(Duration::from_secs(1), injector.remove(handle))
            .await
            .expect("I/O pressure cancellation timed out")
            .unwrap();
        assert!(!pressure.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn inode_pressure_creates_exact_count_and_cleans_up() {
        let (directory, database) = temporary_database();
        let injector = DatabaseFaultInjector::new(DatabaseFaultConfig {
            engine: LocalDatabaseEngine::Sqlite,
            mode: DatabaseFaultMode::InodePressure { files: 12 },
        });
        let handle = injector.inject(&Target::file(&database)).await.unwrap();
        let pressure = metadata_path(&handle, "directory").unwrap();
        assert_eq!(std::fs::read_dir(&pressure).unwrap().count(), 12);
        injector.remove(handle).await.unwrap();
        assert!(!pressure.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
