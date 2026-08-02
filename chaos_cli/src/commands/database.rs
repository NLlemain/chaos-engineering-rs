use anyhow::{Context, Result};
use chaos_core::{
    DatabaseFaultConfig, DatabaseFaultInjector, DatabaseFaultMode, Executor, LocalDatabaseEngine,
    RecoveryJournal, Target,
};
use clap::{Args, ValueEnum};
use colored::Colorize;
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DatabaseEngineArg {
    Duckdb,
    Sqlite,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DatabaseModeArg {
    Unavailable,
    ReadOnly,
    Lock,
    IoPressure,
    InodePressure,
}

#[derive(Debug, Args)]
pub struct DatabaseArgs {
    /// DuckDB or SQLite database file
    #[arg(long)]
    file: PathBuf,

    /// Local database engine
    #[arg(long, value_enum)]
    engine: DatabaseEngineArg,

    /// File-level failure to apply
    #[arg(long, value_enum)]
    mode: DatabaseModeArg,

    /// Bytes written and synced during each I/O pressure cycle
    #[arg(long, default_value = "1048576")]
    bytes_per_cycle: usize,

    /// Delay between I/O pressure cycles
    #[arg(long, default_value = "10ms")]
    cycle_delay: String,

    /// Files created for controlled inode pressure
    #[arg(long, default_value = "1000")]
    files: usize,

    /// Restore after this duration
    #[arg(long)]
    duration: Option<String>,
}

pub async fn execute(args: DatabaseArgs) -> Result<()> {
    let engine = match args.engine {
        DatabaseEngineArg::Duckdb => LocalDatabaseEngine::DuckDb,
        DatabaseEngineArg::Sqlite => LocalDatabaseEngine::Sqlite,
    };
    let mode = match args.mode {
        DatabaseModeArg::Unavailable => DatabaseFaultMode::Unavailable,
        DatabaseModeArg::ReadOnly => DatabaseFaultMode::ReadOnly,
        DatabaseModeArg::Lock => DatabaseFaultMode::Lock,
        DatabaseModeArg::IoPressure => DatabaseFaultMode::IoPressure {
            bytes_per_cycle: args.bytes_per_cycle,
            cycle_delay: humantime::parse_duration(&args.cycle_delay)
                .context("Invalid --cycle-delay")?,
        },
        DatabaseModeArg::InodePressure => DatabaseFaultMode::InodePressure { files: args.files },
    };
    let injector = Arc::new(DatabaseFaultInjector::new(DatabaseFaultConfig {
        engine,
        mode: mode.clone(),
    }));
    let journal = Arc::new(RecoveryJournal::new(RecoveryJournal::default_path()));
    let executor = Executor::with_defaults_and_journal(journal);
    let target = Target::file(&args.file);
    let handle = executor.inject_with(injector.clone(), &target).await?;

    println!("{}", "=== Local Database Fault ===".bold().cyan());
    println!("File:   {}", args.file.display());
    println!("Engine: {:?}", engine);
    println!("Mode:   {:?}", mode);
    println!("ID:     {}", handle.id);
    if let Some(value) = args.duration {
        tokio::time::sleep(humantime::parse_duration(&value)?).await;
    } else {
        println!("Press Ctrl+C to restore the database.");
        tokio::signal::ctrl_c().await?;
    }

    let pressure_bytes = injector.pressure_bytes(&handle.id).await;
    executor.remove(handle).await?;
    if let Some(bytes) = pressure_bytes {
        println!("Pressure bytes written: {}", bytes);
    }
    println!("{}", "Database restored.".green());
    Ok(())
}
