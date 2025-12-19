use anyhow::Result;
use chaos_scenarios::parse_scenario_from_file;
use colored::Colorize;
use std::path::PathBuf;

pub async fn execute(scenario_file: PathBuf) -> Result<()> {
    println!("{}", "=== Validating Scenario ===".bold().cyan());
    println!("File: {}", scenario_file.display());

    match parse_scenario_from_file(&scenario_file).await {
        Ok(scenario) => {
            println!("\n{}", "✓ Scenario is valid!".green().bold());
            println!("\nScenario Details:");
            println!("  Name: {}", scenario.name);
            println!("  Phases: {}", scenario.phases.len());
            println!("  Total Duration: {:?}", scenario.duration);

            if scenario.phases.is_empty() {
                println!("\n{}", "⚠ Warning: Scenario has no phases".yellow());
            }

            for (i, phase) in scenario.phases.iter().enumerate() {
                println!("\n  Phase {}: {}", i + 1, phase.name);
                println!("    Duration: {:?}", phase.duration);
                println!("    Injections: {}", phase.injections.len());

                for (j, injection) in phase.injections.iter().enumerate() {
                    println!("      {}: {}", j + 1, injection.r#type);
                }
            }

            Ok(())
        }
        Err(e) => {
            println!("\n{}", "✗ Scenario is invalid!".red().bold());
            println!("\nError: {}", e);
            Err(e)
        }
    }
}
