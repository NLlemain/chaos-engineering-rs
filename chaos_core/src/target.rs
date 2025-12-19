use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Target {
    /// Target a specific process by PID
    Process { pid: u32 },

    /// Target network traffic to/from an address
    Network { address: SocketAddr },

    /// Target a container by ID
    Container { id: String },

    /// Target a specific thread
    Thread { tid: u32 },

    /// Target all processes matching a pattern
    ProcessPattern { pattern: String },
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

    pub fn thread(tid: u32) -> Self {
        Self::Thread { tid }
    }

    pub fn process_pattern(pattern: impl Into<String>) -> Self {
        Self::ProcessPattern {
            pattern: pattern.into(),
        }
    }

    pub fn description(&self) -> String {
        match self {
            Target::Process { pid } => format!("Process PID {}", pid),
            Target::Network { address } => format!("Network {}", address),
            Target::Container { id } => format!("Container {}", id),
            Target::Thread { tid } => format!("Thread TID {}", tid),
            Target::ProcessPattern { pattern } => format!("Process pattern '{}'", pattern),
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
                // Check if container exists (simplified)
                std::path::Path::new(&format!("/sys/fs/cgroup/docker/{}", id)).exists()
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
        }
    }
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
