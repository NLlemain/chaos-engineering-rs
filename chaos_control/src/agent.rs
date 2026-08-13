use crate::policy::ExperimentPolicy;
use crate::protocol::{
    AgentState, ControlCommand, ControlRequest, ControlResponse, PROTOCOL_VERSION,
};
use crate::tls::{read_frame, write_frame, ServerTlsConfig};
use chaos_core::{Executor, RecoveryJournal};
use chaos_scenarios::{Scenario, ScenarioRunner};
use chrono::Utc;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant};
use tokio_rustls::TlsAcceptor;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct AgentServerConfig {
    pub agent_id: String,
    pub listen: SocketAddr,
    pub tls: ServerTlsConfig,
    pub journal_directory: PathBuf,
}

#[derive(Clone)]
struct PreparedExecution {
    seed: u64,
    scenario: Scenario,
}

struct AgentRuntime {
    agent_id: String,
    policy: ExperimentPolicy,
    journal_directory: PathBuf,
    prepared: Mutex<HashMap<String, PreparedExecution>>,
    active: Mutex<HashMap<String, CancellationToken>>,
}

pub struct AgentServer {
    local_addr: SocketAddr,
    cancellation: CancellationToken,
    task: JoinHandle<()>,
}

impl AgentServer {
    pub async fn start(
        config: AgentServerConfig,
        policy: ExperimentPolicy,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !config.agent_id.trim().is_empty(),
            "agent ID cannot be empty"
        );
        policy.validate()?;
        tokio::fs::create_dir_all(&config.journal_directory).await?;
        let listener = TcpListener::bind(config.listen).await?;
        let local_addr = listener.local_addr()?;
        let acceptor = TlsAcceptor::from(config.tls.rustls_config()?);
        let cancellation = CancellationToken::new();
        let shutdown = cancellation.clone();
        let runtime = Arc::new(AgentRuntime {
            agent_id: config.agent_id,
            policy,
            journal_directory: config.journal_directory,
            prepared: Mutex::new(HashMap::new()),
            active: Mutex::new(HashMap::new()),
        });
        let task = tokio::spawn(async move {
            run_listener(listener, acceptor, runtime, shutdown).await;
        });
        Ok(Self {
            local_addr,
            cancellation,
            task,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.cancellation.cancel();
        self.task.await?;
        Ok(())
    }
}

async fn run_listener(
    listener: TcpListener,
    acceptor: TlsAcceptor,
    runtime: Arc<AgentRuntime>,
    cancellation: CancellationToken,
) {
    info!(agent = %runtime.agent_id, address = %listener.local_addr().unwrap_or_else(|_| "0.0.0.0:0".parse().expect("valid fallback")), "mTLS agent ready");
    loop {
        let accepted = tokio::select! {
            _ = cancellation.cancelled() => break,
            accepted = listener.accept() => accepted,
        };
        let (stream, peer) = match accepted {
            Ok(value) => value,
            Err(error) => {
                warn!(%error, "agent accept failed");
                continue;
            }
        };
        let connection_acceptor = acceptor.clone();
        let connection_runtime = runtime.clone();
        tokio::spawn(async move {
            if let Err(error) =
                handle_connection(stream, connection_acceptor, connection_runtime).await
            {
                warn!(%peer, %error, "agent control connection rejected");
            }
        });
    }
    let tokens: Vec<_> = runtime.active.lock().await.values().cloned().collect();
    for token in tokens {
        token.cancel();
    }
}

async fn handle_connection(
    stream: TcpStream,
    acceptor: TlsAcceptor,
    runtime: Arc<AgentRuntime>,
) -> anyhow::Result<()> {
    let mut stream = acceptor.accept(stream).await?;
    anyhow::ensure!(
        stream
            .get_ref()
            .1
            .peer_certificates()
            .is_some_and(|certificates| !certificates.is_empty()),
        "control connection has no authenticated client certificate"
    );
    let request: ControlRequest = read_frame(&mut stream).await?;
    let response = if request.protocol_version != PROTOCOL_VERSION {
        ControlResponse::rejected(
            &request,
            &runtime.agent_id,
            format!(
                "protocol version {} is unsupported; expected {}",
                request.protocol_version, PROTOCOL_VERSION
            ),
        )
    } else {
        match process_request(&runtime, &request).await {
            Ok(response) => response,
            Err(error) => ControlResponse::rejected(&request, &runtime.agent_id, error.to_string()),
        }
    };
    write_frame(&mut stream, &response).await?;
    Ok(())
}

async fn process_request(
    runtime: &Arc<AgentRuntime>,
    request: &ControlRequest,
) -> anyhow::Result<ControlResponse> {
    match &request.command {
        ControlCommand::Ping => Ok(ControlResponse::accepted(
            request,
            &runtime.agent_id,
            AgentState::Ready,
            "agent is ready",
            None,
        )),
        ControlCommand::Prepare {
            execution_id,
            seed,
            scenario,
        } => {
            validate_execution_id(execution_id)?;
            runtime.policy.validate_at(scenario, Utc::now())?;
            anyhow::ensure!(
                !runtime.active.lock().await.contains_key(execution_id),
                "execution '{execution_id}' is already active"
            );
            let mut scenario = scenario.clone();
            scenario.seed = Some(*seed);
            runtime.prepared.lock().await.insert(
                execution_id.clone(),
                PreparedExecution {
                    seed: *seed,
                    scenario,
                },
            );
            Ok(ControlResponse::accepted(
                request,
                &runtime.agent_id,
                AgentState::Prepared,
                format!("execution '{execution_id}' prepared with seed {seed}"),
                None,
            ))
        }
        ControlCommand::Execute { execution_id } => {
            let prepared = runtime
                .prepared
                .lock()
                .await
                .get(execution_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("execution '{execution_id}' was not prepared"))?;
            let cancellation = CancellationToken::new();
            {
                let mut active = runtime.active.lock().await;
                anyhow::ensure!(
                    active
                        .insert(execution_id.clone(), cancellation.clone())
                        .is_none(),
                    "execution '{execution_id}' is already running"
                );
            }
            let journal = Arc::new(RecoveryJournal::new(
                runtime
                    .journal_directory
                    .join(format!("{execution_id}.json")),
            ));
            let runner = ScenarioRunner::with_journal(journal);
            let result = runner
                .run_with_cancellation(&prepared.scenario, cancellation)
                .await;
            runtime.active.lock().await.remove(execution_id);
            runtime.prepared.lock().await.remove(execution_id);
            let result = result?;
            let state = if result.cancelled {
                AgentState::Recovered
            } else {
                AgentState::Completed
            };
            Ok(ControlResponse::accepted(
                request,
                &runtime.agent_id,
                state,
                format!(
                    "execution '{execution_id}' completed with seed {}",
                    prepared.seed
                ),
                Some(result),
            ))
        }
        ControlCommand::Recover { execution_id } => {
            validate_execution_id(execution_id)?;
            if let Some(cancellation) = runtime.active.lock().await.get(execution_id).cloned() {
                cancellation.cancel();
                wait_until_inactive(runtime, execution_id).await?;
            }
            recover_journal(
                runtime
                    .journal_directory
                    .join(format!("{execution_id}.json")),
            )
            .await?;
            runtime.prepared.lock().await.remove(execution_id);
            Ok(ControlResponse::accepted(
                request,
                &runtime.agent_id,
                AgentState::Recovered,
                format!("execution '{execution_id}' recovered"),
                None,
            ))
        }
        ControlCommand::StopAll => {
            let active: Vec<_> = runtime
                .active
                .lock()
                .await
                .iter()
                .map(|(id, token)| (id.clone(), token.clone()))
                .collect();
            for (_, token) in &active {
                token.cancel();
            }
            for (id, _) in &active {
                wait_until_inactive(runtime, id).await?;
                recover_journal(runtime.journal_directory.join(format!("{id}.json"))).await?;
            }
            runtime.prepared.lock().await.clear();
            Ok(ControlResponse::accepted(
                request,
                &runtime.agent_id,
                AgentState::Recovered,
                format!("stopped and recovered {} execution(s)", active.len()),
                None,
            ))
        }
    }
}

async fn wait_until_inactive(runtime: &AgentRuntime, execution_id: &str) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if !runtime.active.lock().await.contains_key(execution_id) {
            return Ok(());
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "execution '{execution_id}' did not stop within 30 seconds"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn recover_journal(path: PathBuf) -> anyhow::Result<()> {
    let journal = Arc::new(RecoveryJournal::new(path));
    let entries = journal.entries().await?;
    let executor = Executor::with_defaults_and_journal(journal);
    let mut failures = Vec::new();
    for handle in entries {
        if let Err(error) = executor.remove(handle).await {
            failures.push(error.to_string());
        }
    }
    anyhow::ensure!(
        failures.is_empty(),
        "recovery failed: {}",
        failures.join("; ")
    );
    Ok(())
}

fn validate_execution_id(value: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !value.is_empty()
            && value.len() <= 160
            && value.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            }),
        "execution ID contains unsafe characters"
    );
    Ok(())
}
