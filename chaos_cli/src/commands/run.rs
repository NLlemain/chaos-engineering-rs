use anyhow::{bail, Result};
use chaos_scenarios::{parse_scenario_from_file, ScenarioRunner};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use tracing::info;

pub async fn execute(
    scenario_file: PathBuf,
    output_json: Option<PathBuf>,
    output_html: Option<PathBuf>,
    output_markdown: Option<PathBuf>,
    prometheus_port: Option<u16>,
    seed: Option<u64>,
) -> Result<()> {
    if let Some(port) = prometheus_port {
        bail!("Prometheus export on port {} is not implemented yet; use JSON, Markdown, or HTML output", port);
    }

    println!("{}", "=== Chaos Framework ===".bold().cyan());
    println!("Loading scenario: {}", scenario_file.display());

    // Parse scenario
    let mut scenario = parse_scenario_from_file(&scenario_file).await?;

    // Override seed if provided
    if let Some(seed) = seed {
        scenario.seed = Some(seed);
        info!("Overriding scenario seed: {}", seed);
    }

    println!("\n{}", "Scenario Details:".bold());
    println!("  Name: {}", scenario.name.green());
    if let Some(desc) = &scenario.description {
        println!("  Description: {}", desc);
    }
    let duration = scenario.total_duration() + scenario.ramp_up.unwrap_or_default();
    println!("  Duration: {:?}", duration);
    println!("  Phases: {}", scenario.phases.len());
    if let Some(seed) = scenario.seed {
        println!("  Seed: {} (reproducible)", seed);
    }

    // Create progress bar
    let pb = ProgressBar::new(duration.as_secs());
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}s ({msg})",
            )?
            .progress_chars("=>-"),
    );

    println!("\n{}", "Starting chaos test...".bold().yellow());

    // Run scenario
    let runner = ScenarioRunner::with_defaults();

    // Spawn progress updater
    let pb_clone = pb.clone();
    let progress_task = tokio::spawn(async move {
        let start = tokio::time::Instant::now();
        loop {
            let elapsed = start.elapsed();
            if elapsed >= duration {
                break;
            }
            pb_clone.set_position(elapsed.as_secs());
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
        pb_clone.finish_with_message("Complete");
    });

    let result = runner.run(&scenario).await;
    progress_task.abort();
    let result = result?;

    pb.finish_and_clear();

    // Display results
    println!("\n{}", "=== Test Results ===".bold().green());
    println!("Scenario: {}", result.scenario_name.cyan());
    println!("Total Duration: {:?}", result.total_duration);
    println!("Total Injections: {}", result.total_injections);
    println!("Success Rate: {:.2}%", result.success_rate() * 100.0);

    println!("\n{}", "Phase Results:".bold());
    for phase in &result.phase_results {
        println!(
            "  {} - Duration: {:?}, Injections: {}/{} ({} failed)",
            phase.name.yellow(),
            phase.duration,
            phase.injection_count,
            phase.attempted_injections,
            phase.injection_failures.len()
        );
    }

    // Save outputs
    if let Some(json_path) = output_json {
        println!("\nSaving JSON report to: {}", json_path.display());
        let json = serde_json::to_string_pretty(&result)?;
        tokio::fs::write(&json_path, json).await?;
    }

    if let Some(html_path) = output_html {
        println!("Generating HTML report to: {}", html_path.display());
        let html = super::report::generate_html_report(&result);
        tokio::fs::write(&html_path, html).await?;
    }

    if let Some(md_path) = output_markdown {
        println!("Generating Markdown report to: {}", md_path.display());
        let markdown = super::report::generate_markdown_report(&result);
        tokio::fs::write(&md_path, markdown).await?;
    }

    println!(
        "\n{}",
        "✓ Chaos test completed successfully!".bold().green()
    );

    Ok(())
}
