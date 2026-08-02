use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
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
        let pid = match target {
            Target::Process { pid } => *pid,
            _ => self.config.pid.ok_or_else(|| {
                ChaosError::InvalidConfig("Target must specify process PID for freeze".to_string())
            })?,
        };

        info!("Injecting Process Freeze on PID {}", pid);

        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;
            kill(Pid::from_raw(pid as i32), Signal::SIGSTOP).map_err(|e| {
                ChaosError::InjectionFailed(format!("Failed to send SIGSTOP: {}", e))
            })?;
        }

        #[cfg(not(unix))]
        {
            info!("Process freeze simulated on non-unix OS for PID {}", pid);
        }

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

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        let pid = handle
            .metadata
            .get("pid")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .ok_or_else(|| ChaosError::CleanupFailed("Missing PID metadata".to_string()))?;

        info!("Resuming frozen process PID {}", pid);

        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;
            kill(Pid::from_raw(pid as i32), Signal::SIGCONT)
                .map_err(|e| ChaosError::CleanupFailed(format!("Failed to send SIGCONT: {}", e)))?;
        }

        #[cfg(not(unix))]
        {
            info!("Process resume simulated on non-unix OS for PID {}", pid);
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "process_freeze"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["CAP_SYS_PTRACE".to_string()]
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
