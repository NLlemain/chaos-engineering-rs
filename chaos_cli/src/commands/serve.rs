use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

pub async fn execute(
    port: u16,
    host: String,
    scenarios_dir: PathBuf,
    results_dir: PathBuf,
) -> Result<()> {
    println!("{}", "=== Chaos Dashboard ===".bold().cyan());
    println!();
    println!("Starting web dashboard...");
    println!();
    println!("  {} http://{}:{}", "URL:".bold(), host, port);
    println!("  {} {}", "Scenarios:".bold(), scenarios_dir.display());
    println!("  {} {}", "Results:".bold(), results_dir.display());
    println!();
    println!("{}", "Press Ctrl+C to stop the server".dimmed());
    println!();

    // Ensure directories exist
    if !scenarios_dir.exists() {
        println!(
            "{} Scenarios directory not found: {}",
            "Warning:".yellow(),
            scenarios_dir.display()
        );
    }

    if !results_dir.exists() {
        tokio::fs::create_dir_all(&results_dir).await?;
        println!(
            "{} Created results directory: {}",
            "Info:".blue(),
            results_dir.display()
        );
    }

    let config = chaos_web::WebConfig {
        port,
        host,
        scenarios_dir,
        results_dir,
    };

    chaos_web::serve(config).await?;

    Ok(())
}
