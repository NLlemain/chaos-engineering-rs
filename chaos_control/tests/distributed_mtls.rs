use chaos_control::{
    AgentAssignment, AgentServer, AgentServerConfig, ClientTlsConfig, DistributedExperiment,
    DistributedPhase, ExperimentHistory, ExperimentPolicy, Orchestrator, RemoteAgent,
    ServerTlsConfig,
};
use chaos_scenarios::{config::Phase, Scenario};
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn coordinates_two_authenticated_agents_with_a_bounded_blast_radius() {
    let directory = tempfile::tempdir().unwrap();
    let certificates = TestCertificates::write(directory.path(), &["ams-1", "ams-2"]);
    let policy = ExperimentPolicy {
        max_targets: 2,
        max_parallel_targets: 2,
        max_blast_radius_percent: 50,
        ..ExperimentPolicy::default()
    };
    let first = start_agent(
        "ams-1",
        certificates.server("ams-1"),
        directory.path(),
        policy.clone(),
    )
    .await;
    let second = start_agent(
        "ams-2",
        certificates.server("ams-2"),
        directory.path(),
        policy.clone(),
    )
    .await;
    let experiment = DistributedExperiment {
        api_version: "chaos.engineering/v1".into(),
        name: "two-venue-market-data".into(),
        seed: Some(42),
        max_parallel_targets: 2,
        max_blast_radius_percent: 50,
        agents: vec![
            remote("ams-1", first.local_addr()),
            remote("ams-2", second.local_addr()),
        ],
        phases: vec![DistributedPhase {
            name: "venue-degradation".into(),
            assignments: vec![
                AgentAssignment {
                    target_id: "xnas-feed".into(),
                    agent_id: "ams-1".into(),
                    scenario: no_op_scenario("xnas-feed"),
                },
                AgentAssignment {
                    target_id: "xlon-feed".into(),
                    agent_id: "ams-2".into(),
                    scenario: no_op_scenario("xlon-feed"),
                },
            ],
        }],
    };
    let history =
        Arc::new(ExperimentHistory::open(directory.path().join("history.sqlite")).unwrap());
    let orchestrator = Orchestrator::new(certificates.client(), policy, history.clone());

    let result = orchestrator.run(&experiment).await.unwrap();

    assert!(result.succeeded);
    assert_eq!(result.seed, 42);
    assert_eq!(result.parallel_limit, 1);
    assert_eq!(result.phases[0].targets.len(), 2);
    assert_ne!(
        result.phases[0].targets[0].seed,
        result.phases[0].targets[1].seed
    );
    let record = history.get(&result.experiment_id).unwrap().unwrap();
    assert_eq!(record.status, "succeeded");
    assert_eq!(record.artifact_count, 3);

    first.shutdown().await.unwrap();
    second.shutdown().await.unwrap();
}

async fn start_agent(
    id: &str,
    tls: ServerTlsConfig,
    root: &Path,
    policy: ExperimentPolicy,
) -> AgentServer {
    AgentServer::start(
        AgentServerConfig {
            agent_id: id.into(),
            listen: "127.0.0.1:0".parse().unwrap(),
            tls,
            journal_directory: root.join("journals").join(id),
        },
        policy,
    )
    .await
    .unwrap()
}

fn remote(id: &str, address: std::net::SocketAddr) -> RemoteAgent {
    RemoteAgent {
        id: id.into(),
        address,
        server_name: format!("{id}.chaos.test"),
        labels: Default::default(),
    }
}

fn no_op_scenario(name: &str) -> Scenario {
    Scenario {
        name: name.into(),
        description: Some("Control-plane coordination fixture".into()),
        seed: None,
        duration: Duration::from_millis(5),
        ramp_up: None,
        phases: vec![Phase {
            name: "barrier".into(),
            duration: Duration::from_millis(5),
            injections: Vec::new(),
            parallel: false,
        }],
        labels: HashMap::new(),
        assertions: Vec::new(),
    }
}

struct TestCertificates {
    ca: PathBuf,
    client_cert: PathBuf,
    client_key: PathBuf,
    servers: HashMap<String, (PathBuf, PathBuf)>,
}

impl TestCertificates {
    fn write(directory: &Path, server_ids: &[&str]) -> Self {
        let ca_key = KeyPair::generate().unwrap();
        let mut ca_params = CertificateParams::new(vec!["chaos.test".into()]).unwrap();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let ca = ca_params.self_signed(&ca_key).unwrap();
        let ca_path = directory.join("ca.pem");
        std::fs::write(&ca_path, ca.pem()).unwrap();

        let client_key = KeyPair::generate().unwrap();
        let client = CertificateParams::new(vec!["orchestrator.chaos.test".into()])
            .unwrap()
            .signed_by(&client_key, &ca, &ca_key)
            .unwrap();
        let client_cert = directory.join("client.pem");
        let client_key_path = directory.join("client-key.pem");
        std::fs::write(&client_cert, client.pem()).unwrap();
        std::fs::write(&client_key_path, client_key.serialize_pem()).unwrap();

        let mut servers = HashMap::new();
        for id in server_ids {
            let key = KeyPair::generate().unwrap();
            let certificate = CertificateParams::new(vec![format!("{id}.chaos.test")])
                .unwrap()
                .signed_by(&key, &ca, &ca_key)
                .unwrap();
            let cert_path = directory.join(format!("{id}.pem"));
            let key_path = directory.join(format!("{id}-key.pem"));
            std::fs::write(&cert_path, certificate.pem()).unwrap();
            std::fs::write(&key_path, key.serialize_pem()).unwrap();
            servers.insert((*id).to_string(), (cert_path, key_path));
        }
        Self {
            ca: ca_path,
            client_cert,
            client_key: client_key_path,
            servers,
        }
    }

    fn server(&self, id: &str) -> ServerTlsConfig {
        let (cert, key) = self.servers.get(id).unwrap();
        ServerTlsConfig {
            ca_cert: self.ca.clone(),
            cert: cert.clone(),
            key: key.clone(),
        }
    }

    fn client(&self) -> ClientTlsConfig {
        ClientTlsConfig {
            ca_cert: self.ca.clone(),
            cert: self.client_cert.clone(),
            key: self.client_key.clone(),
        }
    }
}
