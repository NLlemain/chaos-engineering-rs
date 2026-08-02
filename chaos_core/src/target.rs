use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Target {
    /// Target a specific process by PID
    Process { pid: u32 },

    /// Target network traffic to/from an address
    Network { address: SocketAddr },

    /// Target a container by ID
    Container { id: String },

    /// Target every container currently backing a Docker Compose service
    ComposeService {
        service: String,
        file: PathBuf,
        project: Option<String>,
    },

    /// Target a specific thread
    Thread { tid: u32 },

    /// Target all processes matching a pattern
    ProcessPattern { pattern: String },

    /// Target system-wide resources
    System,
}

impl Target {
    pub fn process(pid: u32) -> Self {
        Self::Process { pid }
    }

    pub fn network(address: SocketAddr) -> Self {
        Self::Network { address }
    }

    pub fn container(id: impl Into<String>) -> Self {
        Self::Container { id: id.into() }
    }

    pub fn compose_service(
        service: impl Into<String>,
        file: impl Into<PathBuf>,
        project: Option<String>,
    ) -> Self {
        Self::ComposeService {
            service: service.into(),
            file: file.into(),
            project,
        }
    }

    pub fn thread(tid: u32) -> Self {
        Self::Thread { tid }
    }

    pub fn process_pattern(pattern: impl Into<String>) -> Self {
        Self::ProcessPattern {
            pattern: pattern.into(),
        }
    }

    pub fn system() -> Self {
        Self::System
    }

    pub fn description(&self) -> String {
        match self {
            Target::Process { pid } => format!("Process PID {}", pid),
            Target::Network { address } => format!("Network {}", address),
            Target::Container { id } => format!("Container {}", id),
            Target::ComposeService {
                service,
                file,
                project,
            } => format!(
                "Compose service {} in {}{}",
                service,
                file.display(),
                project
                    .as_ref()
                    .map(|name| format!(" (project {})", name))
                    .unwrap_or_default()
            ),
            Target::Thread { tid } => format!("Thread TID {}", tid),
            Target::ProcessPattern { pattern } => format!("Process pattern '{}'", pattern),
            Target::System => "System".to_string(),
        }
    }

    pub async fn exists(&self) -> bool {
        match self {
            Target::Process { pid } => {
                #[cfg(unix)]
                {
                    use nix::sys::signal;
                    use nix::unistd::Pid;
                    signal::kill(Pid::from_raw(*pid as i32), None).is_ok()
                }
                #[cfg(not(unix))]
                {
                    use sysinfo::System;
                    let mut sys = System::new_all();
                    sys.refresh_processes();
                    sys.process(sysinfo::Pid::from(*pid as usize)).is_some()
                }
            }
            Target::Network { address } => {
                // Check if address is reachable
                tokio::net::TcpStream::connect(address).await.is_ok()
            }
            Target::Container { id } => {
                docker_succeeds(["inspect", "--type", "container", id]).await
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
                docker_output(&args)
                    .await
                    .is_some_and(|output| !output.trim().is_empty())
            }
            Target::Thread { tid: _ } => {
                #[cfg(unix)]
                {
                    // Thread validation would require checking /proc/<tid>
                    true
                }
                #[cfg(not(unix))]
                {
                    false
                }
            }
            Target::ProcessPattern { pattern } => {
                use sysinfo::System;
                let mut sys = System::new_all();
                sys.refresh_processes();
                sys.processes().values().any(|p| p.name().contains(pattern))
            }
            Target::System => true,
        }
    }
}

async fn docker_succeeds<'a>(args: impl IntoIterator<Item = &'a str>) -> bool {
    let args: Vec<_> = args.into_iter().map(str::to_string).collect();
    docker_output(&args).await.is_some()
}

async fn docker_output(args: &[String]) -> Option<String> {
    let output = tokio::process::Command::new(
        std::env::var_os("CHAOS_DOCKER_BIN").unwrap_or_else(|| "docker".into()),
    )
    .args(args)
    .output()
    .await
    .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_description() {
        let target = Target::process(1234);
        assert_eq!(target.description(), "Process PID 1234");

        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let target = Target::network(addr);
        assert_eq!(target.description(), "Network 127.0.0.1:8080");
    }

    #[tokio::test]
    async fn test_target_exists() {
        // Test current process exists
        let target = Target::process(std::process::id());
        assert!(target.exists().await);

        // Test non-existent process
        let target = Target::process(999999);
        assert!(!target.exists().await);
    }
}
