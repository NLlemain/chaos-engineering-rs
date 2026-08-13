use crate::protocol::MAX_CONTROL_FRAME_BYTES;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tokio_rustls::rustls::server::WebPkiClientVerifier;
use tokio_rustls::rustls::{ClientConfig, RootCertStore, ServerConfig};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerTlsConfig {
    pub ca_cert: PathBuf,
    pub cert: PathBuf,
    pub key: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientTlsConfig {
    pub ca_cert: PathBuf,
    pub cert: PathBuf,
    pub key: PathBuf,
}

impl ServerTlsConfig {
    pub(crate) fn rustls_config(&self) -> anyhow::Result<Arc<ServerConfig>> {
        let mut roots = RootCertStore::empty();
        for certificate in load_certificates(&self.ca_cert)? {
            roots.add(certificate)?;
        }
        let verifier = WebPkiClientVerifier::builder(Arc::new(roots)).build()?;
        let config = ServerConfig::builder()
            .with_client_cert_verifier(verifier)
            .with_single_cert(load_certificates(&self.cert)?, load_private_key(&self.key)?)?;
        Ok(Arc::new(config))
    }
}

impl ClientTlsConfig {
    pub(crate) fn rustls_config(&self) -> anyhow::Result<Arc<ClientConfig>> {
        let mut roots = RootCertStore::empty();
        for certificate in load_certificates(&self.ca_cert)? {
            roots.add(certificate)?;
        }
        let config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_client_auth_cert(load_certificates(&self.cert)?, load_private_key(&self.key)?)?;
        Ok(Arc::new(config))
    }

    pub(crate) fn server_name(value: &str) -> anyhow::Result<ServerName<'static>> {
        ServerName::try_from(value.to_string())
            .map_err(|_| anyhow::anyhow!("invalid TLS server name '{value}'"))
    }
}

pub(crate) async fn write_frame<W: AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    value: &T,
) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    anyhow::ensure!(
        bytes.len() <= MAX_CONTROL_FRAME_BYTES,
        "control message exceeds {} bytes",
        MAX_CONTROL_FRAME_BYTES
    );
    writer.write_u32(bytes.len() as u32).await?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}

pub(crate) async fn read_frame<R: AsyncRead + Unpin, T: for<'de> Deserialize<'de>>(
    reader: &mut R,
) -> anyhow::Result<T> {
    let length = reader.read_u32().await? as usize;
    anyhow::ensure!(
        length <= MAX_CONTROL_FRAME_BYTES,
        "control message exceeds {} bytes",
        MAX_CONTROL_FRAME_BYTES
    );
    let mut bytes = vec![0u8; length];
    reader.read_exact(&mut bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn load_certificates(path: &PathBuf) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    let mut reader = BufReader::new(File::open(path)?);
    let certificates = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
    anyhow::ensure!(
        !certificates.is_empty(),
        "no certificates found in '{}'",
        path.display()
    );
    Ok(certificates)
}

fn load_private_key(path: &PathBuf) -> anyhow::Result<PrivateKeyDer<'static>> {
    let mut reader = BufReader::new(File::open(path)?);
    rustls_pemfile::private_key(&mut reader)?
        .ok_or_else(|| anyhow::anyhow!("no private key found in '{}'", path.display()))
}
