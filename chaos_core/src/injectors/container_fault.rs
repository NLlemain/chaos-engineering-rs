use crate::{
    environment::command_available,
    error::{ChaosError, Result},
    handle::InjectionHandle,
    injectors::{Injector, InjectorStatus},
    target::Target,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::process::Output;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerFaultAction {
    Pause,
    Stop,
    Kill,
    Restart,
}

impl std::fmt::Display for ContainerFaultAction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Pause => "pause",
            Self::Stop => "stop",
            Self::Kill => "kill",
            Self::Restart => "restart",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerFaultConfig {
    pub action: ContainerFaultAction,
    #[serde(default = "default_stop_timeout")]
    pub stop_timeout_seconds: u64,
}

fn default_stop_timeout() -> u64 {
    10
}

impl Default for ContainerFaultConfig {
    fn default() -> Self {
        Self {
            action: ContainerFaultAction::Pause,
            stop_timeout_seconds: default_stop_timeout(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContainerState {
    id: String,
    was_running: bool,
    was_paused: bool,
}

pub struct ContainerFaultInjector {
    config: ContainerFaultConfig,
}

impl ContainerFaultInjector {
    pub fn new(config: ContainerFaultConfig) -> Self {
        Self { config }
    }

    async fn resolve(&self, target: &Target) -> Result<Vec<String>> {
        match target {
            Target::Container { id } => {
                let output = docker(&["inspect", "--format", "{{.Id}}", id]).await?;
                Ok(nonempty_lines(&output))
            }
            Target::ComposeService {
                service,
                file,
                project,
            } => {
                let mut args = vec![
                    "compose".to_string(),
                    "-f".to_string(),
                    file.to_string_lossy().into_owned(),
                ];
                if let Some(project) = project {
                    args.extend(["-p".to_string(), project.clone()]);
                }
                args.extend([
                    "ps".to_string(),
                    "--all".to_string(),
                    "-q".to_string(),
                    service.clone(),
                ]);
                docker_owned(&args)
                    .await
                    .map(|output| nonempty_lines(&output))
            }
            _ => Err(ChaosError::InvalidConfig(
                "container_fault requires a container or Compose service target".to_string(),
            )),
        }
        .and_then(|ids| {
            if ids.is_empty() {
                Err(ChaosError::TargetNotFound(target.description()))
            } else {
                Ok(ids)
            }
        })
    }

    async fn state(id: &str) -> Result<ContainerState> {
        let output = docker(&[
            "inspect",
            "--format",
            "{{.Id}} {{.State.Running}} {{.State.Paused}}",
            id,
        ])
        .await?;
        let mut fields = output.split_whitespace();
        let resolved = fields
            .next()
            .ok_or_else(|| ChaosError::TargetNotFound(id.to_string()))?;
        let was_running = fields.next() == Some("true");
        let was_paused = fields.next() == Some("true");
        Ok(ContainerState {
            id: resolved.to_string(),
            was_running,
            was_paused,
        })
    }

    async fn apply(&self, state: &ContainerState) -> Result<()> {
        match self.config.action {
            ContainerFaultAction::Pause => {
                if !state.was_running {
                    return Err(ChaosError::InjectionFailed(format!(
                        "Container {} is not running",
                        state.id
                    )));
                }
                if state.was_paused {
                    return Err(ChaosError::InjectionFailed(format!(
                        "Container {} is already paused",
                        state.id
                    )));
                }
                docker(&["pause", &state.id]).await?;
            }
            ContainerFaultAction::Stop => {
                if !state.was_running {
                    return Err(ChaosError::InjectionFailed(format!(
                        "Container {} is not running",
                        state.id
                    )));
                }
                let timeout = self.config.stop_timeout_seconds.to_string();
                docker(&["stop", "--time", &timeout, &state.id]).await?;
            }
            ContainerFaultAction::Kill => {
                if !state.was_running {
                    return Err(ChaosError::InjectionFailed(format!(
                        "Container {} is not running",
                        state.id
                    )));
                }
                docker(&["kill", &state.id]).await?;
            }
            ContainerFaultAction::Restart => {
                if !state.was_running {
                    return Err(ChaosError::InjectionFailed(format!(
                        "Container {} is not running",
                        state.id
                    )));
                }
                docker(&["restart", &state.id]).await?;
            }
        }
        Ok(())
    }

    async fn restore(action: ContainerFaultAction, state: &ContainerState) -> Result<()> {
        match action {
            ContainerFaultAction::Pause if state.was_running && !state.was_paused => {
                docker(&["unpause", &state.id]).await?;
            }
            ContainerFaultAction::Stop | ContainerFaultAction::Kill if state.was_running => {
                docker(&["start", &state.id]).await?;
                if state.was_paused {
                    docker(&["pause", &state.id]).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl Default for ContainerFaultInjector {
    fn default() -> Self {
        Self::new(ContainerFaultConfig::default())
    }
}

#[async_trait]
impl Injector for ContainerFaultInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        let ids = self.resolve(target).await?;
        let mut states = Vec::with_capacity(ids.len());
        for id in ids {
            states.push(Self::state(&id).await?);
        }

        let mut applied = Vec::new();
        for state in &states {
            if let Err(error) = self.apply(state).await {
                for previous in applied.iter().rev() {
                    let _ = Self::restore(self.config.action, previous).await;
                }
                return Err(error);
            }
            applied.push(state.clone());
        }

        Ok(InjectionHandle::new(
            self.name(),
            target.clone(),
            serde_json::json!({
                "action": self.config.action,
                "containers": states,
            }),
        ))
    }

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        let action: ContainerFaultAction =
            serde_json::from_value(handle.metadata.get("action").cloned().ok_or_else(|| {
                ChaosError::CleanupFailed("Missing container action".to_string())
            })?)?;
        let states: Vec<ContainerState> =
            serde_json::from_value(handle.metadata.get("containers").cloned().ok_or_else(
                || ChaosError::CleanupFailed("Missing container states".to_string()),
            )?)?;
        let mut errors = Vec::new();
        for state in &states {
            if let Err(error) = Self::restore(action, state).await {
                errors.push(format!("{}: {}", state.id, error));
            }
        }
        if errors.is_empty() {
            info!("Restored {} container(s) after {}", states.len(), action);
            Ok(())
        } else {
            Err(ChaosError::CleanupFailed(errors.join("; ")))
        }
    }

    fn name(&self) -> &str {
        "container_fault"
    }

    fn status(&self) -> InjectorStatus {
        InjectorStatus::Stable
    }

    async fn validate(&self) -> Result<()> {
        if std::env::var_os("CHAOS_DOCKER_BIN").is_none() && !command_available("docker") {
            return Err(ChaosError::InvalidConfig(
                "Docker CLI was not found in PATH".to_string(),
            ));
        }
        docker(&["info", "--format", "{{.ServerVersion}}"]).await?;
        Ok(())
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["Docker CLI and daemon access".to_string()]
    }
}

fn nonempty_lines(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

async fn docker(args: &[&str]) -> Result<String> {
    let args: Vec<_> = args.iter().map(|value| (*value).to_string()).collect();
    docker_owned(&args).await
}

async fn docker_owned(args: &[String]) -> Result<String> {
    let output = tokio::process::Command::new(docker_binary())
        .args(args)
        .output()
        .await
        .map_err(|error| ChaosError::SystemError(format!("Failed to run Docker: {}", error)))?;
    command_result(output, args)
}

fn command_result(output: Output, args: &[String]) -> Result<String> {
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(ChaosError::InjectionFailed(format!(
            "docker {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

fn docker_binary() -> std::ffi::OsString {
    std::env::var_os("CHAOS_DOCKER_BIN").unwrap_or_else(|| "docker".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_compose_ids_without_blank_entries() {
        assert_eq!(
            nonempty_lines("abc\n\ndef\r\n"),
            vec!["abc".to_string(), "def".to_string()]
        );
    }
}
