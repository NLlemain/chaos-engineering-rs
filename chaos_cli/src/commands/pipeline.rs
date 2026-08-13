use anyhow::{bail, Context, Result};
use chaos_pipeline::{
    evidence, parse_json_lines, PipelineBudget, PipelineEvidence, PipelineFaultPlan,
};
use clap::{Args, Subcommand};
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct PipelineArgs {
    #[command(subcommand)]
    command: PipelineCommand,
}

#[derive(Debug, Subcommand)]
enum PipelineCommand {
    /// Rendezvous every JSONL record through a zero-capacity channel and inject faults
    Replay {
        /// JSON Lines records with sequence, partition, key, timestamp_ns, and data fields
        records: PathBuf,
        /// YAML or JSON pipeline fault plan
        #[arg(long)]
        fault_plan: PathBuf,
        /// Optional YAML or JSON integrity and producer-blocking budget
        #[arg(long)]
        budget: Option<PathBuf>,
        /// Write the complete baseline, chaos, and restored evidence as JSON
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Require the faulted pipeline to remain within the supplied budget
        #[arg(long)]
        assert_chaos_budget: bool,
    },
}

pub async fn execute(args: PipelineArgs) -> Result<()> {
    match args.command {
        PipelineCommand::Replay {
            records,
            fault_plan,
            budget,
            output,
            assert_chaos_budget,
        } => {
            replay_pipeline(
                &records,
                &fault_plan,
                budget.as_deref(),
                output.as_deref(),
                assert_chaos_budget,
            )
            .await
        }
    }
}

async fn replay_pipeline(
    records_path: &Path,
    fault_plan_path: &Path,
    budget_path: Option<&Path>,
    output_path: Option<&Path>,
    assert_chaos_budget: bool,
) -> Result<()> {
    let contents = tokio::fs::read_to_string(records_path)
        .await
        .with_context(|| format!("read pipeline records from '{}'", records_path.display()))?;
    let records = parse_json_lines(&contents)
        .with_context(|| format!("parse pipeline fixture '{}'", records_path.display()))?;
    let plan: PipelineFaultPlan = read_struct(fault_plan_path).await?;
    let budget: PipelineBudget = match budget_path {
        Some(path) => read_struct(path).await?,
        None => PipelineBudget::default(),
    };
    let report = evidence(&records, &plan, &budget)?;
    let json = serde_json::to_vec_pretty(&report)?;
    if let Some(path) = output_path {
        write_file(path, &json).await?;
    }
    println!("{}", String::from_utf8_lossy(&json));

    validate_evidence(&report, assert_chaos_budget)
}

fn validate_evidence(report: &PipelineEvidence, assert_chaos_budget: bool) -> Result<()> {
    if !report.baseline.passed {
        bail!("baseline pipeline violates its invariant or backpressure budget");
    }
    if !report.zero_buffer_verified {
        bail!("pipeline replay did not preserve zero-buffer rendezvous semantics");
    }
    if !report.disruption_observed {
        bail!("fault plan produced no measurable pipeline disruption");
    }
    if !report.restoration_verified {
        bail!("pipeline did not return to its exact baseline delivered state");
    }
    if assert_chaos_budget && !report.chaos.passed {
        bail!(
            "faulted pipeline exceeded its budget: {}",
            report.chaos.violations.join("; ")
        );
    }
    Ok(())
}

async fn read_struct<T: DeserializeOwned>(path: &Path) -> Result<T> {
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

async fn write_file(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, contents).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn checked_in_zero_buffer_fixture_proves_backpressure_and_recovery() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let output = std::env::temp_dir().join(format!(
            "chaos-pipeline-evidence-{}.json",
            std::process::id()
        ));
        replay_pipeline(
            &root.join("tests/pipeline-evidence/records.jsonl"),
            &root.join("tests/pipeline-evidence/zero-buffer-stall.yaml"),
            Some(&root.join("tests/pipeline-evidence/budget.yaml")),
            Some(&output),
            false,
        )
        .await
        .unwrap();

        let report: PipelineEvidence =
            serde_json::from_slice(&tokio::fs::read(&output).await.unwrap()).unwrap();
        assert!(report.zero_buffer_verified);
        assert!(report.backpressure_observed);
        assert!(report.disruption_observed);
        assert!(report.restoration_verified);
        tokio::fs::remove_file(output).await.unwrap();
    }
}
