use anyhow::{bail, Result};
use chaos_core::{Executor, RecoveryJournal};
use colored::Colorize;
use std::{path::PathBuf, sync::Arc};

pub async fn execute(journal_path: Option<PathBuf>, emergency: bool) -> Result<()> {
    let path = journal_path.unwrap_or_else(RecoveryJournal::default_path);
    let journal = Arc::new(RecoveryJournal::new(&path));
    let entries = journal.entries().await?;

    let heading = if emergency {
        "=== Emergency Stop All ==="
    } else {
        "=== Recover Interrupted Injections ==="
    };
    println!("{}", heading.bold().cyan());
    println!("Journal: {}", path.display());

    if entries.is_empty() {
        println!("{}", "No active injections recorded.".green());
        return Ok(());
    }

    let executor = Executor::with_defaults_and_journal(journal);
    let mut failures = Vec::new();
    for handle in entries {
        print!("Removing {} ({}) ... ", handle.injector_name, handle.id);
        match executor.remove(handle).await {
            Ok(()) => println!("{}", "done".green()),
            Err(error) => {
                println!("{}", "failed".red());
                failures.push(error.to_string());
            }
        }
    }

    if failures.is_empty() {
        println!("{}", "Recovery complete.".green().bold());
        Ok(())
    } else {
        bail!("Recovery failed:\n{}", failures.join("\n"))
    }
}
