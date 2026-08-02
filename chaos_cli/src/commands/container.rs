use anyhow::{bail, Result};
use chaos_core::{
    ContainerFaultAction, ContainerFaultConfig, ContainerFaultInjector, Executor, RecoveryJournal,
    Target,
};
use clap::{Args, ValueEnum};
use colored::Colorize;
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ContainerActionArg {
    Pause,
    Stop,
    Kill,
    Restart,
}

impl From<ContainerActionArg> for ContainerFaultAction {
    fn from(value: ContainerActionArg) -> Self {
        match value {
            ContainerActionArg::Pause => Self::Pause,
            ContainerActionArg::Stop => Self::Stop,
            ContainerActionArg::Kill => Self::Kill,
            ContainerActionArg::Restart => Self::Restart,
        }
    }
}

#[derive(Debug, Args)]
pub struct ContainerArgs {
    /// Container ID or name
    #[arg(long, conflicts_with = "compose_service")]
    id: Option<String>,

    /// Docker Compose service name
    #[arg(long, conflicts_with = "id")]
    compose_service: Option<String>,

    /// Compose file containing the service
    #[arg(long, default_value = "compose.yaml", requires = "compose_service")]
    compose_file: PathBuf,

    /// Optional Compose project name
    #[arg(long, requires = "compose_service")]
    compose_project: Option<String>,

    /// Container disruption to apply
    #[arg(long, value_enum)]
    action: ContainerActionArg,

    /// Grace period for stop actions
    #[arg(long, default_value = "10")]
    stop_timeout_seconds: u64,

    /// Restore after this duration; restart actions finish immediately
    #[arg(long)]
    duration: Option<String>,
}

pub async fn execute(args: ContainerArgs) -> Result<()> {
    let target = match (args.id, args.compose_service) {
        (Some(id), None) => Target::container(id),
        (None, Some(service)) => {
            Target::compose_service(service, args.compose_file, args.compose_project)
        }
        _ => bail!("Specify either --id or --compose-service"),
    };
    let action: ContainerFaultAction = args.action.into();
    let injector = Arc::new(ContainerFaultInjector::new(ContainerFaultConfig {
        action,
        stop_timeout_seconds: args.stop_timeout_seconds,
    }));
    let journal = Arc::new(RecoveryJournal::new(RecoveryJournal::default_path()));
    let executor = Executor::with_defaults_and_journal(journal);
    let handle = executor.inject_with(injector, &target).await?;

    println!("{}", "=== Docker Container Fault ===".bold().cyan());
    println!("Target: {}", target.description());
    println!("Action: {}", action.to_string().yellow());
    println!("ID:     {}", handle.id);

    if action != ContainerFaultAction::Restart {
        if let Some(value) = args.duration {
            tokio::time::sleep(humantime::parse_duration(&value)?).await;
        } else {
            println!("Press Ctrl+C to restore the target.");
            tokio::signal::ctrl_c().await?;
        }
    }
    executor.remove(handle).await?;
    println!("{}", "Container target restored.".green());
    Ok(())
}
