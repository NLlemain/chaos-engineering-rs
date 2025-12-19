use anyhow::Result;
use chaos_core::Executor;
use colored::Colorize;

pub async fn execute() -> Result<()> {
    println!("{}", "=== Available Injectors ===".bold().cyan());

    let executor = Executor::with_defaults();
    let injectors = executor.list_injectors();

    println!("\nTotal injectors: {}\n", injectors.len());

    for injector in injectors {
        println!("  {} {}", "•".green(), injector);
    }

    println!(
        "\n{}",
        "Use 'chaos attach' to apply an injector to a target".yellow()
    );

    Ok(())
}
