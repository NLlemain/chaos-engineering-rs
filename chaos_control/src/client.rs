use crate::manifest::RemoteAgent;
use crate::protocol::{AgentState, ControlCommand, ControlRequest, ControlResponse};
use crate::tls::{read_frame, write_frame, ClientTlsConfig};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

#[derive(Clone)]
pub struct AgentClient {
    tls: ClientTlsConfig,
}

impl AgentClient {
    pub fn new(tls: ClientTlsConfig) -> Self {
        Self { tls }
    }

    pub async fn request(
        &self,
        agent: &RemoteAgent,
        command: ControlCommand,
    ) -> anyhow::Result<ControlResponse> {
        let request = ControlRequest::new(command);
        let stream = TcpStream::connect(agent.address).await?;
        let connector = TlsConnector::from(self.tls.rustls_config()?);
        let mut stream = connector
            .connect(ClientTlsConfig::server_name(&agent.server_name)?, stream)
            .await?;
        write_frame(&mut stream, &request).await?;
        let response: ControlResponse = read_frame(&mut stream).await?;
        anyhow::ensure!(
            response.request_id == request.request_id,
            "agent '{}' returned a mismatched request ID",
            agent.id
        );
        anyhow::ensure!(
            response.agent_id == agent.id,
            "connected agent identified as '{}' instead of '{}'",
            response.agent_id,
            agent.id
        );
        anyhow::ensure!(
            response.accepted,
            "agent '{}': {}",
            agent.id,
            response.message
        );
        anyhow::ensure!(
            response.state != AgentState::Rejected,
            "agent '{}' rejected the request",
            agent.id
        );
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentServer, AgentServerConfig, ExperimentPolicy, ServerTlsConfig};
    use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
    use std::path::Path;
    use tokio_rustls::rustls::{ClientConfig, RootCertStore};

    #[tokio::test]
    async fn mutually_authenticated_agent_accepts_the_expected_client() {
        let directory = tempfile::tempdir().unwrap();
        let certificates = write_test_certificates(directory.path());
        let server = AgentServer::start(
            AgentServerConfig {
                agent_id: "agent-ams-1".into(),
                listen: "127.0.0.1:0".parse().unwrap(),
                tls: certificates.server.clone(),
                journal_directory: directory.path().join("journal"),
            },
            ExperimentPolicy::default(),
        )
        .await
        .unwrap();
        let agent = RemoteAgent {
            id: "agent-ams-1".into(),
            address: server.local_addr(),
            server_name: "agent-ams-1.chaos.test".into(),
            labels: Default::default(),
        };
        let response = AgentClient::new(certificates.client)
            .request(&agent, ControlCommand::Ping)
            .await
            .unwrap();
        assert_eq!(response.state, AgentState::Ready);
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn agent_rejects_clients_without_a_certificate() {
        let directory = tempfile::tempdir().unwrap();
        let certificates = write_test_certificates(directory.path());
        let server = AgentServer::start(
            AgentServerConfig {
                agent_id: "agent-ams-1".into(),
                listen: "127.0.0.1:0".parse().unwrap(),
                tls: certificates.server,
                journal_directory: directory.path().join("journal"),
            },
            ExperimentPolicy::default(),
        )
        .await
        .unwrap();

        let mut roots = RootCertStore::empty();
        let mut reader = std::io::BufReader::new(std::fs::File::open(certificates.ca).unwrap());
        for certificate in rustls_pemfile::certs(&mut reader) {
            roots.add(certificate.unwrap()).unwrap();
        }
        let config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let stream = TcpStream::connect(server.local_addr()).await.unwrap();
        let connector = TlsConnector::from(std::sync::Arc::new(config));
        let connected = connector
            .connect(
                ClientTlsConfig::server_name("agent-ams-1.chaos.test").unwrap(),
                stream,
            )
            .await;
        if let Ok(mut stream) = connected {
            let request = ControlRequest::new(ControlCommand::Ping);
            assert!(
                write_frame(&mut stream, &request).await.is_err()
                    || read_frame::<_, ControlResponse>(&mut stream).await.is_err()
            );
        }
        server.shutdown().await.unwrap();
    }

    struct TestCertificates {
        ca: std::path::PathBuf,
        server: ServerTlsConfig,
        client: ClientTlsConfig,
    }

    fn write_test_certificates(directory: &Path) -> TestCertificates {
        let ca_key = KeyPair::generate().unwrap();
        let mut ca_params = CertificateParams::new(vec!["chaos.test".into()]).unwrap();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let ca = ca_params.self_signed(&ca_key).unwrap();

        let server_key = KeyPair::generate().unwrap();
        let server = CertificateParams::new(vec!["agent-ams-1.chaos.test".into()])
            .unwrap()
            .signed_by(&server_key, &ca, &ca_key)
            .unwrap();
        let client_key = KeyPair::generate().unwrap();
        let client = CertificateParams::new(vec!["orchestrator.chaos.test".into()])
            .unwrap()
            .signed_by(&client_key, &ca, &ca_key)
            .unwrap();

        let ca_path = directory.join("ca.pem");
        let server_cert = directory.join("server.pem");
        let server_key_path = directory.join("server-key.pem");
        let client_cert = directory.join("client.pem");
        let client_key_path = directory.join("client-key.pem");
        std::fs::write(&ca_path, ca.pem()).unwrap();
        std::fs::write(&server_cert, server.pem()).unwrap();
        std::fs::write(&server_key_path, server_key.serialize_pem()).unwrap();
        std::fs::write(&client_cert, client.pem()).unwrap();
        std::fs::write(&client_key_path, client_key.serialize_pem()).unwrap();
        TestCertificates {
            ca: ca_path.clone(),
            server: ServerTlsConfig {
                ca_cert: ca_path.clone(),
                cert: server_cert,
                key: server_key_path,
            },
            client: ClientTlsConfig {
                ca_cert: ca_path,
                cert: client_cert,
                key: client_key_path,
            },
        }
    }
}
