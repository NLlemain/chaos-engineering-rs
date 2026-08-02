use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProcessFreezeConfig {
    pub pid: Option<u32>,
    pub freeze_duration: Option<std::time::Duration>,
}

pub struct ProcessFreezeInjector {
    config: ProcessFreezeConfig,
}

impl Default for ProcessFreezeInjector {
    fn default() -> Self {
        Self::new(ProcessFreezeConfig::default())
    }
}

impl ProcessFreezeInjector {
    pub fn new(config: ProcessFreezeConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> ProcessFreezeBuilder {
        ProcessFreezeBuilder::default()
    }
}

#[async_trait]
impl Injector for ProcessFreezeInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        #[cfg(not(unix))]
        {
            let _ = target;
            Err(ChaosError::SystemError(
                "process_freeze is only implemented on Unix".to_string(),
            ))
        }

        #[cfg(unix)]
        {
            let pid = match target {
                Target::Process { pid } => *pid,
                _ => self.config.pid.ok_or_else(|| {
                    ChaosError::InvalidConfig(
                        "Target must specify process PID for freeze".to_string(),
                    )
                })?,
            };
            info!("Injecting Process Freeze on PID {}", pid);
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;
            kill(Pid::from_raw(pid as i32), Signal::SIGSTOP).map_err(|e| {
                ChaosError::InjectionFailed(format!("Failed to send SIGSTOP: {}", e))
            })?;

            let metadata = serde_json::json!({
                "pid": pid,
                "status": "frozen",
            });
            Ok(InjectionHandle::new(
                "process_freeze",
                target.clone(),
                metadata,
            ))
        }
    }

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        #[cfg(not(unix))]
        {
            let _ = handle;
            Ok(())
        }

        #[cfg(unix)]
        {
            let pid = handle
                .metadata
                .get("pid")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .ok_or_else(|| ChaosError::CleanupFailed("Missing PID metadata".to_string()))?;
            info!("Resuming frozen process PID {}", pid);
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;
            kill(Pid::from_raw(pid as i32), Signal::SIGCONT)
                .map_err(|e| ChaosError::CleanupFailed(format!("Failed to send SIGCONT: {}", e)))?;
            Ok(())
        }
    }

    fn name(&self) -> &str {
        "process_freeze"
    }

    fn status(&self) -> crate::injectors::InjectorStatus {
        if cfg!(unix) {
            crate::injectors::InjectorStatus::Stable
        } else {
            crate::injectors::InjectorStatus::Planned
        }
    }

    async fn validate(&self) -> Result<()> {
        if self.config.pid == Some(0) {
            return Err(ChaosError::InvalidConfig(
                "Process freeze PID must be greater than zero".to_string(),
            ));
        }
        if cfg!(unix) {
            Ok(())
        } else {
            Err(ChaosError::SystemError(
                "process_freeze is only implemented on Unix".to_string(),
            ))
        }
    }

    fn required_capabilities(&self) -> Vec<String> {
        if cfg!(unix) {
            vec!["Permission to signal the target process".to_string()]
        } else {
            Vec::new()
        }
    }
}

#[derive(Default)]
pub struct ProcessFreezeBuilder {
    pid: Option<u32>,
    freeze_duration: Option<std::time::Duration>,
}

impl ProcessFreezeBuilder {
    pub fn pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }

    pub fn freeze_duration(mut self, duration: std::time::Duration) -> Self {
        self.freeze_duration = Some(duration);
        self
    }

    pub fn build(self) -> ProcessFreezeInjector {
        ProcessFreezeInjector::new(ProcessFreezeConfig {
            pid: self.pid,
            freeze_duration: self.freeze_duration,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_freeze_builder() {
        let injector = ProcessFreezeInjector::builder().pid(12345).build();
        assert_eq!(injector.config.pid, Some(12345));
    }
}
