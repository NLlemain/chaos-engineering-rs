use anyhow::{bail, Context, Result};
use chaos_core::{
    DependencyProxyConfig, DependencyProxyInjector, DirectedToxic, Executor, ProxyDirection,
    ProxyToxic, RecoveryJournal, Target,
};
use clap::{Args, ValueEnum};
use colored::Colorize;
use std::{net::SocketAddr, sync::Arc};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DirectionArg {
    Upstream,
    Downstream,
    Both,
}

impl From<DirectionArg> for ProxyDirection {
    fn from(value: DirectionArg) -> Self {
        match value {
            DirectionArg::Upstream => Self::Upstream,
            DirectionArg::Downstream => Self::Downstream,
            DirectionArg::Both => Self::Both,
        }
    }
}

#[derive(Debug, Args)]
pub struct ProxyArgs {
    /// Local address clients connect to
    #[arg(long, default_value = "127.0.0.1:0")]
    listen: SocketAddr,

    /// Real dependency address behind the proxy
    #[arg(long)]
    upstream: SocketAddr,

    /// Traffic direction affected by configured faults
    #[arg(long, value_enum, default_value = "both")]
    direction: DirectionArg,

    /// Probability that each configured toxic applies to a connection
    #[arg(long, default_value = "1.0")]
    toxicity: f64,

    /// Added latency, such as 250ms
    #[arg(long)]
    latency: Option<String>,

    /// Random latency variance, such as 40ms
    #[arg(long)]
    jitter: Option<String>,

    /// Maximum bytes per second
    #[arg(long)]
    bandwidth: Option<u64>,

    /// Close a connection after this duration
    #[arg(long)]
    timeout: Option<String>,

    /// Delay socket closure by this duration
    #[arg(long)]
    slow_close: Option<String>,

    /// Close after forwarding this many bytes
    #[arg(long)]
    limit_data: Option<u64>,

    /// Reject connections beyond this concurrent count
    #[arg(long)]
    max_connections: Option<u64>,

    /// Hold all matching traffic until the injection is removed
    #[arg(long)]
    partition: bool,

    /// Probability of flipping a bit in each byte
    #[arg(long)]
    corrupt: Option<f64>,

    /// Probability of duplicating each chunk
    #[arg(long)]
    duplicate: Option<f64>,

    /// Probability of swapping adjacent chunks
    #[arg(long)]
    reorder: Option<f64>,

    /// Delay before releasing a reordered chunk
    #[arg(long, default_value = "10ms")]
    reorder_delay: String,

    /// Automatically stop the proxy after this duration
    #[arg(long)]
    duration: Option<String>,
}

pub async fn execute(args: ProxyArgs) -> Result<()> {
    let direction = args.direction.into();
    let mut config = DependencyProxyConfig::new(args.listen, args.upstream);
    let mut push = |toxic| {
        config
            .toxics
            .push(DirectedToxic::new(direction, toxic).with_toxicity(args.toxicity));
    };

    if let Some(latency) = &args.latency {
        push(ProxyToxic::Latency {
            delay_ms: duration_ms(latency, "latency")?,
            jitter_ms: args
                .jitter
                .as_deref()
                .map(|value| duration_ms(value, "jitter"))
                .transpose()?
                .unwrap_or(0),
        });
    } else if args.jitter.is_some() {
        bail!("--jitter requires --latency");
    }
    if let Some(bytes_per_second) = args.bandwidth {
        push(ProxyToxic::Bandwidth { bytes_per_second });
    }
    if let Some(timeout) = &args.timeout {
        push(ProxyToxic::Timeout {
            timeout_ms: duration_ms(timeout, "timeout")?,
        });
    }
    if let Some(delay) = &args.slow_close {
        push(ProxyToxic::SlowClose {
            delay_ms: duration_ms(delay, "slow-close")?,
        });
    }
    if let Some(bytes) = args.limit_data {
        push(ProxyToxic::LimitData { bytes });
    }
    if let Some(connections) = args.max_connections {
        push(ProxyToxic::ConnectionLimit { connections });
    }
    if args.partition {
        push(ProxyToxic::Partition);
    }
    if let Some(probability) = args.corrupt {
        push(ProxyToxic::Corrupt { probability });
    }
    if let Some(probability) = args.duplicate {
        push(ProxyToxic::Duplicate { probability });
    }
    if let Some(probability) = args.reorder {
        push(ProxyToxic::Reorder {
            probability,
            delay_ms: duration_ms(&args.reorder_delay, "reorder-delay")?,
        });
    }
    config.validate()?;

    let injector = Arc::new(DependencyProxyInjector::new(config));
    let journal = Arc::new(RecoveryJournal::new(RecoveryJournal::default_path()));
    let executor = Executor::with_defaults_and_journal(journal);
    let handle = executor
        .inject_with(injector.clone(), &Target::network(args.upstream))
        .await?;
    let listen = handle.metadata["listen"]
        .as_str()
        .context("Proxy did not report its listen address")?;

    println!("{}", "=== Rootless Dependency Proxy ===".bold().cyan());
    println!("Listen:   {}", listen.green());
    println!("Upstream: {}", args.upstream);
    println!("ID:       {}", handle.id);

    if let Some(value) = args.duration {
        tokio::time::sleep(humantime::parse_duration(&value)?).await;
    } else {
        println!("Press Ctrl+C to stop and clean up.");
        tokio::signal::ctrl_c().await?;
    }

    let metrics = injector.metrics(&handle.id).await.unwrap_or_default();
    executor.remove(handle).await?;
    println!(
        "Stopped: {} accepted, {} rejected, {} upstream bytes, {} downstream bytes, {} disruptions",
        metrics.accepted_connections,
        metrics.rejected_connections,
        metrics.upstream_bytes,
        metrics.downstream_bytes,
        metrics.disruptions
    );
    Ok(())
}

fn duration_ms(value: &str, name: &str) -> Result<u64> {
    let duration = humantime::parse_duration(value)
        .with_context(|| format!("Invalid {} duration '{}'", name, value))?;
    u64::try_from(duration.as_millis()).context("Duration is too large")
}
