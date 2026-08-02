use anyhow::{bail, Context, Result};
use chaos_core::{InjectorRegistry, InjectorStatus};
use chaos_scenarios::{injector_factory::build_injector, parse_scenario_from_file};
use colored::Colorize;
use std::path::PathBuf;

pub async fn execute(scenario_file: PathBuf) -> Result<()> {
    println!("{}", "=== Scenario Dry Run ===".bold().cyan());
    println!("File: {}", scenario_file.display());

    let scenario = parse_scenario_from_file(&scenario_file).await?;
    let registry = InjectorRegistry::with_defaults();
    let mut failures = Vec::new();

    for phase in &scenario.phases {
        for injection in &phase.injections {
            let result = async {
                let target = injection.target.to_target().map_err(anyhow::Error::msg)?;
                let injector = match build_injector(injection)? {
                    Some(injector) => injector,
                    None => registry
                        .get(&injection.r#type)
                        .cloned()
                        .with_context(|| format!("Unknown injector '{}'", injection.r#type))?,
                };

                if injector.status() == InjectorStatus::Planned {
                    bail!("injector is planned but not implemented");
                }
                injector.validate().await?;
                if !target.exists().await {
                    bail!("target does not exist: {}", target.description());
                }
                Result::<()>::Ok(())
            }
            .await;

            match result {
                Ok(()) => println!(
                    "  {} {} / {}",
                    "ready".green(),
                    phase.name,
                    injection.r#type
                ),
                Err(error) => {
                    println!(
                        "  {} {} / {}: {}",
                        "blocked".red(),
                        phase.name,
                        injection.r#type,
                        error
                    );
                    failures.push(format!("{} / {}: {}", phase.name, injection.r#type, error));
                }
            }
        }
    }

    if failures.is_empty() {
        println!(
            "{}",
            "Dry run passed; no faults were applied.".green().bold()
        );
        Ok(())
    } else {
        bail!("Dry run found {} blocking issue(s)", failures.len())
    }
}
