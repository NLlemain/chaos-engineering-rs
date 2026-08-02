use anyhow::{bail, Result};
use chaos_core::{InjectorRegistry, InjectorStatus, RecoveryJournal};
use colored::Colorize;

pub async fn execute() -> Result<()> {
    println!("{}", "=== Chaos Doctor ===".bold().cyan());
    let registry = InjectorRegistry::with_defaults();
    let mut failures = 0usize;

    for info in registry.list_info() {
        let injector = registry
            .get(&info.name)
            .expect("registry info should reference an injector");
        let result = if info.status == InjectorStatus::Planned {
            None
        } else {
            Some(injector.validate().await)
        };

        match result {
            None => println!("  {:<28} {}", info.name, "planned".dimmed()),
            Some(Ok(())) => println!("  {:<28} {}", info.name, "ready".green()),
            Some(Err(error)) => {
                println!("  {:<28} {} ({})", info.name, "blocked".red(), error);
                failures += 1;
            }
        }
    }

    let journal = RecoveryJournal::new(RecoveryJournal::default_path());
    let active = journal.entries().await?;
    println!("\nRecovery journal: {}", journal.path().display());
    println!("Recorded active injections: {}", active.len());

    if failures == 0 {
        println!("{}", "Doctor checks passed.".green().bold());
        Ok(())
    } else {
        bail!("{} operational injector(s) are blocked", failures)
    }
}
