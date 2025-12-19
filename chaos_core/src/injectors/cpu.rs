use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuStarvationConfig {
    pub intensity: f64,    // 0.0 - 1.0, percentage of CPU to consume
    pub threads: Vec<u32>, // Specific CPU cores to target (empty = all)
    pub duration: Option<std::time::Duration>,
}

impl Default for CpuStarvationConfig {
    fn default() -> Self {
        Self {
            intensity: 0.8,
            threads: vec![],
            duration: None,
        }
    }
}

pub struct CpuStarvationInjector {
    config: CpuStarvationConfig,
    stop_signal: Arc<RwLock<bool>>,
}

impl Default for CpuStarvationInjector {
    fn default() -> Self {
        Self {
            config: CpuStarvationConfig::default(),
            stop_signal: Arc::new(RwLock::new(false)),
        }
    }
}

impl CpuStarvationInjector {
    pub fn new(config: CpuStarvationConfig) -> Self {
        Self {
            config,
            stop_signal: Arc::new(RwLock::new(false)),
        }
    }

    pub fn builder() -> CpuStarvationBuilder {
        CpuStarvationBuilder::default()
    }

    async fn spawn_cpu_burner(&self, core_id: Option<u32>) -> tokio::task::JoinHandle<()> {
        let intensity = self.config.intensity;
        let stop_signal = self.stop_signal.clone();

        tokio::task::spawn_blocking(move || {
            #[cfg(unix)]
            {
                // Pin to specific core if requested
                if let Some(core) = core_id {
                    use nix::sched::{sched_setaffinity, CpuSet};
                    use nix::unistd::Pid;

                    let mut cpu_set = CpuSet::new();
                    cpu_set.set(core as usize).ok();
                    sched_setaffinity(Pid::from_raw(0), &cpu_set).ok();
                }
            }

            info!("Starting CPU burner on core {:?}", core_id);

            // Spin loop with controlled intensity
            let burn_duration = std::time::Duration::from_micros((intensity * 1000.0) as u64);
            let sleep_duration =
                std::time::Duration::from_micros(((1.0 - intensity) * 1000.0) as u64);

            loop {
                // Check stop signal
                if *futures::executor::block_on(async { stop_signal.read().await }) {
                    info!("Stopping CPU burner on core {:?}", core_id);
                    break;
                }

                // Busy loop to consume CPU
                let start = std::time::Instant::now();
                while start.elapsed() < burn_duration {
                    // CPU-intensive operation
                    let _ = (0..1000).fold(0u64, |acc, x| acc.wrapping_add(x));
                }

                // Sleep to achieve desired intensity
                if sleep_duration > std::time::Duration::ZERO {
                    std::thread::sleep(sleep_duration);
                }
            }
        })
    }
}

#[async_trait]
impl Injector for CpuStarvationInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        info!(
            "Injecting CPU starvation: intensity={}, cores={:?}",
            self.config.intensity, self.config.threads
        );

        // Reset stop signal
        *self.stop_signal.write().await = false;

        let cores = if self.config.threads.is_empty() {
            // Use all available cores
            let num_cpus = num_cpus::get() as u32;
            (0..num_cpus).collect()
        } else {
            self.config.threads.clone()
        };

        // Spawn burner threads
        let mut handles = vec![];
        for core in &cores {
            let handle = self.spawn_cpu_burner(Some(*core)).await;
            handles.push(handle);
        }

        let metadata = serde_json::json!({
            "intensity": self.config.intensity,
            "cores": cores,
            "num_threads": handles.len(),
        });

        Ok(InjectionHandle::new(
            "cpu_starvation",
            target.clone(),
            metadata,
        ))
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        info!("Removing CPU starvation");

        // Signal all threads to stop
        *self.stop_signal.write().await = true;

        // Give threads time to exit gracefully
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        Ok(())
    }

    fn name(&self) -> &str {
        "cpu_starvation"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["CAP_SYS_NICE".to_string()]
    }
}

#[derive(Default)]
pub struct CpuStarvationBuilder {
    intensity: Option<f64>,
    threads: Option<Vec<u32>>,
    duration: Option<std::time::Duration>,
}

