use crate::{
    error::{ChaosError, Result},
    handle::InjectionHandle,
    injectors::{Injector, InjectorStatus},
    target::Target,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use serde_json::json;

#[cfg(windows)]
use {
    std::{
        collections::HashMap,
        fs::{File, OpenOptions},
        os::windows::fs::OpenOptionsExt,
        path::PathBuf,
        sync::Arc,
        time::Duration,
    },
    tokio::{
        net::windows::named_pipe::ServerOptions, process::Command, sync::Mutex, task::JoinHandle,
    },
    tokio_util::sync::CancellationToken,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum WindowsFaultMode {
    ServiceStop { service: String },
    FileLock,
    HandleExhaustion { count: usize },
    NamedPipeBlackhole { pipe_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowsFaultConfig {
    #[serde(flatten)]
    pub mode: WindowsFaultMode,
}

impl Default for WindowsFaultConfig {
    fn default() -> Self {
        Self {
            mode: WindowsFaultMode::FileLock,
        }
    }
}

impl WindowsFaultConfig {
    pub fn validate(&self) -> Result<()> {
        match &self.mode {
            WindowsFaultMode::ServiceStop { service } if service.trim().is_empty() => Err(
                ChaosError::InvalidConfig("Windows service name cannot be empty".to_string()),
            ),
            WindowsFaultMode::HandleExhaustion { count: 0 } => Err(ChaosError::InvalidConfig(
                "Windows handle count must be greater than zero".to_string(),
            )),
            WindowsFaultMode::HandleExhaustion { count } if *count > 100_000 => Err(
                ChaosError::InvalidConfig("Windows handle count cannot exceed 100000".to_string()),
            ),
            WindowsFaultMode::NamedPipeBlackhole { pipe_name } if pipe_name.trim().is_empty() => {
                Err(ChaosError::InvalidConfig(
                    "Windows named pipe name cannot be empty".to_string(),
                ))
            }
            _ => Ok(()),
        }
    }
}

#[cfg(windows)]
enum WindowsEffect {
    Service {
        service: String,
        was_running: bool,
    },
    FileLock {
        path: PathBuf,
        file: File,
    },
    Handles {
        directory: PathBuf,
        files: Vec<File>,
    },
    NamedPipe {
        cancellation: CancellationToken,
        task: JoinHandle<()>,
    },
}

pub struct WindowsFaultInjector {
    config: WindowsFaultConfig,
    #[cfg(windows)]
    active: Arc<Mutex<HashMap<String, WindowsEffect>>>,
}

impl WindowsFaultInjector {
    pub fn new(config: WindowsFaultConfig) -> Self {
        Self {
            config,
            #[cfg(windows)]
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for WindowsFaultInjector {
    fn default() -> Self {
        Self::new(WindowsFaultConfig::default())
    }
}

#[async_trait]
impl Injector for WindowsFaultInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        #[cfg(not(windows))]
        {
            let _ = target;
            return Err(ChaosError::SystemError(
                "windows_fault is only available on Windows".to_string(),
            ));
        }

        #[cfg(windows)]
        {
            let (effect, metadata) = match &self.config.mode {
                WindowsFaultMode::ServiceStop { service } => {
                    require_system_target(target, "service_stop")?;
                    let was_running = service_is_running(service).await?;
                    if was_running {
                        service_command("stop", service).await?;
                        wait_for_service_state(service, false).await?;
                    }
                    (
                        WindowsEffect::Service {
                            service: service.clone(),
                            was_running,
                        },
                        json!({
                            "mode": "service_stop",
                            "service": service,
                            "was_running": was_running,
                            "observed_running_after_injection": service_is_running(service).await?,
                        }),
                    )
                }
                WindowsFaultMode::FileLock => {
                    let Target::File { path } = target else {
                        return Err(ChaosError::InvalidConfig(
                            "Windows file_lock requires a file target".to_string(),
                        ));
                    };
                    let file = exclusive_open(path)?;
                    if exclusive_open(path).is_ok() {
                        return Err(ChaosError::InjectionFailed(format!(
                            "Exclusive lock verification unexpectedly succeeded for {}",
                            path.display()
                        )));
                    }
                    (
                        WindowsEffect::FileLock {
                            path: path.clone(),
                            file,
                        },
                        json!({
                            "mode": "file_lock",
                            "path": path,
                            "exclusive_open_blocked": true,
                        }),
                    )
                }
                WindowsFaultMode::HandleExhaustion { count } => {
                    require_system_target(target, "handle_exhaustion")?;
                    let (directory, files) = open_handles(*count)?;
                    let opened = files.len();
                    if opened == 0 {
                        return Err(ChaosError::InjectionFailed(
                            "Windows handle pressure did not open any handles".to_string(),
                        ));
                    }
                    (
                        WindowsEffect::Handles {
                            directory: directory.clone(),
                            files,
                        },
                        json!({
                            "mode": "handle_exhaustion",
                            "requested_handles": count,
                            "opened_handles": opened,
                            "temporary_directory": directory,
                        }),
                    )
                }
                WindowsFaultMode::NamedPipeBlackhole { pipe_name } => {
                    require_system_target(target, "named_pipe_blackhole")?;
                    let path = normalize_pipe_name(pipe_name);
                    let server = ServerOptions::new()
                        .first_pipe_instance(true)
                        .create(&path)
                        .map_err(|error| {
                            ChaosError::InjectionFailed(format!(
                                "Failed to create named pipe '{}': {}",
                                path, error
                            ))
                        })?;
                    let cancellation = CancellationToken::new();
                    let stop = cancellation.clone();
                    let task = tokio::spawn(async move {
                        tokio::select! {
                            _ = stop.cancelled() => {}
                            connected = server.connect() => {
                                if connected.is_ok() {
                                    stop.cancelled().await;
                                }
                            }
                        }
                    });
                    (
                        WindowsEffect::NamedPipe { cancellation, task },
                        json!({
                            "mode": "named_pipe_blackhole",
                            "pipe_name": path,
                            "listening": true,
                        }),
                    )
                }
            };

            let handle = InjectionHandle::new("windows_fault", target.clone(), metadata);
            self.active.lock().await.insert(handle.id.clone(), effect);
            Ok(handle)
        }
    }

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        #[cfg(not(windows))]
        {
            let _ = handle;
            return Ok(());
        }

        #[cfg(windows)]
        {
            if let Some(effect) = self.active.lock().await.remove(&handle.id) {
                cleanup_effect(effect).await?;
                return Ok(());
            }

            recover_from_metadata(&handle).await
        }
    }

    fn name(&self) -> &str {
        "windows_fault"
    }

    fn status(&self) -> InjectorStatus {
        if cfg!(windows) {
            InjectorStatus::Experimental
        } else {
            InjectorStatus::Planned
        }
    }

    async fn validate(&self) -> Result<()> {
        self.config.validate()?;
        #[cfg(windows)]
        {
            if matches!(self.config.mode, WindowsFaultMode::ServiceStop { .. }) {
                crate::environment::require_command("sc.exe")?;
                crate::environment::require_elevated_privileges().await?;
            }
            Ok(())
        }
        #[cfg(not(windows))]
        Err(ChaosError::SystemError(
            "windows_fault is only available on Windows".to_string(),
        ))
    }

    fn required_capabilities(&self) -> Vec<String> {
        match self.config.mode {
            WindowsFaultMode::ServiceStop { .. } => vec!["Administrator".to_string()],
            _ => Vec::new(),
        }
    }
}

#[cfg(windows)]
fn require_system_target(target: &Target, mode: &str) -> Result<()> {
    if matches!(target, Target::System) {
        Ok(())
    } else {
        Err(ChaosError::InvalidConfig(format!(
            "Windows {} requires a system target",
            mode
        )))
    }
}

#[cfg(windows)]
fn exclusive_open(path: &std::path::Path) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(path)
        .map_err(|error| {
            ChaosError::InjectionFailed(format!(
                "Failed to lock Windows file '{}': {}",
                path.display(),
                error
            ))
        })
}

#[cfg(windows)]
fn open_handles(count: usize) -> Result<(PathBuf, Vec<File>)> {
    let directory =
        std::env::temp_dir().join(format!("chaos-windows-handles-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory)?;
    let mut files = Vec::with_capacity(count);
    for index in 0..count {
        let path = directory.join(format!("handle-{}.tmp", index));
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(path)
        {
            Ok(file) => files.push(file),
            Err(error) if !files.is_empty() => {
                tracing::warn!("Stopped after opening {} handles: {}", files.len(), error);
                break;
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok((directory, files))
}

#[cfg(windows)]
fn normalize_pipe_name(name: &str) -> String {
    if name.starts_with(r"\\.\pipe\") {
        name.to_string()
    } else {
        format!(r"\\.\pipe\{}", name)
    }
}

#[cfg(windows)]
async fn service_command(command: &str, service: &str) -> Result<()> {
    let output = Command::new("sc.exe")
        .args([command, service])
        .output()
        .await
        .map_err(|error| ChaosError::SystemError(format!("Failed to run sc.exe: {}", error)))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ChaosError::InjectionFailed(format!(
            "sc.exe {} {} failed: {}",
            command,
            service,
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

#[cfg(windows)]
async fn service_is_running(service: &str) -> Result<bool> {
    let output = Command::new("sc.exe")
        .args(["query", service])
        .output()
        .await
        .map_err(|error| ChaosError::SystemError(format!("Failed to query service: {}", error)))?;
    if !output.status.success() {
        return Err(ChaosError::InvalidConfig(format!(
            "Windows service '{}' was not found",
            service
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).contains("RUNNING"))
}

#[cfg(windows)]
async fn wait_for_service_state(service: &str, running: bool) -> Result<()> {
    for _ in 0..50 {
        if service_is_running(service).await? == running {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Err(ChaosError::InjectionFailed(format!(
        "Windows service '{}' did not become {}",
        service,
        if running { "running" } else { "stopped" }
    )))
}

#[cfg(windows)]
async fn cleanup_effect(effect: WindowsEffect) -> Result<()> {
    match effect {
        WindowsEffect::Service {
            service,
            was_running,
        } => {
            if was_running && !service_is_running(&service).await? {
                service_command("start", &service).await?;
                wait_for_service_state(&service, true).await?;
            }
        }
        WindowsEffect::FileLock { path, file } => {
            drop(file);
            let verification = exclusive_open(&path)?;
            drop(verification);
        }
        WindowsEffect::Handles { directory, files } => {
            drop(files);
            if directory.is_dir() {
                std::fs::remove_dir_all(directory)?;
            }
        }
        WindowsEffect::NamedPipe { cancellation, task } => {
            cancellation.cancel();
            task.await.map_err(|error| {
                ChaosError::CleanupFailed(format!("Named pipe task failed: {}", error))
            })?;
        }
    }
    Ok(())
}

#[cfg(windows)]
async fn recover_from_metadata(handle: &InjectionHandle) -> Result<()> {
    match handle.metadata.get("mode").and_then(|value| value.as_str()) {
        Some("service_stop") if handle.metadata["was_running"].as_bool() == Some(true) => {
            let service = handle.metadata["service"].as_str().ok_or_else(|| {
                ChaosError::CleanupFailed("Windows recovery is missing service name".to_string())
            })?;
            if !service_is_running(service).await? {
                service_command("start", service).await?;
                wait_for_service_state(service, true).await?;
            }
        }
        Some("handle_exhaustion") => {
            if let Some(directory) = handle.metadata["temporary_directory"].as_str() {
                let path = PathBuf::from(directory);
                if path.is_dir() {
                    std::fs::remove_dir_all(path)?;
                }
            }
        }
        Some("file_lock" | "named_pipe_blackhole" | "service_stop") => {}
        _ => {
            return Err(ChaosError::CleanupFailed(
                "Windows recovery journal has an unknown fault mode".to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use tokio::net::windows::named_pipe::ClientOptions;

    #[tokio::test]
    async fn file_lock_has_a_real_effect_and_is_restored() {
        let directory =
            std::env::temp_dir().join(format!("chaos-lock-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("locked.db");
        std::fs::write(&path, b"original").unwrap();
        let injector = WindowsFaultInjector::new(WindowsFaultConfig {
            mode: WindowsFaultMode::FileLock,
        });
        let handle = injector.inject(&Target::file(&path)).await.unwrap();

        assert!(OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .is_err());
        injector.remove(handle).await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert!(OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .is_ok());

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn handle_pressure_opens_and_removes_real_handles() {
        let injector = WindowsFaultInjector::new(WindowsFaultConfig {
            mode: WindowsFaultMode::HandleExhaustion { count: 8 },
        });
        let handle = injector.inject(&Target::System).await.unwrap();
        assert_eq!(handle.metadata["opened_handles"], 8);
        let directory = PathBuf::from(handle.metadata["temporary_directory"].as_str().unwrap());
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 8);
        injector.remove(handle).await.unwrap();
        assert!(!directory.exists());
    }

    #[tokio::test]
    async fn named_pipe_accepts_a_client_and_is_removed() {
        let name = format!("chaos-test-{}", uuid::Uuid::new_v4());
        let path = normalize_pipe_name(&name);
        let injector = WindowsFaultInjector::new(WindowsFaultConfig {
            mode: WindowsFaultMode::NamedPipeBlackhole { pipe_name: name },
        });
        let handle = injector.inject(&Target::System).await.unwrap();
        let client = ClientOptions::new().open(&path).unwrap();
        injector.remove(handle).await.unwrap();
        drop(client);
        assert!(ClientOptions::new().open(&path).is_err());
    }
}
