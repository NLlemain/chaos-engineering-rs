use anyhow::Result;
use chaos_core::{Executor, Target};
use colored::Colorize;
use std::path::PathBuf;

pub async fn execute(
    pid: Option<u32>,
    address: Option<String>,
    injection: String,
    duration: Option<String>,
    _config: Option<PathBuf>,
) -> Result<()> {
    println!("{}", "=== Attach Mode ===".bold().cyan());

    // Determine target
    let target = if let Some(pid) = pid {
        println!("Target: Process PID {}", pid);
        Target::process(pid)
    } else if let Some(addr) = address {
        println!("Target: Network {}", addr);
        let socket_addr = addr.parse()?;
        Target::network(socket_addr)
    } else {
        anyhow::bail!("Must specify either --pid or --address");
    };

    // Check if target exists
    if !target.exists().await {
        println!("{}", "✗ Target not found!".red().bold());
        anyhow::bail!("Target does not exist");
    }

    println!("Injection: {}", injection.green());

    if let Some(dur) = &duration {
        println!("Duration: {}", dur);
    }

    // Create executor
    let executor = Executor::with_defaults();

    println!("\n{}", "Applying injection...".yellow());

    // Apply injection
    let handle = executor.inject(&injection, &target).await?;

    println!("{}", "✓ Injection applied successfully!".green().bold());
    println!("Injection ID: {}", handle.id);

    // Wait for duration if specified
    if let Some(dur_str) = duration {
        let duration = humantime::parse_duration(&dur_str)?;
        println!("\nWaiting {:?}...", duration);
        tokio::time::sleep(duration).await;

        println!("\n{}", "Removing injection...".yellow());
        executor.remove(handle).await?;
        println!("{}", "✓ Injection removed".green());
    } else {
        println!(
            "\n{}",
            "Injection will remain active (no duration specified)".yellow()
        );
        println!("Injection ID: {} (save this to remove later)", handle.id);
    }

    Ok(())
}