impl CpuStarvationBuilder {
    pub fn intensity(mut self, intensity: f64) -> Self {
        self.intensity = Some(intensity.clamp(0.0, 1.0));
        self
    }

    pub fn threads(mut self, threads: Vec<u32>) -> Self {
        self.threads = Some(threads);
        self
    }

    pub fn duration(mut self, duration: std::time::Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn build(self) -> CpuStarvationInjector {
        CpuStarvationInjector {
            config: CpuStarvationConfig {
                intensity: self.intensity.unwrap_or(0.8),
                threads: self.threads.unwrap_or_default(),
                duration: self.duration,
            },
            stop_signal: Arc::new(RwLock::new(false)),
        }
    }
}

// CPU Quota Injector (using cgroups)
#[derive(Debug, Clone)]
pub struct CpuQuotaInjector {
    #[allow(dead_code)]
    quota: u32, // Percentage of CPU time (0-100)
}

impl CpuQuotaInjector {
    pub fn new(quota: u32) -> Self {
        Self {
            quota: quota.min(100),
        }
    }

    #[cfg(target_os = "linux")]
    async fn inject_linux(&self, target: &Target) -> Result<InjectionHandle> {
        let Target::Process { pid } = target else {
            return Err(ChaosError::InvalidConfig(
                "CPU quota requires Process target".to_string(),
            ));
        };

        info!("Setting CPU quota to {}% for PID {}", self.quota, pid);

        // Create a cgroup for this process
        let cgroup_name = format!("chaos_cpu_{}", pid);
        let cgroup_path = format!("/sys/fs/cgroup/cpu/{}", cgroup_name);

        // Create cgroup directory
        tokio::fs::create_dir_all(&cgroup_path)
            .await
            .map_err(|e| ChaosError::InjectionFailed(format!("Failed to create cgroup: {}", e)))?;

        // Set CPU quota (in microseconds per 100ms period)
        let quota_us = self.quota as u64 * 1000;
        let quota_file = format!("{}/cpu.cfs_quota_us", cgroup_path);
        tokio::fs::write(&quota_file, quota_us.to_string())
            .await
            .map_err(|e| ChaosError::InjectionFailed(format!("Failed to set CPU quota: {}", e)))?;

        // Add process to cgroup
        let tasks_file = format!("{}/tasks", cgroup_path);
        tokio::fs::write(&tasks_file, pid.to_string())
            .await
            .map_err(|e| {
                ChaosError::InjectionFailed(format!("Failed to add process to cgroup: {}", e))
            })?;

        let metadata = serde_json::json!({
            "cgroup_name": cgroup_name,
            "cgroup_path": cgroup_path,
            "quota": self.quota,
        });

        Ok(InjectionHandle::new("cpu_quota", target.clone(), metadata))
    }

    #[cfg(not(target_os = "linux"))]
    async fn inject_linux(&self, _target: &Target) -> Result<InjectionHandle> {
        Err(ChaosError::SystemError(
            "CPU quota injection only supported on Linux with cgroups".to_string(),
        ))
    }
}

#[async_trait]
impl Injector for CpuQuotaInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        self.inject_linux(target).await
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            let cgroup_path = _handle
                .metadata
                .get("cgroup_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ChaosError::CleanupFailed("Missing cgroup_path metadata".to_string())
                })?;

            info!("Removing CPU quota cgroup: {}", cgroup_path);

            // Remove cgroup (this will move processes back to parent)
            tokio::fs::remove_dir(cgroup_path).await.map_err(|e| {
                ChaosError::CleanupFailed(format!("Failed to remove cgroup: {}", e))
            })?;
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "cpu_quota"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["CAP_SYS_ADMIN".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_starvation_builder() {
        let injector = CpuStarvationInjector::builder()
            .intensity(0.5)
            .threads(vec![0, 1])
            .build();

        assert_eq!(injector.config.intensity, 0.5);
        assert_eq!(injector.config.threads, vec![0, 1]);
    }

    #[test]
    fn test_cpu_quota_clamping() {
        let injector = CpuQuotaInjector::new(150);
        assert_eq!(injector.quota, 100);
    }
}
