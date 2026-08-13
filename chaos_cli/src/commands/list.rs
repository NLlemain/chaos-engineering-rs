use anyhow::Result;
use chaos_core::{Executor, InjectorPlatform, InjectorStatus};
use clap::ValueEnum;
use colored::Colorize;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PlatformArg {
    Linux,
    Windows,
    Macos,
}

impl From<PlatformArg> for InjectorPlatform {
    fn from(value: PlatformArg) -> Self {
        match value {
            PlatformArg::Linux => Self::Linux,
            PlatformArg::Windows => Self::Windows,
            PlatformArg::Macos => Self::Macos,
        }
    }
}

pub async fn execute(json: bool, platform: Option<PlatformArg>) -> Result<()> {
    let executor = Executor::with_defaults();
    let injectors = platform.map_or_else(
        || executor.list_injector_info(),
        |platform| executor.list_injector_info_for(platform.into()),
    );

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
