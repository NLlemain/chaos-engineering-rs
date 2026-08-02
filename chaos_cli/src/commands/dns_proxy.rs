use anyhow::{Context, Result};
use chaos_core::{
    DnsFaultConfig, DnsFaultInjector, DnsFaultMode, Executor, RecoveryJournal, Target,
};
use clap::{Args, ValueEnum};
use colored::Colorize;
use std::{net::SocketAddr, sync::Arc, time::Duration};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DnsModeArg {
    Latency,
    Nxdomain,
    Spoof,
    Blackhole,
}

#[derive(Debug, Args)]
pub struct DnsProxyArgs {
    /// Local UDP address applications use as their resolver
    #[arg(long, default_value = "127.0.0.1:0")]
    listen: SocketAddr,

    /// Real DNS resolver behind the fault endpoint
    #[arg(long, default_value = "1.1.1.1:53")]
    upstream: SocketAddr,

    /// Domain or wildcard suffix to affect
    #[arg(long, default_value = "*")]
    domain: String,

    /// Fault applied to matching queries
    #[arg(long, value_enum)]
    mode: DnsModeArg,

    /// Delay for latency mode
    #[arg(long, default_value = "500ms")]
    delay: String,

    /// Address returned by spoof mode
    #[arg(long, default_value = "127.0.0.1")]
    fake_ip: String,

    /// Probability of faulting each matching query
    #[arg(long, default_value = "1.0")]
    rate: f64,

    /// Stop and clean up after this duration
    #[arg(long)]
    duration: Option<String>,
}

pub async fn execute(args: DnsProxyArgs) -> Result<()> {
    let mode = match args.mode {
        DnsModeArg::Latency => DnsFaultMode::Latency {
            delay: humantime::parse_duration(&args.delay).context("Invalid --delay")?,
        },
        DnsModeArg::Nxdomain => DnsFaultMode::NxDomain,
        DnsModeArg::Spoof => DnsFaultMode::Spoof {
            fake_ip: args.fake_ip,
        },
        DnsModeArg::Blackhole => DnsFaultMode::Blackhole,
    };
    let config = DnsFaultConfig {
        listen: args.listen,
        upstream: args.upstream,
        domain_pattern: args.domain,
        fault_mode: mode,
        failure_rate: args.rate,
        upstream_timeout: Duration::from_secs(2),
    };
    config.validate()?;

    let injector = Arc::new(DnsFaultInjector::new(config));
    let journal = Arc::new(RecoveryJournal::new(RecoveryJournal::default_path()));
    let executor = Executor::with_defaults_and_journal(journal);
    let handle = executor
        .inject_with(injector.clone(), &Target::network(args.upstream))
        .await?;
    let listen = handle.metadata["listen"]
        .as_str()
        .context("DNS proxy did not report its listen address")?;

    println!("{}", "=== Rootless DNS Fault Proxy ===".bold().cyan());
    println!("Resolver: {}", listen.green());
    println!("Upstream: {}", args.upstream);
    println!("ID:       {}", handle.id);
    wait(args.duration.as_deref()).await?;

    let metrics = injector.metrics(&handle.id).await.unwrap_or_default();
    executor.remove(handle).await?;
    println!(
        "Stopped: {} queries, {} injected, {} forwarded, {} dropped",
        metrics.queries,
        metrics.injected_queries,
        metrics.forwarded_queries,
        metrics.dropped_queries
    );
    Ok(())
}

async fn wait(duration: Option<&str>) -> Result<()> {
    if let Some(value) = duration {
        tokio::time::sleep(humantime::parse_duration(value)?).await;
    } else {
        println!("Press Ctrl+C to stop and clean up.");
        tokio::signal::ctrl_c().await?;
    }
    Ok(())
}
