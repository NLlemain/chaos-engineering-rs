use crate::commands::agent::default_control_directory;
use anyhow::{bail, Context, Result};
use chaos_control::{
    ClientTlsConfig, DistributedExperiment, ExperimentHistory, ExperimentPolicy, Orchestrator,
};
use clap::Args;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Args)]
pub struct DistributedArgs {
    /// Distributed experiment manifest in YAML or JSON
    pub manifest: PathBuf,
    #[arg(long)]
    pub ca_cert: PathBuf,
    #[arg(long)]
    pub cert: PathBuf,
    #[arg(long)]
    pub key: PathBuf,
    /// Policy enforced again by the orchestrator
    #[arg(long)]
    pub policy: Option<PathBuf>,
    /// Central SQLite experiment history
    #[arg(long)]
    pub history: Option<PathBuf>,
    /// Write the combined distributed result as JSON
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

pub async fn execute(args: DistributedArgs) -> Result<()> {
    let experiment: DistributedExperiment = read_struct(&args.manifest).await?;
    let policy = match args.policy {
        Some(path) => read_struct(&path).await?,
        None => ExperimentPolicy::default(),
    };
    let history_path = args
        .history
        .unwrap_or_else(|| default_control_directory().join("history.sqlite"));
    let history = Arc::new(ExperimentHistory::open(history_path)?);
    let orchestrator = Orchestrator::new(
        ClientTlsConfig {
            ca_cert: args.ca_cert,
            cert: args.cert,
            key: args.key,
        },
        policy,
        history,
    );
    let result = orchestrator.run(&experiment).await?;
    let json = serde_json::to_vec_pretty(&result)?;
    if let Some(path) = args.output {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, &json).await?;
    }
    println!("{}", String::from_utf8_lossy(&json));
    if !result.succeeded {
        bail!("distributed experiment failed");
    }
    Ok(())
}

async fn read_struct<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let contents = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("read '{}'", path.display()))?;
    match path.extension().and_then(|value| value.to_str()) {
        Some("yaml" | "yml") => serde_yaml::from_str(&contents).map_err(Into::into),
        Some("json") => serde_json::from_str(&contents).map_err(Into::into),
        _ => bail!(
            "'{}' must use a .yaml, .yml, or .json extension",
            path.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn checked_in_manifest_and_policy_are_valid() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let experiment: DistributedExperiment =
            read_struct(&root.join("examples/distributed-experiment.yaml"))
                .await
                .unwrap();
        let policy: ExperimentPolicy = read_struct(&root.join("examples/distributed-policy.yaml"))
            .await
            .unwrap();
        experiment.validate().unwrap();
        policy.validate().unwrap();
        assert_eq!(experiment.target_count(), 2);
        assert_eq!(
            policy
                .effective_parallel_limit(
                    experiment.max_parallel_targets,
                    experiment.target_count(),
                    experiment.max_blast_radius_percent,
                )
                .unwrap(),
            1
        );
    }
}
