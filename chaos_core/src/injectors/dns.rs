use crate::{
    error::{ChaosError, Result},
    handle::InjectionHandle,
    injectors::{Injector, InjectorStatus},
    target::Target,
};
use async_trait::async_trait;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{net::UdpSocket, sync::Mutex, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

const MAX_DNS_PACKET_SIZE: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DnsFaultMode {
    Latency { delay: Duration },
    NxDomain,
    Spoof { fake_ip: String },
    Blackhole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsFaultConfig {
    pub listen: SocketAddr,
    pub upstream: SocketAddr,
    pub domain_pattern: String,
    pub fault_mode: DnsFaultMode,
    pub failure_rate: f64,
    #[serde(default = "default_upstream_timeout")]
    pub upstream_timeout: Duration,
}

fn default_upstream_timeout() -> Duration {
    Duration::from_secs(2)
}

impl Default for DnsFaultConfig {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1:0".parse().expect("valid loopback address"),
            upstream: "1.1.1.1:53".parse().expect("valid DNS address"),
            domain_pattern: "*".to_string(),
            fault_mode: DnsFaultMode::NxDomain,
            failure_rate: 1.0,
            upstream_timeout: default_upstream_timeout(),
        }
    }
}

impl DnsFaultConfig {
    pub fn validate(&self) -> Result<()> {
        if self.listen == self.upstream {
            return Err(ChaosError::InvalidConfig(
                "DNS listen and upstream addresses must differ".to_string(),
            ));
        }
        if self.domain_pattern.trim().is_empty() {
            return Err(ChaosError::InvalidConfig(
                "DNS domain pattern cannot be empty".to_string(),
            ));
        }
        if !self.failure_rate.is_finite() || !(0.0..=1.0).contains(&self.failure_rate) {
            return Err(ChaosError::InvalidConfig(
                "DNS failure rate must be between 0.0 and 1.0".to_string(),
            ));
        }
        if self.upstream_timeout.is_zero() {
            return Err(ChaosError::InvalidConfig(
                "DNS upstream timeout must be greater than zero".to_string(),
            ));
        }
        if let DnsFaultMode::Spoof { fake_ip } = &self.fault_mode {
            fake_ip.parse::<IpAddr>().map_err(|error| {
                ChaosError::InvalidConfig(format!("Invalid spoof IP '{}': {}", fake_ip, error))
            })?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsFaultMetrics {
    pub queries: u64,
    pub injected_queries: u64,
    pub forwarded_queries: u64,
    pub dropped_queries: u64,
    pub upstream_errors: u64,
}

#[derive(Default)]
struct DnsMetricsState {
    queries: AtomicU64,
    injected_queries: AtomicU64,
    forwarded_queries: AtomicU64,
    dropped_queries: AtomicU64,
    upstream_errors: AtomicU64,
}

impl DnsMetricsState {
    fn snapshot(&self) -> DnsFaultMetrics {
        DnsFaultMetrics {
            queries: self.queries.load(Ordering::Relaxed),
            injected_queries: self.injected_queries.load(Ordering::Relaxed),
            forwarded_queries: self.forwarded_queries.load(Ordering::Relaxed),
            dropped_queries: self.dropped_queries.load(Ordering::Relaxed),
            upstream_errors: self.upstream_errors.load(Ordering::Relaxed),
        }
    }
}

struct DnsFaultServer {
    listen: SocketAddr,
    cancellation: CancellationToken,
    task: JoinHandle<()>,
    metrics: Arc<DnsMetricsState>,
}

impl DnsFaultServer {
    async fn start(config: DnsFaultConfig) -> Result<Self> {
        config.validate()?;
        let socket = UdpSocket::bind(config.listen).await?;
        let listen = socket.local_addr()?;
        let cancellation = CancellationToken::new();
        let shutdown = cancellation.clone();
        let metrics = Arc::new(DnsMetricsState::default());
        let task_metrics = metrics.clone();
        let task = tokio::spawn(async move {
            run_dns_proxy(socket, config, shutdown, task_metrics).await;
        });

        Ok(Self {
            listen,
            cancellation,
            task,
            metrics,
        })
    }

    async fn shutdown(self) -> Result<DnsFaultMetrics> {
        self.cancellation.cancel();
        self.task.await.map_err(|error| {
            ChaosError::CleanupFailed(format!("DNS proxy task failed: {}", error))
        })?;
        Ok(self.metrics.snapshot())
    }
}

async fn run_dns_proxy(
    socket: UdpSocket,
    config: DnsFaultConfig,
    cancellation: CancellationToken,
    metrics: Arc<DnsMetricsState>,
) {
    let socket = Arc::new(socket);
    let mut buffer = vec![0u8; MAX_DNS_PACKET_SIZE];
    loop {
        let received = tokio::select! {
            _ = cancellation.cancelled() => break,
            received = socket.recv_from(&mut buffer) => received,
        };
        let (length, peer) = match received {
            Ok(received) => received,
            Err(error) => {
                warn!("DNS proxy receive failed: {}", error);
                continue;
            }
        };

        metrics.queries.fetch_add(1, Ordering::Relaxed);
        let query = buffer[..length].to_vec();
        let request_socket = socket.clone();
        let request_config = config.clone();
        let request_metrics = metrics.clone();
        tokio::spawn(async move {
            if let Err(error) = handle_dns_query(
                request_socket,
                peer,
                query,
                request_config,
                request_metrics.clone(),
            )
            .await
            {
                request_metrics
                    .upstream_errors
                    .fetch_add(1, Ordering::Relaxed);
                debug!("DNS query from {} failed: {}", peer, error);
            }
        });
    }
}

async fn handle_dns_query(
    listen_socket: Arc<UdpSocket>,
    peer: SocketAddr,
    query: Vec<u8>,
    config: DnsFaultConfig,
    metrics: Arc<DnsMetricsState>,
) -> Result<()> {
    let (domain, question_end) = parse_question(&query)?;
    let inject = domain_matches(&config.domain_pattern, &domain) && selected(config.failure_rate);

    if inject {
        metrics.injected_queries.fetch_add(1, Ordering::Relaxed);
        match &config.fault_mode {
            DnsFaultMode::Latency { delay } => {
                tokio::time::sleep(*delay).await;
            }
            DnsFaultMode::NxDomain => {
                let response = nxdomain_response(&query, question_end)?;
                listen_socket.send_to(&response, peer).await?;
                return Ok(());
            }
            DnsFaultMode::Spoof { fake_ip } => {
                let response = spoof_response(
                    &query,
                    question_end,
                    fake_ip.parse().map_err(|error| {
                        ChaosError::InvalidConfig(format!("Invalid spoof IP: {}", error))
                    })?,
                )?;
                listen_socket.send_to(&response, peer).await?;
                return Ok(());
            }
            DnsFaultMode::Blackhole => {
                metrics.dropped_queries.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
        }
    }

    let upstream = UdpSocket::bind(match config.upstream {
        SocketAddr::V4(_) => "0.0.0.0:0",
        SocketAddr::V6(_) => "[::]:0",
    })
    .await?;
    upstream.send_to(&query, config.upstream).await?;
    let mut response = vec![0u8; MAX_DNS_PACKET_SIZE];
    let (length, _) =
        tokio::time::timeout(config.upstream_timeout, upstream.recv_from(&mut response))
            .await
            .map_err(|_| ChaosError::NetworkError("DNS upstream timed out".to_string()))??;
    metrics.forwarded_queries.fetch_add(1, Ordering::Relaxed);
    listen_socket.send_to(&response[..length], peer).await?;
    Ok(())
}

fn parse_question(packet: &[u8]) -> Result<(String, usize)> {
    if packet.len() < 12 {
        return Err(ChaosError::NetworkError(
            "DNS packet is shorter than its header".to_string(),
        ));
    }
    let questions = u16::from_be_bytes([packet[4], packet[5]]);
    if questions == 0 {
        return Err(ChaosError::NetworkError(
            "DNS packet has no question".to_string(),
        ));
    }

    let mut labels = Vec::new();
    let mut offset = 12usize;
    loop {
        let length = *packet
            .get(offset)
            .ok_or_else(|| ChaosError::NetworkError("Truncated DNS name".to_string()))?
            as usize;
        offset += 1;
        if length == 0 {
            break;
        }
        if length > 63 || offset + length > packet.len() {
            return Err(ChaosError::NetworkError(
                "Invalid DNS label length".to_string(),
            ));
        }
        labels.push(String::from_utf8_lossy(&packet[offset..offset + length]).to_string());
        offset += length;
    }
    let question_end = offset
        .checked_add(4)
        .ok_or_else(|| ChaosError::NetworkError("DNS question offset overflow".to_string()))?;
    if question_end > packet.len() {
        return Err(ChaosError::NetworkError(
            "Truncated DNS question".to_string(),
        ));
    }
    Ok((labels.join(".").to_ascii_lowercase(), question_end))
}

fn response_prefix(query: &[u8], question_end: usize, rcode: u8, answers: u16) -> Result<Vec<u8>> {
    if question_end > query.len() || query.len() < 12 {
        return Err(ChaosError::NetworkError(
            "Invalid DNS question boundary".to_string(),
        ));
    }
    let mut response = query[..question_end].to_vec();
    response[2] |= 0x80;
    response[3] = (response[3] & 0xf0) | (rcode & 0x0f) | 0x80;
    response[6..8].copy_from_slice(&answers.to_be_bytes());
    response[8..12].fill(0);
    Ok(response)
}

fn nxdomain_response(query: &[u8], question_end: usize) -> Result<Vec<u8>> {
    response_prefix(query, question_end, 3, 0)
}

fn spoof_response(query: &[u8], question_end: usize, ip: IpAddr) -> Result<Vec<u8>> {
    let mut response = response_prefix(query, question_end, 0, 1)?;
    response.extend_from_slice(&[0xc0, 0x0c]);
    match ip {
        IpAddr::V4(address) => {
            response.extend_from_slice(&1u16.to_be_bytes());
            response.extend_from_slice(&1u16.to_be_bytes());
            response.extend_from_slice(&30u32.to_be_bytes());
            response.extend_from_slice(&4u16.to_be_bytes());
            response.extend_from_slice(&address.octets());
        }
        IpAddr::V6(address) => {
            response.extend_from_slice(&28u16.to_be_bytes());
            response.extend_from_slice(&1u16.to_be_bytes());
            response.extend_from_slice(&30u32.to_be_bytes());
            response.extend_from_slice(&16u16.to_be_bytes());
            response.extend_from_slice(&address.octets());
        }
    }
    Ok(response)
}

fn domain_matches(pattern: &str, domain: &str) -> bool {
    let pattern = pattern.trim_end_matches('.').to_ascii_lowercase();
    let domain = domain.trim_end_matches('.').to_ascii_lowercase();
    pattern == "*"
        || pattern == domain
        || pattern
            .strip_prefix("*.")
            .is_some_and(|suffix| domain == suffix || domain.ends_with(&format!(".{}", suffix)))
}

fn selected(rate: f64) -> bool {
    rate >= 1.0 || (rate > 0.0 && rand::thread_rng().gen_bool(rate))
}

pub struct DnsFaultInjector {
    config: DnsFaultConfig,
    active: Arc<Mutex<HashMap<String, DnsFaultServer>>>,
}

impl Default for DnsFaultInjector {
    fn default() -> Self {
        Self::new(DnsFaultConfig::default())
    }
}

impl DnsFaultInjector {
    pub fn new(config: DnsFaultConfig) -> Self {
        Self {
            config,
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn builder() -> DnsFaultBuilder {
        DnsFaultBuilder::default()
    }

    pub async fn metrics(&self, handle_id: &str) -> Option<DnsFaultMetrics> {
        self.active
            .lock()
            .await
            .get(handle_id)
            .map(|server| server.metrics.snapshot())
    }
}

#[async_trait]
impl Injector for DnsFaultInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        let Target::Network { address } = target else {
            return Err(ChaosError::InvalidConfig(
                "dns_fault requires its upstream as a network target".to_string(),
            ));
        };
        if *address != self.config.upstream {
            return Err(ChaosError::InvalidConfig(format!(
                "DNS upstream {} does not match target {}",
                self.config.upstream, address
            )));
        }

        let server = DnsFaultServer::start(self.config.clone()).await?;
        let handle = InjectionHandle::new(
            self.name(),
            target.clone(),
            serde_json::json!({
                "listen": server.listen,
                "upstream": self.config.upstream,
                "domain_pattern": self.config.domain_pattern,
                "fault_mode": self.config.fault_mode,
                "rootless": true,
            }),
        );
        self.active.lock().await.insert(handle.id.clone(), server);
        Ok(handle)
    }

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        if let Some(server) = self.active.lock().await.remove(&handle.id) {
            let metrics = server.shutdown().await?;
            info!("DNS fault proxy {} stopped: {:?}", handle.id, metrics);
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "dns_fault"
    }

    fn status(&self) -> InjectorStatus {
        InjectorStatus::Stable
    }

    async fn validate(&self) -> Result<()> {
        self.config.validate()
    }
}

#[derive(Default)]
pub struct DnsFaultBuilder {
    listen: Option<SocketAddr>,
    upstream: Option<SocketAddr>,
    domain_pattern: Option<String>,
    fault_mode: Option<DnsFaultMode>,
    failure_rate: Option<f64>,
    upstream_timeout: Option<Duration>,
}

impl DnsFaultBuilder {
    pub fn listen(mut self, address: SocketAddr) -> Self {
        self.listen = Some(address);
        self
    }

    pub fn upstream(mut self, address: SocketAddr) -> Self {
        self.upstream = Some(address);
        self
    }

    pub fn domain_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.domain_pattern = Some(pattern.into());
        self
    }

    pub fn fault_mode(mut self, mode: DnsFaultMode) -> Self {
        self.fault_mode = Some(mode);
        self
    }

    pub fn failure_rate(mut self, rate: f64) -> Self {
        self.failure_rate = Some(rate.clamp(0.0, 1.0));
        self
    }

    pub fn upstream_timeout(mut self, timeout: Duration) -> Self {
        self.upstream_timeout = Some(timeout);
        self
    }

    pub fn build(self) -> DnsFaultInjector {
        let defaults = DnsFaultConfig::default();
        DnsFaultInjector::new(DnsFaultConfig {
            listen: self.listen.unwrap_or(defaults.listen),
            upstream: self.upstream.unwrap_or(defaults.upstream),
            domain_pattern: self.domain_pattern.unwrap_or_else(|| "*".to_string()),
            fault_mode: self.fault_mode.unwrap_or(DnsFaultMode::NxDomain),
            failure_rate: self.failure_rate.unwrap_or(1.0),
            upstream_timeout: self.upstream_timeout.unwrap_or(defaults.upstream_timeout),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(domain: &str) -> Vec<u8> {
        let mut packet = vec![0x12, 0x34, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
        for label in domain.split('.') {
            packet.push(label.len() as u8);
            packet.extend_from_slice(label.as_bytes());
        }
        packet.extend_from_slice(&[0, 0, 1, 0, 1]);
        packet
    }

    #[tokio::test]
    async fn spoofed_answer_is_observable_and_proxy_is_removed() {
        let upstream = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let upstream_address = upstream.local_addr().unwrap();
        let injector = DnsFaultInjector::builder()
            .listen("127.0.0.1:0".parse().unwrap())
            .upstream(upstream_address)
            .domain_pattern("*.internal")
            .fault_mode(DnsFaultMode::Spoof {
                fake_ip: "127.0.0.42".to_string(),
            })
            .build();

        let target = Target::network(upstream_address);
        let handle = injector.inject(&target).await.unwrap();
        let listen: SocketAddr = handle.metadata["listen"].as_str().unwrap().parse().unwrap();
        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        client.send_to(&query("db.internal"), listen).await.unwrap();
        let mut response = [0u8; 512];
        let (length, _) =
            tokio::time::timeout(Duration::from_secs(1), client.recv_from(&mut response))
                .await
                .unwrap()
                .unwrap();

        assert_eq!(&response[length - 4..length], &[127, 0, 0, 42]);
        assert_eq!(
            injector.metrics(&handle.id).await.unwrap().injected_queries,
            1
        );
        injector.remove(handle).await.unwrap();
        client.send_to(&query("db.internal"), listen).await.unwrap();
        assert!(!matches!(
            tokio::time::timeout(Duration::from_millis(100), client.recv_from(&mut response)).await,
            Ok(Ok(_))
        ));
    }

    #[tokio::test]
    async fn nxdomain_response_has_real_dns_error_code() {
        let packet = query("missing.example");
        let (_, end) = parse_question(&packet).unwrap();
        let response = nxdomain_response(&packet, end).unwrap();
        assert_eq!(response[3] & 0x0f, 3);
        assert_eq!(&response[6..8], &[0, 0]);
    }

    #[test]
    fn wildcard_matches_suffix_but_not_lookalikes() {
        assert!(domain_matches("*.example.com", "api.example.com"));
        assert!(domain_matches("*.example.com", "example.com"));
        assert!(!domain_matches("*.example.com", "notexample.com"));
    }
}
