use anyhow::Result;
use chaos_core::{Executor, InjectorStatus};
use colored::Colorize;

pub async fn execute(json: bool) -> Result<()> {
    let executor = Executor::with_defaults();
    let injectors = executor.list_injector_info();

    if json {
        println!("{}", serde_json::to_string_pretty(&injectors)?);
        return Ok(());
    }

    println!("{}", "=== Available Injectors ===".bold().cyan());

    println!("\nTotal injectors: {}\n", injectors.len());

    for injector in injectors {
        let status = match injector.status {
            InjectorStatus::Stable => injector.status.to_string().green(),
            InjectorStatus::Experimental => injector.status.to_string().yellow(),
            InjectorStatus::Planned => injector.status.to_string().dimmed(),
        };
        let capabilities = if injector.required_capabilities.is_empty() {
            String::new()
        } else {
            format!(" [{}]", injector.required_capabilities.join(", "))
        };
        println!("  {:<28} {:<12}{}", injector.name, status, capabilities);
    }

    println!(
        "\n{}",
        "Use 'chaos attach' to apply an injector to a target".yellow()
    );

    Ok(())
}
