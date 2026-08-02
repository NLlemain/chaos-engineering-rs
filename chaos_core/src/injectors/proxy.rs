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
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
    task::JoinHandle,
    time::Instant,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyDirection {
    Upstream,
    Downstream,
    Both,
}

impl ProxyDirection {
    fn applies_to(self, direction: Self) -> bool {
        self == Self::Both || self == direction
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProxyToxic {
    Latency { delay_ms: u64, jitter_ms: u64 },
    Bandwidth { bytes_per_second: u64 },
    Timeout { timeout_ms: u64 },
    SlowClose { delay_ms: u64 },
    LimitData { bytes: u64 },
    Partition,
    Corrupt { probability: f64 },
    Duplicate { probability: f64 },
    Reorder { probability: f64, delay_ms: u64 },
    ConnectionLimit { connections: u64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirectedToxic {
    pub direction: ProxyDirection,
    #[serde(default = "default_toxicity")]
    pub toxicity: f64,
    #[serde(flatten)]
    pub toxic: ProxyToxic,
}

impl DirectedToxic {
    pub fn new(direction: ProxyDirection, toxic: ProxyToxic) -> Self {
        Self {
            direction,
            toxicity: 1.0,
            toxic,
        }
    }

    pub fn with_toxicity(mut self, toxicity: f64) -> Self {
        self.toxicity = toxicity;
        self
    }
}

fn default_toxicity() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyProxyConfig {
    pub listen: SocketAddr,
    pub upstream: SocketAddr,
    pub toxics: Vec<DirectedToxic>,
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,
}

fn default_buffer_size() -> usize {
    16 * 1024
}

impl DependencyProxyConfig {
    pub fn new(listen: SocketAddr, upstream: SocketAddr) -> Self {
        Self {
            listen,
            upstream,
            toxics: Vec::new(),
            buffer_size: default_buffer_size(),
        }
    }

    pub fn with_toxic(mut self, toxic: DirectedToxic) -> Self {
        self.toxics.push(toxic);
        self
    }

    pub fn validate(&self) -> Result<()> {
        if self.listen == self.upstream {
            return Err(ChaosError::InvalidConfig(
                "Proxy listen and upstream addresses must differ".to_string(),
            ));
        }
        if self.toxics.is_empty() {
            return Err(ChaosError::InvalidConfig(
                "Dependency proxy requires at least one toxic".to_string(),
            ));
        }
        if self.buffer_size == 0 || self.buffer_size > 1024 * 1024 {
            return Err(ChaosError::InvalidConfig(
                "Proxy buffer_size must be between 1 byte and 1 MiB".to_string(),
            ));
        }

        for toxic in &self.toxics {
            validate_probability("toxicity", toxic.toxicity)?;
            match toxic.toxic {
                ProxyToxic::Bandwidth {
                    bytes_per_second: 0,
                } => {
                    return Err(ChaosError::InvalidConfig(
                        "Bandwidth bytes_per_second must be greater than zero".to_string(),
                    ));
                }
                ProxyToxic::Timeout { timeout_ms: 0 } => {
                    return Err(ChaosError::InvalidConfig(
                        "Timeout must be greater than zero".to_string(),
                    ));
                }
                ProxyToxic::LimitData { bytes: 0 } => {
                    return Err(ChaosError::InvalidConfig(
                        "Data limit must be greater than zero".to_string(),
                    ));
                }
                ProxyToxic::ConnectionLimit { connections: 0 } => {
                    return Err(ChaosError::InvalidConfig(
                        "Connection limit must be greater than zero".to_string(),
                    ));
                }
                ProxyToxic::Corrupt { probability }
                | ProxyToxic::Duplicate { probability }
                | ProxyToxic::Reorder { probability, .. } => {
                    validate_probability("probability", probability)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn validate_probability(name: &str, value: f64) -> Result<()> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(ChaosError::InvalidConfig(format!(
            "{} must be between 0.0 and 1.0",
            name
        )))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyMetricsSnapshot {
    pub accepted_connections: u64,
    pub rejected_connections: u64,
    pub active_connections: u64,
    pub upstream_bytes: u64,
    pub downstream_bytes: u64,
    pub disruptions: u64,
}

#[derive(Default)]
struct ProxyMetrics {
    accepted_connections: AtomicU64,
    rejected_connections: AtomicU64,
    active_connections: AtomicU64,
    upstream_bytes: AtomicU64,
    downstream_bytes: AtomicU64,
    disruptions: AtomicU64,
}

impl ProxyMetrics {
    fn snapshot(&self) -> ProxyMetricsSnapshot {
        ProxyMetricsSnapshot {
            accepted_connections: self.accepted_connections.load(Ordering::Relaxed),
            rejected_connections: self.rejected_connections.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            upstream_bytes: self.upstream_bytes.load(Ordering::Relaxed),
            downstream_bytes: self.downstream_bytes.load(Ordering::Relaxed),
            disruptions: self.disruptions.load(Ordering::Relaxed),
        }
    }

    fn add_bytes(&self, direction: ProxyDirection, count: usize) {
        let counter = match direction {
            ProxyDirection::Upstream => &self.upstream_bytes,
            ProxyDirection::Downstream => &self.downstream_bytes,
            ProxyDirection::Both => return,
        };
        counter.fetch_add(count as u64, Ordering::Relaxed);
    }
}

struct ProxyServer {
    listen: SocketAddr,
    cancellation: CancellationToken,
    task: JoinHandle<()>,
    metrics: Arc<ProxyMetrics>,
}

impl ProxyServer {
    async fn start(config: DependencyProxyConfig) -> Result<Self> {
        config.validate()?;
        let listener = TcpListener::bind(config.listen).await?;
        let listen = listener.local_addr()?;
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let metrics = Arc::new(ProxyMetrics::default());
        let task_metrics = metrics.clone();

        let task = tokio::spawn(async move {
            run_proxy(listener, config, task_cancellation, task_metrics).await;
        });

        Ok(Self {
            listen,
            cancellation,
            task,
            metrics,
        })
    }

    async fn shutdown(self) -> Result<ProxyMetricsSnapshot> {
        self.cancellation.cancel();
        self.task.await.map_err(|error| {
            ChaosError::CleanupFailed(format!("Proxy task failed during shutdown: {}", error))
        })?;
        Ok(self.metrics.snapshot())
    }
}

async fn run_proxy(
    listener: TcpListener,
    config: DependencyProxyConfig,
    cancellation: CancellationToken,
    metrics: Arc<ProxyMetrics>,
) {
    info!(
        "Dependency proxy listening on {}",
        listener.local_addr().unwrap_or(config.listen)
    );
    loop {
        let accepted = tokio::select! {
            _ = cancellation.cancelled() => break,
            accepted = listener.accept() => accepted,
        };

        let (client, peer) = match accepted {
            Ok(connection) => connection,
            Err(error) => {
                warn!("Dependency proxy accept failed: {}", error);
                continue;
            }
        };
        metrics.accepted_connections.fetch_add(1, Ordering::Relaxed);

        if let Some(limit) = selected_connection_limit(&config.toxics) {
            let active = metrics.active_connections.fetch_add(1, Ordering::Relaxed) + 1;
            if active > limit {
                metrics.active_connections.fetch_sub(1, Ordering::Relaxed);
                metrics.rejected_connections.fetch_add(1, Ordering::Relaxed);
                metrics.disruptions.fetch_add(1, Ordering::Relaxed);
                drop(client);
                continue;
            }
        } else {
            metrics.active_connections.fetch_add(1, Ordering::Relaxed);
        }

        let connection_config = config.clone();
        let connection_cancellation = cancellation.clone();
        let connection_metrics = metrics.clone();
        tokio::spawn(async move {
            let result = handle_connection(
                client,
                connection_config,
                connection_cancellation,
                connection_metrics.clone(),
            )
            .await;
            connection_metrics
                .active_connections
                .fetch_sub(1, Ordering::Relaxed);
            if let Err(error) = result {
                debug!("Proxy connection from {} ended: {}", peer, error);
            }
        });
    }
}

async fn handle_connection(
    client: TcpStream,
    config: DependencyProxyConfig,
    cancellation: CancellationToken,
    metrics: Arc<ProxyMetrics>,
) -> Result<()> {
    let upstream = tokio::select! {
        _ = cancellation.cancelled() => return Ok(()),
        result = TcpStream::connect(config.upstream) => result?,
    };

    let (client_reader, client_writer) = client.into_split();
    let (upstream_reader, upstream_writer) = upstream.into_split();
    let upstream_plan = CopyPlan::new(&config.toxics, ProxyDirection::Upstream);
    let downstream_plan = CopyPlan::new(&config.toxics, ProxyDirection::Downstream);

    let upstream_copy = copy_with_faults(
        client_reader,
        upstream_writer,
        upstream_plan,
        ProxyDirection::Upstream,
        config.buffer_size,
        cancellation.clone(),
        metrics.clone(),
    );
    let downstream_copy = copy_with_faults(
        upstream_reader,
        client_writer,
        downstream_plan,
        ProxyDirection::Downstream,
        config.buffer_size,
        cancellation,
        metrics,
    );
    tokio::try_join!(upstream_copy, downstream_copy)?;
    Ok(())
}

#[derive(Default)]
struct CopyPlan {
    latency: Option<(u64, u64)>,
    bandwidth: Option<u64>,
    timeout: Option<Duration>,
    slow_close: Duration,
    limit_data: Option<u64>,
    partition: bool,
    corrupt: f64,
    duplicate: f64,
    reorder: Option<(f64, Duration)>,
}

impl CopyPlan {
    fn new(toxics: &[DirectedToxic], direction: ProxyDirection) -> Self {
        let mut plan = Self::default();
        for directed in toxics {
            if !directed.direction.applies_to(direction) || !selected(directed.toxicity) {
                continue;
            }
            match directed.toxic {
                ProxyToxic::Latency {
                    delay_ms,
                    jitter_ms,
                } => plan.latency = Some((delay_ms, jitter_ms)),
                ProxyToxic::Bandwidth { bytes_per_second } => {
                    plan.bandwidth = Some(
                        plan.bandwidth
                            .map_or(bytes_per_second, |current| current.min(bytes_per_second)),
                    );
                }
                ProxyToxic::Timeout { timeout_ms } => {
                    let duration = Duration::from_millis(timeout_ms);
                    plan.timeout = Some(
                        plan.timeout
                            .map_or(duration, |current| current.min(duration)),
                    );
                }
                ProxyToxic::SlowClose { delay_ms } => {
                    plan.slow_close = plan.slow_close.max(Duration::from_millis(delay_ms));
                }
                ProxyToxic::LimitData { bytes } => {
                    plan.limit_data =
                        Some(plan.limit_data.map_or(bytes, |current| current.min(bytes)));
                }
                ProxyToxic::Partition => plan.partition = true,
                ProxyToxic::Corrupt { probability } => plan.corrupt = plan.corrupt.max(probability),
                ProxyToxic::Duplicate { probability } => {
                    plan.duplicate = plan.duplicate.max(probability);
                }
                ProxyToxic::Reorder {
                    probability,
                    delay_ms,
                } => plan.reorder = Some((probability, Duration::from_millis(delay_ms))),
                ProxyToxic::ConnectionLimit { .. } => {}
            }
        }
        plan
    }
}

fn selected_connection_limit(toxics: &[DirectedToxic]) -> Option<u64> {
    toxics
        .iter()
        .filter_map(|directed| match directed.toxic {
            ProxyToxic::ConnectionLimit { connections } if selected(directed.toxicity) => {
                Some(connections)
            }
            _ => None,
        })
        .min()
}

async fn copy_with_faults<R, W>(
    mut reader: R,
    mut writer: W,
    plan: CopyPlan,
    direction: ProxyDirection,
    buffer_size: usize,
    cancellation: CancellationToken,
    metrics: Arc<ProxyMetrics>,
) -> Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    if plan.partition {
        metrics.disruptions.fetch_add(1, Ordering::Relaxed);
        cancellation.cancelled().await;
        return Ok(());
    }

    let started = Instant::now();
    let mut transferred = 0u64;
    let mut buffer = vec![0u8; buffer_size];
    let mut pending: Option<Vec<u8>> = None;

    loop {
        let read = async {
            if let Some(timeout) = plan.timeout {
                let remaining = timeout.saturating_sub(started.elapsed());
                if remaining.is_zero() {
                    return Ok(0);
                }
                tokio::time::timeout(remaining, reader.read(&mut buffer))
                    .await
                    .map_err(|_| {
                        std::io::Error::new(std::io::ErrorKind::TimedOut, "toxic timeout")
                    })?
            } else {
                reader.read(&mut buffer).await
            }
        };

        let count = tokio::select! {
            _ = cancellation.cancelled() => return Ok(()),
            result = read => match result {
                Ok(count) => count,
                Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
                    metrics.disruptions.fetch_add(1, Ordering::Relaxed);
                    writer.shutdown().await?;
                    return Ok(());
                }
                Err(error) => return Err(error.into()),
            },
        };

        if count == 0 {
            if let Some(bytes) = pending.take() {
                writer.write_all(&bytes).await?;
                metrics.add_bytes(direction, bytes.len());
            }
            if !plan.slow_close.is_zero() {
                tokio::time::sleep(plan.slow_close).await;
            }
            writer.shutdown().await?;
            return Ok(());
        }

        let mut bytes = buffer[..count].to_vec();
        if let Some(limit) = plan.limit_data {
            let remaining = limit.saturating_sub(transferred);
            if remaining == 0 {
                metrics.disruptions.fetch_add(1, Ordering::Relaxed);
                writer.shutdown().await?;
                return Ok(());
            }
            bytes.truncate(
                usize::try_from(remaining.min(bytes.len() as u64)).unwrap_or(bytes.len()),
            );
        }

        if plan.corrupt > 0.0 {
            let mut rng = rand::thread_rng();
            for byte in &mut bytes {
                if rng.gen_bool(plan.corrupt) {
                    *byte ^= 1 << rng.gen_range(0..8);
                    metrics.disruptions.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        if let Some((delay_ms, jitter_ms)) = plan.latency {
            tokio::time::sleep(jittered_delay(delay_ms, jitter_ms)).await;
        }
        if let Some(bytes_per_second) = plan.bandwidth {
            let seconds = bytes.len() as f64 / bytes_per_second as f64;
            tokio::time::sleep(Duration::from_secs_f64(seconds)).await;
        }

        if let Some((probability, _)) = plan.reorder {
            if pending.is_none() && selected(probability) {
                pending = Some(bytes);
                metrics.disruptions.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        }

        writer.write_all(&bytes).await?;
        transferred += bytes.len() as u64;
        metrics.add_bytes(direction, bytes.len());

        if plan.duplicate > 0.0 && selected(plan.duplicate) {
            writer.write_all(&bytes).await?;
            metrics.add_bytes(direction, bytes.len());
            metrics.disruptions.fetch_add(1, Ordering::Relaxed);
        }

        if let Some(pending_bytes) = pending.take() {
            if let Some((_, delay)) = plan.reorder {
                tokio::time::sleep(delay).await;
            }
            writer.write_all(&pending_bytes).await?;
            transferred += pending_bytes.len() as u64;
            metrics.add_bytes(direction, pending_bytes.len());
        }

        if plan.limit_data.is_some_and(|limit| transferred >= limit) {
            metrics.disruptions.fetch_add(1, Ordering::Relaxed);
            writer.shutdown().await?;
            return Ok(());
        }
    }
}

fn selected(probability: f64) -> bool {
    probability >= 1.0 || (probability > 0.0 && rand::thread_rng().gen_bool(probability))
}

fn jittered_delay(delay_ms: u64, jitter_ms: u64) -> Duration {
    if jitter_ms == 0 {
        return Duration::from_millis(delay_ms);
    }
    let lower = delay_ms.saturating_sub(jitter_ms);
    let upper = delay_ms.saturating_add(jitter_ms);
    Duration::from_millis(rand::thread_rng().gen_range(lower..=upper))
}

pub struct DependencyProxyInjector {
    config: DependencyProxyConfig,
    active: Arc<Mutex<HashMap<String, ProxyServer>>>,
}

impl DependencyProxyInjector {
    pub fn new(config: DependencyProxyConfig) -> Self {
        Self {
            config,
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn metrics(&self, handle_id: &str) -> Option<ProxyMetricsSnapshot> {
        self.active
            .lock()
            .await
            .get(handle_id)
            .map(|server| server.metrics.snapshot())
    }
}

impl Default for DependencyProxyInjector {
    fn default() -> Self {
        let config = DependencyProxyConfig::new(
            "127.0.0.1:0".parse().expect("valid loopback address"),
            "127.0.0.1:1".parse().expect("valid loopback address"),
        )
        .with_toxic(DirectedToxic::new(
            ProxyDirection::Both,
            ProxyToxic::Latency {
                delay_ms: 100,
                jitter_ms: 0,
            },
        ));
        Self::new(config)
    }
}

#[async_trait]
impl Injector for DependencyProxyInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        let Target::Network { address } = target else {
            return Err(ChaosError::InvalidConfig(
                "dependency_proxy requires a network target".to_string(),
            ));
        };
        if *address != self.config.upstream {
            return Err(ChaosError::InvalidConfig(format!(
                "Proxy upstream {} does not match target {}",
                self.config.upstream, address
            )));
        }

        let server = ProxyServer::start(self.config.clone()).await?;
        let metadata = serde_json::json!({
            "listen": server.listen,
            "upstream": self.config.upstream,
            "toxics": self.config.toxics,
            "rootless": true,
        });
        let handle = InjectionHandle::new("dependency_proxy", target.clone(), metadata);
        self.active.lock().await.insert(handle.id.clone(), server);
        Ok(handle)
    }

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        let server = self.active.lock().await.remove(&handle.id);
        if let Some(server) = server {
            let metrics = server.shutdown().await?;
            info!("Dependency proxy {} stopped: {:?}", handle.id, metrics);
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "dependency_proxy"
    }

    fn status(&self) -> InjectorStatus {
        InjectorStatus::Stable
    }

    async fn validate(&self) -> Result<()> {
        self.config.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn echo_server() -> (SocketAddr, CancellationToken) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        tokio::spawn(async move {
            loop {
                let accepted = tokio::select! {
                    _ = task_cancellation.cancelled() => break,
                    accepted = listener.accept() => accepted,
                };
                let Ok((mut stream, _)) = accepted else {
                    continue;
                };
                tokio::spawn(async move {
                    let mut buffer = [0u8; 1024];
                    while let Ok(count) = stream.read(&mut buffer).await {
                        if count == 0 || stream.write_all(&buffer[..count]).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        (address, cancellation)
    }

    #[tokio::test]
    async fn latency_is_observable_and_removal_stops_the_proxy() {
        let (upstream, stop_echo) = echo_server().await;
        let config = DependencyProxyConfig::new("127.0.0.1:0".parse().unwrap(), upstream)
            .with_toxic(DirectedToxic::new(
                ProxyDirection::Downstream,
                ProxyToxic::Latency {
                    delay_ms: 80,
                    jitter_ms: 0,
                },
            ));
        let injector = DependencyProxyInjector::new(config);
        let handle = injector.inject(&Target::network(upstream)).await.unwrap();
        let listen: SocketAddr = handle.metadata["listen"].as_str().unwrap().parse().unwrap();

        let mut client = TcpStream::connect(listen).await.unwrap();
        let started = Instant::now();
        client.write_all(b"ping").await.unwrap();
        let mut response = [0u8; 4];
        client.read_exact(&mut response).await.unwrap();
        assert_eq!(&response, b"ping");
        assert!(started.elapsed() >= Duration::from_millis(70));
        assert_eq!(
            injector
                .metrics(&handle.id)
                .await
                .unwrap()
                .accepted_connections,
            1
        );

        injector.remove(handle).await.unwrap();
        assert!(TcpStream::connect(listen).await.is_err());

        let mut direct = TcpStream::connect(upstream).await.unwrap();
        direct.write_all(b"ok").await.unwrap();
        let mut direct_response = [0u8; 2];
        direct.read_exact(&mut direct_response).await.unwrap();
        assert_eq!(&direct_response, b"ok");
        stop_echo.cancel();
    }

    #[tokio::test]
    async fn data_limit_closes_the_stream_after_the_configured_bytes() {
        let (upstream, stop_echo) = echo_server().await;
        let config = DependencyProxyConfig::new("127.0.0.1:0".parse().unwrap(), upstream)
            .with_toxic(DirectedToxic::new(
                ProxyDirection::Downstream,
                ProxyToxic::LimitData { bytes: 4 },
            ));
        let injector = DependencyProxyInjector::new(config);
        let handle = injector.inject(&Target::network(upstream)).await.unwrap();
        let listen: SocketAddr = handle.metadata["listen"].as_str().unwrap().parse().unwrap();

        let mut client = TcpStream::connect(listen).await.unwrap();
        client.write_all(b"abcdefgh").await.unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        assert_eq!(&response, b"abcd");

        injector.remove(handle).await.unwrap();
        stop_echo.cancel();
    }

    #[tokio::test]
    async fn connection_limit_rejects_excess_clients() {
        let (upstream, stop_echo) = echo_server().await;
        let config = DependencyProxyConfig::new("127.0.0.1:0".parse().unwrap(), upstream)
            .with_toxic(DirectedToxic::new(
                ProxyDirection::Both,
                ProxyToxic::ConnectionLimit { connections: 1 },
            ));
        let injector = DependencyProxyInjector::new(config);
        let handle = injector.inject(&Target::network(upstream)).await.unwrap();
        let listen: SocketAddr = handle.metadata["listen"].as_str().unwrap().parse().unwrap();

        let _first = TcpStream::connect(listen).await.unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        let mut second = TcpStream::connect(listen).await.unwrap();
        let _ = second.write_all(b"rejected").await;
        let mut byte = [0u8; 1];
        let received =
            tokio::time::timeout(Duration::from_millis(200), second.read(&mut byte)).await;
        assert!(!matches!(received, Ok(Ok(1))));

        let metrics = injector.metrics(&handle.id).await.unwrap();
        assert_eq!(metrics.rejected_connections, 1);
        assert_eq!(metrics.disruptions, 1);
        injector.remove(handle).await.unwrap();
        stop_echo.cancel();
    }
}
