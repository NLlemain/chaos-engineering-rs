use anyhow::{bail, Result};
use chaos_core::RecoveryJournal;
use chaos_metrics::exporters::otlp::OtlpHttpExporter;
use chaos_scenarios::{parse_scenario_from_file, runner::RunTelemetry, ScenarioRunner};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::{path::PathBuf, sync::Arc};
use tracing::info;

pub async fn execute(
    scenario_file: PathBuf,
    output_json: Option<PathBuf>,
    output_html: Option<PathBuf>,
    output_markdown: Option<PathBuf>,
    prometheus_port: Option<u16>,
    otlp_endpoint: Option<String>,
    seed: Option<u64>,
) -> Result<()> {
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
    let journal = Arc::new(RecoveryJournal::new(RecoveryJournal::default_path()));
    let telemetry = Arc::new(RunTelemetry::default());
    let runner = ScenarioRunner::with_journal_and_telemetry(journal, telemetry.clone());
    let prometheus = if let Some(port) = prometheus_port {
        let server = super::telemetry::PrometheusServer::start(port, telemetry).await?;
        println!("Prometheus: http://{}/metrics", server.address);
        Some(server)
    } else {
        None
    };

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
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            if let Some(server) = prometheus {
                server.shutdown().await?;
            }
            return Err(error);
        }
    };

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

    if !result.slo_results.is_empty() {
        println!("\n{}", "SLO Assertions:".bold());
        for slo in &result.slo_results {
            let status = if slo.passed {
                "PASS".green()
            } else {
                "FAIL".red()
            };
            println!(
                "  {} {} - {} probes, {:.2}% errors, p95 {:?}",
                status,
                slo.name,
                slo.total_requests,
                slo.error_rate * 100.0,
                slo.latency_p95
            );
        }
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

    if let Some(endpoint) = otlp_endpoint {
        OtlpHttpExporter::export(&endpoint, &result.scenario_name, &result.telemetry).await?;
        println!("OTLP metrics exported to {}", endpoint);
    }

    if let Some(server) = prometheus {
        server.shutdown().await?;
    }

    if !result.slos_passed() {
        bail!("One or more SLO assertions failed");
    }

    println!(
        "\n{}",
        "✓ Chaos test completed successfully!".bold().green()
    );

    Ok(())
}
