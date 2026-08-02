mod commands;
mod ui;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::Level;

#[derive(Parser)]
#[command(name = "chaos")]
#[command(about = "Production-grade chaos testing framework for Rust async services", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Enable quiet mode (errors only)
    #[arg(short, long, global = true)]
    quiet: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a chaos scenario from file
    Run {
        /// Path to scenario file (YAML, TOML, or JSON)
        scenario_file: PathBuf,

        /// Output metrics to JSON file
        #[arg(short, long)]
        output_json: Option<PathBuf>,

        /// Generate HTML report
        #[arg(long)]
        output_html: Option<PathBuf>,

        /// Generate Markdown report
        #[arg(short = 'm', long)]
        output_markdown: Option<PathBuf>,

        /// Expose Prometheus metrics on port
        #[arg(short, long)]
        prometheus_port: Option<u16>,

        /// Override scenario seed
        #[arg(long)]
        seed: Option<u64>,
    },

    /// Start the web dashboard
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Directory containing scenario files
        #[arg(long, default_value = "scenarios")]
        scenarios_dir: PathBuf,

        /// Directory to store test results
        #[arg(long, default_value = "test_results")]
        results_dir: PathBuf,
    },

    /// Attach to a running process and inject chaos
    Attach {
        /// Process ID to attach to
        #[arg(short, long, group = "target")]
        pid: Option<u32>,

        /// Network address to target
        #[arg(short, long, group = "target")]
        address: Option<String>,

        /// Injection type
        #[arg(short, long)]
        injection: String,

        /// Duration of injection
        #[arg(short, long)]
        duration: Option<String>,

        /// Config file for injection parameters
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Generate report from metrics file
    Report {
        /// Path to metrics JSON file
        metrics_file: PathBuf,

        /// Output format (cli, json, markdown, html)
        #[arg(short, long, default_value = "cli")]
        format: String,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Compare with other runs
        #[arg(long)]
        compare: Vec<PathBuf>,
    },

    /// Validate a scenario file
    Validate {
        /// Path to scenario file
        scenario_file: PathBuf,
    },

    /// List available injectors
    List,

    /// Validate a scenario and environment without applying faults
    DryRun {
        /// Path to scenario file
        scenario_file: PathBuf,
    },

    /// Check injector dependencies and recovery state
    Doctor,

    /// Recover effects left by an interrupted experiment
    Recover {
        /// Override the recovery journal path
        #[arg(long)]
        journal: Option<PathBuf>,
    },

    /// Emergency cleanup of every journaled injection
    StopAll {
        /// Override the recovery journal path
        #[arg(long)]
        journal: Option<PathBuf>,
    },

    /// Start a rootless directional dependency fault proxy
    Proxy(commands::proxy::ProxyArgs),

    /// Start a provider-aware AI and HTTP fault proxy
    #[command(alias = "http-proxy")]
    AiProxy(commands::ai_proxy::AiProxyArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let log_level = if cli.verbose {
        Level::DEBUG
    } else if cli.quiet {
        Level::ERROR
    } else {
        Level::INFO
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    match cli.command {
        Commands::Run {
            scenario_file,
            output_json,
            output_html,
            output_markdown,
            prometheus_port,
            seed,
        } => {
            commands::run::execute(
                scenario_file,
                output_json,
                output_html,
                output_markdown,
                prometheus_port,
                seed,
            )
            .await?;
        }

        Commands::Serve {
            port,
            host,
            scenarios_dir,
            results_dir,
        } => {
            commands::serve::execute(port, host, scenarios_dir, results_dir).await?;
        }

        Commands::Attach {
            pid,
            address,
            injection,
            duration,
            config,
        } => {
            commands::attach::execute(pid, address, injection, duration, config).await?;
        }

        Commands::Report {
            metrics_file,
            format,
            output,
            compare,
        } => {
            commands::report::execute(metrics_file, format, output, compare).await?;
        }

        Commands::Validate { scenario_file } => {
            commands::validate::execute(scenario_file).await?;
        }

        Commands::List => {
            commands::list::execute().await?;
        }
        Commands::DryRun { scenario_file } => {
            commands::dry_run::execute(scenario_file).await?;
        }
        Commands::Doctor => {
            commands::doctor::execute().await?;
        }
        Commands::Recover { journal } => {
            commands::recover::execute(journal, false).await?;
        }
        Commands::StopAll { journal } => {
            commands::recover::execute(journal, true).await?;
        }
        Commands::Proxy(args) => {
            commands::proxy::execute(args).await?;
        }
        Commands::AiProxy(args) => {
            commands::ai_proxy::execute(args).await?;
        }
    }

    Ok(())
}
