use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::process::Command;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Signal {
    SIGTERM,
    SIGKILL,
    SIGSTOP,
    SIGCONT,
    SIGHUP,
}

impl Signal {
    #[allow(dead_code)]
    fn as_unix_signal(&self) -> i32 {
        match self {
            Signal::SIGTERM => 15,
            Signal::SIGKILL => 9,
            Signal::SIGSTOP => 19,
            Signal::SIGCONT => 18,
            Signal::SIGHUP => 1,
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Signal::SIGTERM => "SIGTERM",
            Signal::SIGKILL => "SIGKILL",
            Signal::SIGSTOP => "SIGSTOP",
            Signal::SIGCONT => "SIGCONT",
            Signal::SIGHUP => "SIGHUP",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RestartMode {
    ColdBoot, // Full restart with initialization
    WarmBoot, // Fast restart preserving state
    None,     // No restart
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessKillConfig {
    pub signal: Signal,
    pub restart_delay: Duration,
    pub restart_mode: RestartMode,
    pub restart_command: Option<String>,
    pub health_check_url: Option<String>,
}

impl Default for ProcessKillConfig {
    fn default() -> Self {
        Self {
            signal: Signal::SIGTERM,
            restart_delay: Duration::from_secs(5),
            restart_mode: RestartMode::None,
            restart_command: None,
            health_check_url: None,
        }
    }
}

pub struct ProcessKillInjector {
    config: ProcessKillConfig,
}

impl Default for ProcessKillInjector {
    fn default() -> Self {
        Self {
            config: ProcessKillConfig::default(),
        }
    }
}

impl ProcessKillInjector {
    pub fn new(config: ProcessKillConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> ProcessKillBuilder {
        ProcessKillBuilder::default()
    }

    async fn send_signal(&self, pid: u32) -> Result<()> {
        info!("Sending {} to PID {}", self.config.signal.as_str(), pid);

        #[cfg(unix)]
        {
            use nix::sys::signal;
            use nix::unistd::Pid;

            let signal = match self.config.signal {
                Signal::SIGTERM => signal::Signal::SIGTERM,
                Signal::SIGKILL => signal::Signal::SIGKILL,
                Signal::SIGSTOP => signal::Signal::SIGSTOP,
                Signal::SIGCONT => signal::Signal::SIGCONT,
                Signal::SIGHUP => signal::Signal::SIGHUP,
            };

            signal::kill(Pid::from_raw(pid as i32), signal)
                .map_err(|e| ChaosError::ProcessError(format!("Failed to send signal: {}", e)))?;
        }

        #[cfg(windows)]
        {
            // Windows doesn't have Unix signals, use TerminateProcess
            if matches!(self.config.signal, Signal::SIGKILL) {
                Command::new("taskkill")
                    .args(&["/F", "/PID", &pid.to_string()])
                    .output()
                    .await
                    .map_err(|e| {
                        ChaosError::ProcessError(format!("Failed to kill process: {}", e))
                    })?;
            } else {
                return Err(ChaosError::SystemError(
                    "Only SIGKILL supported on Windows".to_string(),
                ));
            }
        }

        Ok(())
    }

    async fn wait_for_process_death(&self, pid: u32, timeout: Duration) -> Result<()> {
        let start = tokio::time::Instant::now();

        while start.elapsed() < timeout {
            let target = Target::process(pid);
            if !target.exists().await {
                info!("Process {} terminated", pid);
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        warn!("Process {} did not terminate within timeout", pid);
        Ok(())
    }

    async fn restart_process(&self) -> Result<u32> {
        if self.config.restart_mode == RestartMode::None {
            return Err(ChaosError::InvalidConfig(
                "No restart mode configured".to_string(),
            ));
        }

        let command = self.config.restart_command.as_ref().ok_or_else(|| {
            ChaosError::InvalidConfig("No restart command configured".to_string())
        })?;

        info!(
            "Restarting process after {} seconds (mode: {:?})",
            self.config.restart_delay.as_secs(),
            self.config.restart_mode
        );

        tokio::time::sleep(self.config.restart_delay).await;

        // Execute restart command
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .spawn()
            .map_err(|e| ChaosError::ProcessError(format!("Failed to restart process: {}", e)))?;

        let pid = output.id().ok_or_else(|| {
            ChaosError::ProcessError("Failed to get PID of restarted process".to_string())
        })?;

        info!("Process restarted with PID {}", pid);

        // Wait for health check if configured
        if let Some(health_url) = &self.config.health_check_url {
            self.wait_for_health(health_url).await?;
        }

        Ok(pid)
    }

    async fn wait_for_health(&self, url: &str) -> Result<()> {
        info!("Waiting for health check: {}", url);

        for attempt in 1..=30 {
            match reqwest::get(url).await {
                Ok(response) if response.status().is_success() => {
                    info!("Health check passed");
                    return Ok(());
                }
                _ => {
                    if attempt % 5 == 0 {
                        info!("Health check attempt {}/30...", attempt);
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        Err(ChaosError::ProcessError(
            "Health check failed after 30 attempts".to_string(),
        ))
    }
}

#[async_trait]
impl Injector for ProcessKillInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        let Target::Process { pid } = target else {
            return Err(ChaosError::InvalidConfig(
                "Process kill requires Process target".to_string(),
            ));
        };

        let original_pid = *pid;
        self.send_signal(*pid).await?;

        // Wait for process to die if not SIGSTOP
        if !matches!(self.config.signal, Signal::SIGSTOP) {
            self.wait_for_process_death(*pid, Duration::from_secs(10))
                .await?;
        }

        // Restart if configured
        let new_pid = if self.config.restart_mode != RestartMode::None {
            Some(self.restart_process().await?)
        } else {
            None
        };

        let metadata = serde_json::json!({
            "original_pid": original_pid,
            "new_pid": new_pid,
            "signal": self.config.signal.as_str(),
            "restart_mode": format!("{:?}", self.config.restart_mode),
        });

        Ok(InjectionHandle::new(
            "process_kill",
            target.clone(),
            metadata,
        ))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        // Process kill is a one-time action, cleanup is no-op
        Ok(())
    }

    fn name(&self) -> &str {
        "process_kill"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["CAP_KILL".to_string()]
    }
}

#[derive(Default)]
pub struct ProcessKillBuilder {
    signal: Option<Signal>,
    restart_delay: Option<Duration>,
    restart_mode: Option<RestartMode>,
    restart_command: Option<String>,
    health_check_url: Option<String>,
}

impl ProcessKillBuilder {
    pub fn signal(mut self, signal: Signal) -> Self {
        self.signal = Some(signal);
        self
    }

    pub fn restart_delay(mut self, delay: Duration) -> Self {
        self.restart_delay = Some(delay);
        self
    }

    pub fn restart_mode(mut self, mode: RestartMode) -> Self {
        self.restart_mode = Some(mode);
        self
    }

    pub fn restart_command(mut self, command: impl Into<String>) -> Self {
        self.restart_command = Some(command.into());
        self
    }

    pub fn health_check_url(mut self, url: impl Into<String>) -> Self {
        self.health_check_url = Some(url.into());
        self
    }

    pub fn build(self) -> ProcessKillInjector {
        ProcessKillInjector {
            config: ProcessKillConfig {
                signal: self.signal.unwrap_or(Signal::SIGTERM),
                restart_delay: self.restart_delay.unwrap_or(Duration::from_secs(5)),
                restart_mode: self.restart_mode.unwrap_or(RestartMode::None),
                restart_command: self.restart_command,
                health_check_url: self.health_check_url,
            },
        }
    }
}

// Process Suspend/Resume Injector
#[derive(Debug, Clone)]
pub struct ProcessSuspendInjector {
    duration: Duration,
}

impl ProcessSuspendInjector {
    pub fn new(duration: Duration) -> Self {
        Self { duration }
    }

    async fn suspend(&self, pid: u32) -> Result<()> {
        info!("Suspending process {} for {:?}", pid, self.duration);

        #[cfg(unix)]
        {
            use nix::sys::signal;
            use nix::unistd::Pid;

            signal::kill(Pid::from_raw(pid as i32), signal::Signal::SIGSTOP).map_err(|e| {
                ChaosError::ProcessError(format!("Failed to suspend process: {}", e))
            })?;

            tokio::time::sleep(self.duration).await;

            signal::kill(Pid::from_raw(pid as i32), signal::Signal::SIGCONT).map_err(|e| {
                ChaosError::ProcessError(format!("Failed to resume process: {}", e))
            })?;
        }

        Ok(())
    }
}

#[async_trait]
impl Injector for ProcessSuspendInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        let Target::Process { pid } = target else {
            return Err(ChaosError::InvalidConfig(
                "Process suspend requires Process target".to_string(),
            ));
        };

        self.suspend(*pid).await?;

        let metadata = serde_json::json!({
            "pid": pid,
            "duration_secs": self.duration.as_secs(),
        });

        Ok(InjectionHandle::new(
            "process_suspend",
            target.clone(),
            metadata,
        ))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        // Suspension is time-limited, no cleanup needed
        Ok(())
    }

    fn name(&self) -> &str {
        "process_suspend"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["CAP_KILL".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_conversion() {
        assert_eq!(Signal::SIGTERM.as_unix_signal(), 15);
        assert_eq!(Signal::SIGKILL.as_unix_signal(), 9);
        assert_eq!(Signal::SIGTERM.as_str(), "SIGTERM");
    }

    #[test]
    fn test_process_kill_builder() {
        let injector = ProcessKillInjector::builder()
            .signal(Signal::SIGKILL)
            .restart_delay(Duration::from_secs(10))
            .restart_mode(RestartMode::ColdBoot)
            .build();

        assert!(matches!(injector.config.signal, Signal::SIGKILL));
        assert_eq!(injector.config.restart_delay, Duration::from_secs(10));
    }
}
