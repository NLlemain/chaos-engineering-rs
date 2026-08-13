use crate::commands::agent::default_control_directory;
use anyhow::Result;
use chaos_control::{ExperimentHistory, RetentionPolicy};
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct HistoryArgs {
    #[arg(long, global = true)]
    database: Option<PathBuf>,
    #[command(subcommand)]
    command: HistoryCommand,
}

#[derive(Debug, Subcommand)]
enum HistoryCommand {
    /// List recent distributed experiments
    List {
        #[arg(long, default_value = "20")]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Show one experiment and its retained artifacts
    Show { id: String },
    /// Apply run, age, and artifact-byte retention limits
    Prune {
        #[arg(long, default_value = "500")]
        max_runs: usize,
        #[arg(long, default_value = "30")]
        max_age_days: u32,
        #[arg(long, default_value = "2147483648")]
        max_artifact_bytes: u64,
    },
}

pub fn execute(args: HistoryArgs) -> Result<()> {
    let history = ExperimentHistory::open(
        args.database
            .unwrap_or_else(|| default_control_directory().join("history.sqlite")),
    )?;
    match args.command {
        HistoryCommand::List { limit, json } => {
            let records = history.list(limit)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&records)?);
            } else {
                for record in records {
                    println!(
                        "{}  {:10}  seed={}  targets={}  {}",
                        record.id, record.status, record.seed, record.target_count, record.name
                    );
                }
            }
        }
        HistoryCommand::Show { id } => {
            let record = history
                .get(&id)?
                .ok_or_else(|| anyhow::anyhow!("unknown experiment '{id}'"))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "experiment": record,
                    "artifacts": history.artifacts(&id)?,
                }))?
            );
        }
        HistoryCommand::Prune {
            max_runs,
            max_age_days,
            max_artifact_bytes,
        } => {
            let removed = history.prune(RetentionPolicy {
                max_runs,
                max_age_days,
                max_artifact_bytes,
            })?;
            println!("Removed {} expired run or artifact record(s).", removed);
        }
    }
    Ok(())
}
