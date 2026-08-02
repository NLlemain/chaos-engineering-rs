use crate::{
    error::{ChaosError, Result},
    handle::InjectionHandle,
    injectors::{Injector, InjectorStatus},
    target::Target,
};
use async_trait::async_trait;
use rcgen::{date_time_ymd, BasicConstraints, CertificateParams, IsCa, KeyPair};
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
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::Mutex,
    task::JoinHandle,
};
use tokio_rustls::{
    rustls::{
        pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
        ServerConfig,
    },
    TlsAcceptor,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CryptoFaultType {
    CertExpired,
    UntrustedCa,
    IncompleteChain,
    HandshakeAbort,
    HandshakeDelay { delay: Duration },
    OcspOffline,
    SignatureCorrupt,
    EntropyStarvation,
}

impl CryptoFaultType {
    fn supported(&self) -> bool {
        matches!(
            self,
            Self::CertExpired
                | Self::UntrustedCa
                | Self::IncompleteChain
                | Self::HandshakeAbort
                | Self::HandshakeDelay { .. }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoFaultConfig {
    pub listen: SocketAddr,
    pub fault_type: CryptoFaultType,
    pub target_cert_domain: String,
}

impl Default for CryptoFaultConfig {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1:0".parse().expect("valid loopback address"),
            fault_type: CryptoFaultType::CertExpired,
            target_cert_domain: "localhost".to_string(),
        }
    }
}

impl CryptoFaultConfig {
    pub fn validate(&self) -> Result<()> {
        if self.target_cert_domain.trim().is_empty() {
            return Err(ChaosError::InvalidConfig(
                "TLS certificate domain cannot be empty".to_string(),
            ));
        }
        if !self.fault_type.supported() {
            return Err(ChaosError::InvalidConfig(
                "This cryptographic fault is planned; use cert_expired, untrusted_ca, incomplete_chain, handshake_abort, or handshake_delay"
                    .to_string(),
            ));
        }
        if matches!(
            self.fault_type,
            CryptoFaultType::HandshakeDelay { delay } if delay.is_zero()
        ) {
            return Err(ChaosError::InvalidConfig(
                "TLS handshake delay must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CryptoFaultMetrics {
    pub accepted_connections: u64,
    pub aborted_connections: u64,
    pub handshake_failures: u64,
    pub completed_handshakes: u64,
}

#[derive(Default)]
struct CryptoMetricsState {
    accepted_connections: AtomicU64,
    aborted_connections: AtomicU64,
    handshake_failures: AtomicU64,
    completed_handshakes: AtomicU64,
}

impl CryptoMetricsState {
    fn snapshot(&self) -> CryptoFaultMetrics {
        CryptoFaultMetrics {
            accepted_connections: self.accepted_connections.load(Ordering::Relaxed),
            aborted_connections: self.aborted_connections.load(Ordering::Relaxed),
            handshake_failures: self.handshake_failures.load(Ordering::Relaxed),
            completed_handshakes: self.completed_handshakes.load(Ordering::Relaxed),
        }
    }
}

struct CertificateProfile {
    server_config: Arc<ServerConfig>,
    trust_anchor: Option<Vec<u8>>,
}

struct CryptoFaultServer {
    listen: SocketAddr,
    cancellation: CancellationToken,
    task: JoinHandle<()>,
    metrics: Arc<CryptoMetricsState>,
    trust_anchor: Option<Vec<u8>>,
}

impl CryptoFaultServer {
    async fn start(config: CryptoFaultConfig) -> Result<Self> {
        config.validate()?;
        let listener = TcpListener::bind(config.listen).await?;
        let listen = listener.local_addr()?;
        let cancellation = CancellationToken::new();
        let shutdown = cancellation.clone();
        let metrics = Arc::new(CryptoMetricsState::default());
        let task_metrics = metrics.clone();
        let certificate = match config.fault_type {
            CryptoFaultType::HandshakeAbort | CryptoFaultType::HandshakeDelay { .. } => None,
            _ => Some(certificate_profile(&config)?),
        };
        let trust_anchor = certificate
            .as_ref()
            .and_then(|profile| profile.trust_anchor.clone());
        let task = tokio::spawn(async move {
            run_tls_fault(listener, config, certificate, shutdown, task_metrics).await;
        });

        Ok(Self {
            listen,
            cancellation,
            task,
            metrics,
            trust_anchor,
        })
    }

    async fn shutdown(self) -> Result<CryptoFaultMetrics> {
        self.cancellation.cancel();
        self.task.await.map_err(|error| {
            ChaosError::CleanupFailed(format!("TLS fault endpoint task failed: {}", error))
        })?;
        Ok(self.metrics.snapshot())
    }
}

async fn run_tls_fault(
    listener: TcpListener,
    config: CryptoFaultConfig,
    certificate: Option<CertificateProfile>,
    cancellation: CancellationToken,
    metrics: Arc<CryptoMetricsState>,
) {
    loop {
        let accepted = tokio::select! {
            _ = cancellation.cancelled() => break,
            accepted = listener.accept() => accepted,
        };
        let (stream, peer) = match accepted {
            Ok(value) => value,
            Err(error) => {
                warn!("TLS fault endpoint accept failed: {}", error);
                continue;
            }
        };
        metrics.accepted_connections.fetch_add(1, Ordering::Relaxed);
        let mode = config.fault_type.clone();
        let acceptor = certificate
            .as_ref()
            .map(|profile| TlsAcceptor::from(profile.server_config.clone()));
        let connection_metrics = metrics.clone();
        tokio::spawn(async move {
            if let Err(error) =
                handle_tls_connection(stream, mode, acceptor, connection_metrics).await
            {
                debug!("TLS fault connection from {} ended: {}", peer, error);
            }
        });
    }
}

async fn handle_tls_connection(
    mut stream: TcpStream,
    mode: CryptoFaultType,
    acceptor: Option<TlsAcceptor>,
    metrics: Arc<CryptoMetricsState>,
) -> Result<()> {
    match mode {
        CryptoFaultType::HandshakeAbort => {
            metrics.aborted_connections.fetch_add(1, Ordering::Relaxed);
            stream.shutdown().await?;
            return Ok(());
        }
        CryptoFaultType::HandshakeDelay { delay } => {
            tokio::time::sleep(delay).await;
            metrics.aborted_connections.fetch_add(1, Ordering::Relaxed);
            stream.shutdown().await?;
            return Ok(());
        }
        _ => {}
    }

    let acceptor = acceptor.ok_or_else(|| {
        ChaosError::InjectionFailed("TLS certificate profile was not created".to_string())
    })?;
    match acceptor.accept(stream).await {
        Ok(mut tls) => {
            metrics.completed_handshakes.fetch_add(1, Ordering::Relaxed);
            tls.write_all(
                b"HTTP/1.1 503 Service Unavailable\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
            )
            .await?;
            tls.shutdown().await?;
        }
        Err(error) => {
            metrics.handshake_failures.fetch_add(1, Ordering::Relaxed);
            return Err(ChaosError::NetworkError(error.to_string()));
        }
    }
    Ok(())
}

fn certificate_profile(config: &CryptoFaultConfig) -> Result<CertificateProfile> {
    match config.fault_type {
        CryptoFaultType::CertExpired => expired_certificate(&config.target_cert_domain),
        CryptoFaultType::UntrustedCa => untrusted_certificate(&config.target_cert_domain),
        CryptoFaultType::IncompleteChain => incomplete_chain(&config.target_cert_domain),
        _ => Err(ChaosError::InvalidConfig(
            "Fault mode does not use a certificate".to_string(),
        )),
    }
}

fn expired_certificate(domain: &str) -> Result<CertificateProfile> {
    let (root, root_key) = root_ca("Chaos Test Root")?;
    let leaf_key = generate_key()?;
    let mut leaf_params = CertificateParams::new(vec![domain.to_string()])
        .map_err(|error| ChaosError::InjectionFailed(error.to_string()))?;
    leaf_params.not_before = date_time_ymd(2019, 1, 1);
    leaf_params.not_after = date_time_ymd(2020, 1, 1);
    let leaf = leaf_params
        .signed_by(&leaf_key, &root, &root_key)
        .map_err(|error| ChaosError::InjectionFailed(error.to_string()))?;
    server_profile(
        vec![leaf.der().clone()],
        leaf_key,
        Some(root.der().to_vec()),
    )
}

fn untrusted_certificate(domain: &str) -> Result<CertificateProfile> {
    let key = generate_key()?;
    let cert = CertificateParams::new(vec![domain.to_string()])
        .map_err(|error| ChaosError::InjectionFailed(error.to_string()))?
        .self_signed(&key)
        .map_err(|error| ChaosError::InjectionFailed(error.to_string()))?;
    server_profile(vec![cert.der().clone()], key, None)
}

fn incomplete_chain(domain: &str) -> Result<CertificateProfile> {
    let (root, root_key) = root_ca("Chaos Test Root")?;
    let intermediate_key = generate_key()?;
    let mut intermediate_params = CertificateParams::new(vec!["chaos-intermediate".to_string()])
        .map_err(|error| ChaosError::InjectionFailed(error.to_string()))?;
    intermediate_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let intermediate = intermediate_params
        .signed_by(&intermediate_key, &root, &root_key)
        .map_err(|error| ChaosError::InjectionFailed(error.to_string()))?;
    let leaf_key = generate_key()?;
    let leaf = CertificateParams::new(vec![domain.to_string()])
        .map_err(|error| ChaosError::InjectionFailed(error.to_string()))?
        .signed_by(&leaf_key, &intermediate, &intermediate_key)
        .map_err(|error| ChaosError::InjectionFailed(error.to_string()))?;

    // Deliberately omit the intermediate certificate from the served chain.
    server_profile(
        vec![leaf.der().clone()],
        leaf_key,
        Some(root.der().to_vec()),
    )
}

fn root_ca(common_name: &str) -> Result<(rcgen::Certificate, KeyPair)> {
    let key = generate_key()?;
    let mut params = CertificateParams::new(vec![common_name.to_string()])
        .map_err(|error| ChaosError::InjectionFailed(error.to_string()))?;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let cert = params
        .self_signed(&key)
        .map_err(|error| ChaosError::InjectionFailed(error.to_string()))?;
    Ok((cert, key))
}

fn generate_key() -> Result<KeyPair> {
    KeyPair::generate().map_err(|error| ChaosError::InjectionFailed(error.to_string()))
}

fn server_profile(
    certificates: Vec<CertificateDer<'static>>,
    key: KeyPair,
    trust_anchor: Option<Vec<u8>>,
) -> Result<CertificateProfile> {
    let private_key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der()));
    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certificates, private_key)
        .map_err(|error| ChaosError::InjectionFailed(error.to_string()))?;
    Ok(CertificateProfile {
        server_config: Arc::new(server_config),
        trust_anchor,
    })
}

pub struct CryptoFaultInjector {
    config: CryptoFaultConfig,
    active: Arc<Mutex<HashMap<String, CryptoFaultServer>>>,
}

impl Default for CryptoFaultInjector {
    fn default() -> Self {
        Self::new(CryptoFaultConfig::default())
    }
}

impl CryptoFaultInjector {
    pub fn new(config: CryptoFaultConfig) -> Self {
        Self {
            config,
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn builder() -> CryptoFaultBuilder {
        CryptoFaultBuilder::default()
    }

    pub async fn metrics(&self, handle_id: &str) -> Option<CryptoFaultMetrics> {
        self.active
            .lock()
            .await
            .get(handle_id)
            .map(|server| server.metrics.snapshot())
    }

    pub async fn trust_anchor(&self, handle_id: &str) -> Option<Vec<u8>> {
        self.active
            .lock()
            .await
            .get(handle_id)
            .and_then(|server| server.trust_anchor.clone())
    }
}

#[async_trait]
impl Injector for CryptoFaultInjector {
    async fn inject(&self, target: &Target) -> Result<InjectionHandle> {
        if !matches!(target, Target::System) {
            return Err(ChaosError::InvalidConfig(
                "crypto_fault uses a rootless local endpoint and requires a system target"
                    .to_string(),
            ));
        }
        let server = CryptoFaultServer::start(self.config.clone()).await?;
        let handle = InjectionHandle::new(
            self.name(),
            target.clone(),
            serde_json::json!({
                "listen": server.listen,
                "domain": self.config.target_cert_domain,
                "fault_type": self.config.fault_type,
                "rootless": true,
            }),
        );
        self.active.lock().await.insert(handle.id.clone(), server);
        Ok(handle)
    }

    async fn remove(&self, handle: InjectionHandle) -> Result<()> {
        if let Some(server) = self.active.lock().await.remove(&handle.id) {
            let metrics = server.shutdown().await?;
            info!("TLS fault endpoint {} stopped: {:?}", handle.id, metrics);
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "crypto_fault"
    }

    fn status(&self) -> InjectorStatus {
        if self.config.fault_type.supported() {
            InjectorStatus::Stable
        } else {
            InjectorStatus::Planned
        }
    }

    async fn validate(&self) -> Result<()> {
        self.config.validate()
    }
}

#[derive(Default)]
pub struct CryptoFaultBuilder {
    listen: Option<SocketAddr>,
    fault_type: Option<CryptoFaultType>,
    target_cert_domain: Option<String>,
}

impl CryptoFaultBuilder {
    pub fn listen(mut self, listen: SocketAddr) -> Self {
        self.listen = Some(listen);
        self
    }

    pub fn fault_type(mut self, fault: CryptoFaultType) -> Self {
        self.fault_type = Some(fault);
        self
    }

    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.target_cert_domain = Some(domain.into());
        self
    }

    pub fn build(self) -> CryptoFaultInjector {
        let defaults = CryptoFaultConfig::default();
        CryptoFaultInjector::new(CryptoFaultConfig {
            listen: self.listen.unwrap_or(defaults.listen),
            fault_type: self.fault_type.unwrap_or(defaults.fault_type),
            target_cert_domain: self
                .target_cert_domain
                .unwrap_or(defaults.target_cert_domain),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_rustls::{
        rustls::{pki_types::ServerName, ClientConfig, RootCertStore},
        TlsConnector,
    };

    async fn handshake(
        listen: SocketAddr,
        domain: &str,
        trust_anchor: Option<Vec<u8>>,
    ) -> std::io::Result<()> {
        let mut roots = RootCertStore::empty();
        if let Some(anchor) = trust_anchor {
            roots.add(CertificateDer::from(anchor)).unwrap();
        }
        let client = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(client));
        let stream = TcpStream::connect(listen).await.unwrap();
        let name = ServerName::try_from(domain.to_string()).unwrap();
        connector.connect(name, stream).await.map(|_| ())
    }

    #[tokio::test]
    async fn expired_certificate_fails_even_when_issuer_is_trusted() {
        let injector = CryptoFaultInjector::builder()
            .listen("127.0.0.1:0".parse().unwrap())
            .fault_type(CryptoFaultType::CertExpired)
            .domain("localhost")
            .build();
        let handle = injector.inject(&Target::System).await.unwrap();
        let listen = handle.metadata["listen"].as_str().unwrap().parse().unwrap();
        let anchor = injector.trust_anchor(&handle.id).await;

        assert!(handshake(listen, "localhost", anchor).await.is_err());
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            injector
                .metrics(&handle.id)
                .await
                .unwrap()
                .handshake_failures
                > 0
        );
        injector.remove(handle).await.unwrap();
        assert!(TcpStream::connect(listen).await.is_err());
    }

    #[tokio::test]
    async fn incomplete_chain_fails_with_trusted_root() {
        let injector = CryptoFaultInjector::builder()
            .listen("127.0.0.1:0".parse().unwrap())
            .fault_type(CryptoFaultType::IncompleteChain)
            .domain("localhost")
            .build();
        let handle = injector.inject(&Target::System).await.unwrap();
        let listen = handle.metadata["listen"].as_str().unwrap().parse().unwrap();

        assert!(
            handshake(listen, "localhost", injector.trust_anchor(&handle.id).await)
                .await
                .is_err()
        );
        injector.remove(handle).await.unwrap();
    }

    #[tokio::test]
    async fn handshake_delay_is_measurable() {
        let delay = Duration::from_millis(120);
        let injector = CryptoFaultInjector::builder()
            .listen("127.0.0.1:0".parse().unwrap())
            .fault_type(CryptoFaultType::HandshakeDelay { delay })
            .build();
        let handle = injector.inject(&Target::System).await.unwrap();
        let listen = handle.metadata["listen"].as_str().unwrap().parse().unwrap();
        let started = tokio::time::Instant::now();
        let _ = handshake(listen, "localhost", None).await;
        assert!(started.elapsed() >= delay);
        injector.remove(handle).await.unwrap();
    }
}
