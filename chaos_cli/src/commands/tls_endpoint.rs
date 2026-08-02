use anyhow::{Context, Result};
use chaos_core::{
    CryptoFaultConfig, CryptoFaultInjector, CryptoFaultType, Executor, RecoveryJournal, Target,
};
use clap::{Args, ValueEnum};
use colored::Colorize;
use std::{net::SocketAddr, sync::Arc};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TlsModeArg {
    Expired,
    Untrusted,
    IncompleteChain,
    Abort,
    Delay,
}

#[derive(Debug, Args)]
pub struct TlsEndpointArgs {
    /// Local TCP address for the failing TLS endpoint
    #[arg(long, default_value = "127.0.0.1:0")]
    listen: SocketAddr,

    /// Certificate hostname presented to clients
    #[arg(long, default_value = "localhost")]
    domain: String,

    /// TLS failure profile
    #[arg(long, value_enum)]
    mode: TlsModeArg,

    /// Delay before closing a delayed handshake
    #[arg(long, default_value = "2s")]
    delay: String,

    /// Stop and clean up after this duration
    #[arg(long)]
    duration: Option<String>,
}

pub async fn execute(args: TlsEndpointArgs) -> Result<()> {
    let fault_type = match args.mode {
        TlsModeArg::Expired => CryptoFaultType::CertExpired,
        TlsModeArg::Untrusted => CryptoFaultType::UntrustedCa,
        TlsModeArg::IncompleteChain => CryptoFaultType::IncompleteChain,
        TlsModeArg::Abort => CryptoFaultType::HandshakeAbort,
        TlsModeArg::Delay => CryptoFaultType::HandshakeDelay {
            delay: humantime::parse_duration(&args.delay).context("Invalid --delay")?,
        },
    };
    let config = CryptoFaultConfig {
        listen: args.listen,
        fault_type,
        target_cert_domain: args.domain,
    };
    config.validate()?;

    let injector = Arc::new(CryptoFaultInjector::new(config));
    let journal = Arc::new(RecoveryJournal::new(RecoveryJournal::default_path()));
    let executor = Executor::with_defaults_and_journal(journal);
    let handle = executor
        .inject_with(injector.clone(), &Target::System)
        .await?;
    let listen = handle.metadata["listen"]
        .as_str()
        .context("TLS endpoint did not report its listen address")?;

    println!("{}", "=== TLS Failure Endpoint ===".bold().cyan());
    println!("Endpoint: {}", format!("https://{}", listen).green());
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
        "Stopped: {} connections, {} aborted, {} failed handshakes",
        metrics.accepted_connections, metrics.aborted_connections, metrics.handshake_failures
    );
    Ok(())
}
