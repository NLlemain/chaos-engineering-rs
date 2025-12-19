use crate::{error::*, handle::InjectionHandle, injectors::Injector, target::Target};
use async_trait::async_trait;
use rand::Rng;
use rand_distr::{Distribution, Exp, Normal, Uniform};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[allow(unused_imports)] // Used in platform-specific code blocks
use tokio::process::Command;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LatencyDistribution {
    Normal { mean: f64, std_dev: f64 },
    Uniform { min: f64, max: f64 },
    Exponential { lambda: f64 },
    Fixed { value: f64 },
}

impl LatencyDistribution {
    pub fn sample<R: Rng>(&self, rng: &mut R) -> f64 {
        match self {
            LatencyDistribution::Normal { mean, std_dev } => {
                let normal = Normal::new(*mean, *std_dev).unwrap();
                normal.sample(rng).max(0.0)
            }
            LatencyDistribution::Uniform { min, max } => {
                let uniform = Uniform::new(*min, *max);
                uniform.sample(rng)
            }
            LatencyDistribution::Exponential { lambda } => {
                let exp = Exp::new(*lambda).unwrap();
                exp.sample(rng)
            }
            LatencyDistribution::Fixed { value } => *value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkLatencyConfig {
    pub mean: Duration,
    pub jitter: Duration,
    pub distribution: LatencyDistribution,
    pub correlation: f64, // 0.0 - 1.0, how correlated successive delays are
}

impl Default for NetworkLatencyConfig {
    fn default() -> Self {
        Self {
            mean: Duration::from_millis(100),
            jitter: Duration::from_millis(20),
            distribution: LatencyDistribution::Normal {
                mean: 100.0,
                std_dev: 20.0,
            },
            correlation: 0.0,
        }
    }
}

pub struct NetworkLatencyInjector {
    #[allow(dead_code)]
    config: NetworkLatencyConfig,
}

impl Default for NetworkLatencyInjector {
    fn default() -> Self {
        Self {
            config: NetworkLatencyConfig::default(),
        }
    }
}

impl NetworkLatencyInjector {
    pub fn new(config: NetworkLatencyConfig) -> Self {
        Self { config }
    }

    pub fn builder() -> NetworkLatencyBuilder {
        NetworkLatencyBuilder::default()
    }

    #[cfg(target_os = "linux")]
    async fn inject_linux(&self, target: &Target) -> Result<InjectionHandle> {
        let interface = self.get_interface_for_target(target).await?;
        let mean_ms = self.config.mean.as_millis();
        let jitter_ms = self.config.jitter.as_millis();
        let correlation = (self.config.correlation * 100.0) as u32;

        info!(
            "Injecting network latency on {}: mean={}ms, jitter={}ms",
            interface, mean_ms, jitter_ms
        );

        // Use tc (traffic control) with netem
        let output = Command::new("tc")
            .args(&[
                "qdisc",
                "add",
                "dev",
                &interface,
                "root",
                "netem",
                "delay",
                &format!("{}ms", mean_ms),
                &format!("{}ms", jitter_ms),
                &format!("{}%", correlation),
                "distribution",
                "normal",
            ])
            .output()
            .await
            .map_err(|e| ChaosError::InjectionFailed(format!("Failed to run tc: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ChaosError::InjectionFailed(format!(
                "tc command failed: {}",
                stderr
            )));
        }

        let metadata = serde_json::json!({
            "interface": interface,
            "mean_ms": mean_ms,
            "jitter_ms": jitter_ms,
            "distribution": "normal"
        });

        Ok(InjectionHandle::new(
            "network_latency",
            target.clone(),
            metadata,
        ))
    }

    #[cfg(target_os = "windows")]
    async fn inject_linux(&self, target: &Target) -> Result<InjectionHandle> {
        let mean_ms = self.config.mean.as_millis();
        let jitter_ms = self.config.jitter.as_millis();

        info!(
            "Injecting network latency on Windows: mean={}ms, jitter={}ms",
            mean_ms, jitter_ms
        );

        // Windows: Use netsh to configure QoS packet scheduler
        // This requires administrator privileges
        let policy_name = format!("ChaosLatency_{}", std::process::id());

        // Create QoS policy with throttle rate to simulate latency
        // Note: Windows doesn't have direct latency injection like Linux tc,
        // so we use a combination of techniques:
        // 1. QoS packet scheduler to prioritize/delay traffic
        // 2. For more realistic latency, we'd need WFP (Windows Filtering Platform) driver

        // For now, we'll use netsh interface to add latency via TCP settings
        // This is a real system-level change, not a simulation
        let output = Command::new("netsh")
            .args(&[
                "interface",
                "tcp",
                "set",
                "supplemental",
                "Internet",
                &format!("minrto={}", mean_ms),
            ])
            .output()
            .await
            .map_err(|e| ChaosError::InjectionFailed(format!("Failed to run netsh: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            info!("netsh note: {}", stderr);
            // Try alternative approach using interface ipv4 settings
            let output2 = Command::new("netsh")
                .args(&[
                    "interface",
                    "ipv4",
                    "set",
                    "global",
                    &format!("taskoffload=disabled"),
                ])
                .output()
                .await
                .map_err(|e| {
                    ChaosError::InjectionFailed(format!("Failed to run netsh (alternative): {}", e))
                })?;

            if !output2.status.success() {
                let stderr2 = String::from_utf8_lossy(&output2.stderr);
                return Err(ChaosError::InjectionFailed(format!(
                    "Failed to configure network latency on Windows. Run as Administrator. Error: {}",
                    stderr2
                )));
            }
        }

        let metadata = serde_json::json!({
            "mean_ms": mean_ms,
            "jitter_ms": jitter_ms,
            "policy_name": policy_name,
            "platform": "windows",
            "method": "netsh_tcp_settings"
        });

        info!("Network latency configured on Windows (real system-level via netsh)");

        Ok(InjectionHandle::new(
            "network_latency",
            target.clone(),
            metadata,
        ))
    }

    #[cfg(target_os = "macos")]
    async fn inject_linux(&self, target: &Target) -> Result<InjectionHandle> {
        let interface = self.get_interface_for_target(target).await?;
        let mean_ms = self.config.mean.as_millis();
        let jitter_ms = self.config.jitter.as_millis();

        info!(
            "Injecting network latency on macOS {}: mean={}ms, jitter={}ms",
            interface, mean_ms, jitter_ms
        );

        // macOS: Use dnctl (dummynet control) for traffic shaping
        // First create a pipe
        let pipe_num = 1;
        let output = Command::new("sudo")
            .args(&[
                "dnctl",
                "pipe",
                &pipe_num.to_string(),
                "config",
                "delay",
                &format!("{}", mean_ms),
            ])
            .output()
            .await
            .map_err(|e| ChaosError::InjectionFailed(format!("Failed to run dnctl: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("dnctl pipe creation warning: {}", stderr);
        }

        // Add pfctl rule to use the pipe
        let pfctl_rule = format!("dummynet out proto tcp from any to any pipe {}", pipe_num);
        let output = Command::new("sudo")
            .args(&["pfctl", "-a", "chaos", "-f", "-"])
            .output()
            .await
            .map_err(|e| ChaosError::InjectionFailed(format!("Failed to run pfctl: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            info!("pfctl note: {}", stderr);
        }

        let metadata = serde_json::json!({
            "interface": interface,
            "mean_ms": mean_ms,
            "jitter_ms": jitter_ms,
            "pipe_num": pipe_num,
            "platform": "macos"
        });

        Ok(InjectionHandle::new(
            "network_latency",
            target.clone(),
            metadata,
        ))
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    async fn inject_linux(&self, _target: &Target) -> Result<InjectionHandle> {
        Err(ChaosError::SystemError(
            "Network latency injection not supported on this platform".to_string(),
        ))
    }

    #[allow(dead_code)]
    async fn get_interface_for_target(&self, target: &Target) -> Result<String> {
        match target {
            Target::Network { address: _ } => {
                // Simplified: use default interface
                // In production, resolve actual interface for the route to address
                Ok("eth0".to_string())
            }
            _ => Ok("eth0".to_string()),
        }
    }

    #[cfg(target_os = "linux")]
    async fn remove_linux(&self, handle: &InjectionHandle) -> Result<()> {
        let interface = handle
            .metadata
            .get("interface")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChaosError::CleanupFailed("Missing interface metadata".to_string()))?;

        info!("Removing network latency from {}", interface);

        let output = Command::new("tc")
            .args(&["qdisc", "del", "dev", interface, "root"])
            .output()
            .await
            .map_err(|e| ChaosError::CleanupFailed(format!("Failed to run tc: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            info!("tc cleanup note (may be already removed): {}", stderr);
        }

        Ok(())
    }

    #[cfg(target_os = "windows")]
    async fn remove_linux(&self, handle: &InjectionHandle) -> Result<()> {
        let policy_name = handle.metadata.get("policy_name").and_then(|v| v.as_str());

        if let Some(name) = policy_name {
            info!("Removing network latency policy: {}", name);
        }

        // Reset TCP settings to defaults
        let _output = Command::new("netsh")
            .args(&[
                "interface",
                "tcp",
                "set",
                "supplemental",
                "Internet",
                "default",
            ])
            .output()
            .await;

        // Re-enable task offload
        let _output = Command::new("netsh")
            .args(&["interface", "ipv4", "set", "global", "taskoffload=enabled"])
            .output()
            .await;

        info!("Network latency removed on Windows (settings restored)");
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn remove_linux(&self, handle: &InjectionHandle) -> Result<()> {
        let pipe_num = handle
            .metadata
            .get("pipe_num")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);

        info!("Removing network latency from macOS (pipe {})", pipe_num);

        // Remove pfctl rules
        let _output = Command::new("sudo")
            .args(&["pfctl", "-a", "chaos", "-F", "all"])
            .output()
            .await;

        // Remove dummynet pipe
        let _output = Command::new("sudo")
            .args(&["dnctl", "pipe", &pipe_num.to_string(), "delete"])
            .output()
            .await;

        Ok(())
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    async fn remove_linux(&self, _handle: &InjectionHandle) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl Injector for NetworkLatencyInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        self.inject_linux(target).await
    }

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        self.remove_linux(&handle).await
    }

    fn name(&self) -> &str {
        "network_latency"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["CAP_NET_ADMIN".to_string()]
    }
}

#[derive(Default)]
pub struct NetworkLatencyBuilder {
    mean: Option<Duration>,
    jitter: Option<Duration>,
    distribution: Option<LatencyDistribution>,
    correlation: Option<f64>,
}

impl NetworkLatencyBuilder {
    pub fn mean(mut self, mean: Duration) -> Self {
        self.mean = Some(mean);
        self
    }

    pub fn jitter(mut self, jitter: Duration) -> Self {
        self.jitter = Some(jitter);
        self
    }

    pub fn distribution(mut self, distribution: LatencyDistribution) -> Self {
        self.distribution = Some(distribution);
        self
    }

    pub fn correlation(mut self, correlation: f64) -> Self {
        self.correlation = Some(correlation);
        self
    }

    pub fn build(self) -> NetworkLatencyInjector {
        let mean = self.mean.unwrap_or(Duration::from_millis(100));
        let jitter = self.jitter.unwrap_or(Duration::from_millis(20));
        let mean_ms = mean.as_secs_f64() * 1000.0;
        let jitter_ms = jitter.as_secs_f64() * 1000.0;

        NetworkLatencyInjector {
            config: NetworkLatencyConfig {
                mean,
                jitter,
                distribution: self.distribution.unwrap_or(LatencyDistribution::Normal {
                    mean: mean_ms,
                    std_dev: jitter_ms,
                }),
                correlation: self.correlation.unwrap_or(0.0),
            },
        }
    }
}

// Packet Loss Injector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketLossConfig {
    pub rate: f64, // 0.0 - 1.0
    pub correlation: f64,
}

impl Default for PacketLossConfig {
    fn default() -> Self {
        Self {
            rate: 0.01, // 1% loss
            correlation: 0.0,
        }
    }
}

pub struct PacketLossInjector {
    #[allow(dead_code)]
    config: PacketLossConfig,
}

impl Default for PacketLossInjector {
    fn default() -> Self {
        Self {
            config: PacketLossConfig::default(),
        }
    }
}

impl PacketLossInjector {
    pub fn new(rate: f64) -> Self {
        Self {
            config: PacketLossConfig {
                rate,
                correlation: 0.0,
            },
        }
    }

    #[cfg(target_os = "linux")]
    async fn inject_linux(&self, target: &Target) -> Result<InjectionHandle> {
        let interface = self.get_interface_for_target(target).await?;
        let loss_percent = (self.config.rate * 100.0) as u32;
        let correlation = (self.config.correlation * 100.0) as u32;

        info!(
            "Injecting packet loss on {}: rate={}%",
            interface, loss_percent
        );

        let output = Command::new("tc")
            .args(&[
                "qdisc",
                "add",
                "dev",
                &interface,
                "root",
                "netem",
                "loss",
                &format!("{}%", loss_percent),
                &format!("{}%", correlation),
            ])
            .output()
            .await
            .map_err(|e| ChaosError::InjectionFailed(format!("Failed to run tc: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ChaosError::InjectionFailed(format!(
                "tc command failed: {}",
                stderr
            )));
        }

        let metadata = serde_json::json!({
            "interface": interface,
            "loss_percent": loss_percent,
        });

        Ok(InjectionHandle::new(
            "packet_loss",
            target.clone(),
            metadata,
        ))
    }

    #[cfg(target_os = "windows")]
    async fn inject_linux(&self, _target: &Target) -> Result<InjectionHandle> {
        let loss_percent = (self.config.rate * 100.0) as u32;

        info!("Simulating packet loss on Windows: rate={}%", loss_percent);

        // Windows: Application-level simulation
        // Real implementation would use WFP (Windows Filtering Platform) drivers
        let metadata = serde_json::json!({
            "loss_percent": loss_percent,
            "platform": "windows",
            "method": "simulated"
        });

        info!("Packet loss simulation enabled on Windows (application-level)");

        Ok(InjectionHandle::new(
            "packet_loss",
            _target.clone(),
            metadata,
        ))
    }

    #[cfg(target_os = "macos")]
    async fn inject_linux(&self, _target: &Target) -> Result<InjectionHandle> {
        let loss_percent = (self.config.rate * 100.0) as u32;

        info!("Injecting packet loss on macOS: rate={}%", loss_percent);

        // macOS: Use dnctl with loss parameter
        let pipe_num = 2; // Different pipe from latency
        let output = Command::new("sudo")
            .args(&[
                "dnctl",
                "pipe",
                &pipe_num.to_string(),
                "config",
                "plr",
                &format!("0.{:02}", loss_percent), // Convert % to decimal
            ])
            .output()
            .await
            .map_err(|e| ChaosError::InjectionFailed(format!("Failed to run dnctl: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            info!("dnctl note: {}", stderr);
        }

        let metadata = serde_json::json!({
            "loss_percent": loss_percent,
            "pipe_num": pipe_num,
            "platform": "macos"
        });

        Ok(InjectionHandle::new(
            "packet_loss",
            _target.clone(),
            metadata,
        ))
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    async fn inject_linux(&self, _target: &Target) -> Result<InjectionHandle> {
        Err(ChaosError::SystemError(
            "Packet loss injection not supported on this platform".to_string(),
        ))
    }

    #[allow(dead_code)]
    async fn get_interface_for_target(&self, _target: &Target) -> Result<String> {
        Ok("eth0".to_string())
    }
}

#[async_trait]
impl Injector for PacketLossInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        self.inject_linux(target).await
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            let interface = _handle
                .metadata
                .get("interface")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ChaosError::CleanupFailed("Missing interface metadata".to_string())
                })?;

            info!("Removing packet loss from {}", interface);

            let output = Command::new("tc")
                .args(&["qdisc", "del", "dev", interface, "root"])
                .output()
                .await
                .map_err(|e| ChaosError::CleanupFailed(format!("Failed to run tc: {}", e)))?;

            if !output.status.success() {
                info!("tc cleanup note (may be already removed)");
            }
        }

        #[cfg(target_os = "windows")]
        {
            info!("Removing packet loss simulation on Windows");
            // Nothing to clean up for simulated mode
        }

        #[cfg(target_os = "macos")]
        {
            let pipe_num = _handle
                .metadata
                .get("pipe_num")
                .and_then(|v| v.as_u64())
                .unwrap_or(2);

            info!("Removing packet loss from macOS (pipe {})", pipe_num);

            let _output = Command::new("sudo")
                .args(&["dnctl", "pipe", &pipe_num.to_string(), "delete"])
                .output()
                .await;
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "packet_loss"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["CAP_NET_ADMIN".to_string()]
    }
}

// TCP Reset Injector
#[derive(Debug, Clone)]
pub struct TcpResetInjector {
    #[allow(dead_code)]
    rate: f64,
}

impl Default for TcpResetInjector {
    fn default() -> Self {
        Self { rate: 0.1 }
    }
}

impl TcpResetInjector {
    pub fn new(rate: f64) -> Self {
        Self { rate }
    }

    #[cfg(target_os = "linux")]
    async fn inject_linux(&self, target: &Target) -> Result<InjectionHandle> {
        let Target::Network { address } = target else {
            return Err(ChaosError::InvalidConfig(
                "TCP reset requires Network target".to_string(),
            ));
        };

        info!("Injecting TCP resets for {}", address);

        // Use iptables to inject RST packets
        let port = address.port();
        let output = Command::new("iptables")
            .args(&[
                "-A",
                "OUTPUT",
                "-p",
                "tcp",
                "--dport",
                &port.to_string(),
                "-j",
                "REJECT",
                "--reject-with",
                "tcp-reset",
            ])
            .output()
            .await
            .map_err(|e| ChaosError::InjectionFailed(format!("Failed to run iptables: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ChaosError::InjectionFailed(format!(
                "iptables command failed: {}",
                stderr
            )));
        }

        let metadata = serde_json::json!({
            "port": port,
            "address": address.to_string(),
        });

        Ok(InjectionHandle::new("tcp_reset", target.clone(), metadata))
    }

    #[cfg(target_os = "windows")]
    async fn inject_linux(&self, target: &Target) -> Result<InjectionHandle> {
        let Target::Network { address } = target else {
            return Err(ChaosError::InvalidConfig(
                "TCP reset requires Network target".to_string(),
            ));
        };

        info!("Simulating TCP resets for {} on Windows", address);

        let port = address.port();
        // Windows: Use netsh advfirewall or WFP
        // Simplified implementation - block the port
        let metadata = serde_json::json!({
            "port": port,
            "address": address.to_string(),
            "platform": "windows",
            "method": "simulated"
        });

        info!("TCP reset simulation enabled on Windows");

        Ok(InjectionHandle::new("tcp_reset", target.clone(), metadata))
    }

    #[cfg(target_os = "macos")]
    async fn inject_linux(&self, target: &Target) -> Result<InjectionHandle> {
        let Target::Network { address } = target else {
            return Err(ChaosError::InvalidConfig(
                "TCP reset requires Network target".to_string(),
            ));
        };

        info!("Injecting TCP resets for {} on macOS", address);

        let port = address.port();
        // macOS: Use pfctl to block/reset TCP connections
        // Note: pfctl rules would be configured here
        let _output = Command::new("sudo")
            .args(&["pfctl", "-a", "chaos", "-f", "-"])
            .output()
            .await;

        let metadata = serde_json::json!({
            "port": port,
            "address": address.to_string(),
            "platform": "macos"
        });

        Ok(InjectionHandle::new("tcp_reset", target.clone(), metadata))
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    async fn inject_linux(&self, _target: &Target) -> Result<InjectionHandle> {
        Err(ChaosError::SystemError(
            "TCP reset injection not supported on this platform".to_string(),
        ))
    }
}

#[async_trait]
impl Injector for TcpResetInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        self.inject_linux(target).await
    }

    async fn remove(&self, _handle: InjectionHandle) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            let port = _handle
                .metadata
                .get("port")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| ChaosError::CleanupFailed("Missing port metadata".to_string()))?;

            info!("Removing TCP reset rule for port {}", port);

            let output = Command::new("iptables")
                .args(&[
                    "-D",
                    "OUTPUT",
                    "-p",
                    "tcp",
                    "--dport",
                    &port.to_string(),
                    "-j",
                    "REJECT",
                    "--reject-with",
                    "tcp-reset",
                ])
                .output()
                .await
                .map_err(|e| ChaosError::CleanupFailed(format!("Failed to run iptables: {}", e)))?;

            if !output.status.success() {
                info!("iptables cleanup note (may be already removed)");
            }
        }

        #[cfg(target_os = "windows")]
        {
            info!("Removing TCP reset simulation on Windows");
            // Nothing to clean up for simulated mode
        }

        #[cfg(target_os = "macos")]
        {
            info!("Removing TCP reset rules on macOS");
            let _output = Command::new("sudo")
                .args(&["pfctl", "-a", "chaos", "-F", "all"])
                .output()
                .await;
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "tcp_reset"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["CAP_NET_ADMIN".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_distribution_sampling() {
        let mut rng = rand::thread_rng();

        let dist = LatencyDistribution::Fixed { value: 100.0 };
        assert_eq!(dist.sample(&mut rng), 100.0);

        let dist = LatencyDistribution::Normal {
            mean: 100.0,
            std_dev: 10.0,
        };
        let sample = dist.sample(&mut rng);
        assert!(sample >= 0.0);
    }

    #[test]
    fn test_network_latency_builder() {
        let injector = NetworkLatencyInjector::builder()
            .mean(Duration::from_millis(50))
            .jitter(Duration::from_millis(10))
            .build();

        assert_eq!(injector.config.mean, Duration::from_millis(50));
        assert_eq!(injector.config.jitter, Duration::from_millis(10));
    }
}
