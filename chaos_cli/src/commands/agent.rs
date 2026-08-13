use anyhow::{Context, Result};
use chaos_control::{AgentServer, AgentServerConfig, ExperimentPolicy, ServerTlsConfig};
use clap::{Args, Subcommand};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct AgentArgs {
    #[command(subcommand)]
    command: AgentCommand,
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    /// Serve the mutually authenticated remote control protocol
    Serve {
        #[arg(long)]
        id: String,
        #[arg(long, default_value = "127.0.0.1:9443")]
        listen: SocketAddr,
        #[arg(long)]
        ca_cert: PathBuf,
        #[arg(long)]
        cert: PathBuf,
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        policy: Option<PathBuf>,
        #[arg(long)]
        journal_directory: Option<PathBuf>,
    },
}

pub async fn execute(args: AgentArgs) -> Result<()> {
    match args.command {
        AgentCommand::Serve {
            id,
            listen,
            ca_cert,
            cert,
            key,
            policy,
            journal_directory,
        } => {
            let policy = match policy {
                Some(path) => read_policy(&path).await?,
                None => ExperimentPolicy::default(),
            };
            let server = AgentServer::start(
                AgentServerConfig {
                    agent_id: id.clone(),
                    listen,
                    tls: ServerTlsConfig { ca_cert, cert, key },
                    journal_directory: journal_directory
                        .unwrap_or_else(|| default_control_directory().join("agents").join(&id)),
                },
                policy,
            )
            .await?;
            println!(
                "Agent '{}' listening on {} with mutual TLS",
                id,
                server.local_addr()
            );
            tokio::signal::ctrl_c().await?;
            server.shutdown().await
        }
    }
}

async fn read_policy(path: &Path) -> Result<ExperimentPolicy> {
    let contents = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("read policy '{}'", path.display()))?;
    match path.extension().and_then(|value| value.to_str()) {
        Some("yaml" | "yml") => serde_yaml::from_str(&contents).map_err(Into::into),
        Some("json") => serde_json::from_str(&contents).map_err(Into::into),
        _ => anyhow::bail!("policy must use a .yaml, .yml, or .json extension"),
    }
}

pub(crate) fn default_control_directory() -> PathBuf {
    chaos_core::RecoveryJournal::default_path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}
