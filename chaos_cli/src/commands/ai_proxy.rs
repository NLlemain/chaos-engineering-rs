use anyhow::{bail, Context, Result};
use chaos_core::{
    AiProvider, Executor, HttpFaultConfig, HttpFaultInjector, HttpFaultType, RecoveryJournal,
    Target,
};
use clap::{Args, ValueEnum};
use colored::Colorize;
use std::{net::SocketAddr, sync::Arc};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ProviderArg {
    Generic,
    OpenAi,
    AzureOpenAi,
    Anthropic,
    Gemini,
    OpenRouter,
    Ollama,
    Mistral,
    Groq,
    Cohere,
    Together,
    Vllm,
}

impl From<ProviderArg> for AiProvider {
    fn from(value: ProviderArg) -> Self {
        match value {
            ProviderArg::Generic => Self::Generic,
            ProviderArg::OpenAi => Self::OpenAi,
            ProviderArg::AzureOpenAi => Self::AzureOpenAi,
            ProviderArg::Anthropic => Self::Anthropic,
            ProviderArg::Gemini => Self::Gemini,
            ProviderArg::OpenRouter => Self::OpenRouter,
            ProviderArg::Ollama => Self::Ollama,
            ProviderArg::Mistral => Self::Mistral,
            ProviderArg::Groq => Self::Groq,
            ProviderArg::Cohere => Self::Cohere,
            ProviderArg::Together => Self::Together,
            ProviderArg::Vllm => Self::Vllm,
        }
    }
}

#[derive(Debug, Args)]
pub struct AiProxyArgs {
    /// Local HTTP address clients connect to
    #[arg(long, default_value = "127.0.0.1:0")]
    listen: SocketAddr,

    /// Provider base URL, including any required base path
    #[arg(long)]
    upstream: String,

    /// Provider wire format used for synthetic errors and tool calls
    #[arg(long, value_enum, default_value = "generic")]
    provider: ProviderArg,

    /// Only inject requests whose path matches this prefix glob
    #[arg(long, default_value = "/*")]
    path: String,

    /// Fraction of matching requests that receive faults
    #[arg(long, default_value = "1.0")]
    rate: f64,

    /// Replace matching responses with this HTTP status
    #[arg(long)]
    status: Option<u16>,

    /// Custom response body used with --status
    #[arg(long, requires = "status")]
    status_body: Option<String>,

    /// Delay the request before contacting the provider
    #[arg(long)]
    latency: Option<String>,

    /// Delay every SSE or NDJSON event
    #[arg(long)]
    stream_delay: Option<String>,

    /// Abort a stream after this many events
    #[arg(long)]
    stream_abort: Option<usize>,

    /// Emit provider-shaped invalid tool arguments
    #[arg(long)]
    malformed_tool_call: bool,

    /// Retain only this many newest context items
    #[arg(long)]
    context_keep: Option<usize>,

    /// Truncate response bodies to this many bytes
    #[arg(long)]
    truncate_body: Option<usize>,

    /// Replace the response with invalid JSON
    #[arg(long)]
    malformed_json: bool,

    /// Remove content type and add contradictory retry metadata
    #[arg(long)]
    malformed_headers: bool,

    /// Return a successful response with an empty body
    #[arg(long)]
    empty_response: bool,

    /// Remove a response header; may be supplied more than once
    #[arg(long)]
    strip_header: Vec<String>,

    /// Drip stream events with this delay
    #[arg(long)]
    slowloris: Option<String>,

    /// Automatically stop after this duration
    #[arg(long)]
    duration: Option<String>,
}

pub async fn execute(args: AiProxyArgs) -> Result<()> {
    let mut faults = Vec::new();
    if let Some(code) = args.status {
        faults.push(HttpFaultType::Status {
            code,
            body: args.status_body.unwrap_or_default(),
        });
    }
    if let Some(value) = &args.latency {
        faults.push(HttpFaultType::Latency {
            delay: parse_duration(value, "latency")?,
        });
    }
    if let Some(value) = &args.stream_delay {
        faults.push(HttpFaultType::StreamDelay {
            chunk_delay: parse_duration(value, "stream-delay")?,
        });
    }
    if let Some(after_events) = args.stream_abort {
        faults.push(HttpFaultType::StreamAbort { after_events });
    }
    if args.malformed_tool_call {
        faults.push(HttpFaultType::MalformedToolCall);
    }
    if let Some(keep_last_items) = args.context_keep {
        faults.push(HttpFaultType::ContextTruncate { keep_last_items });
    }
    if let Some(bytes) = args.truncate_body {
        faults.push(HttpFaultType::TruncateBody { bytes });
    }
    if args.malformed_json {
        faults.push(HttpFaultType::MalformedJson);
    }
    if args.malformed_headers {
        faults.push(HttpFaultType::MalformedHeaders);
    }
    if args.empty_response {
        faults.push(HttpFaultType::EmptyResponse);
    }
    if !args.strip_header.is_empty() {
        faults.push(HttpFaultType::StripHeaders {
            headers: args.strip_header,
        });
    }
    if let Some(value) = &args.slowloris {
        faults.push(HttpFaultType::Slowloris {
            chunk_delay: parse_duration(value, "slowloris")?,
        });
    }
    if faults.is_empty() {
        bail!("Configure at least one HTTP or AI fault");
    }

    let config = HttpFaultConfig {
        listen: args.listen,
        upstream_url: args.upstream.clone(),
        path_pattern: args.path,
        provider: args.provider.into(),
        faults,
        rate: args.rate,
    };
    config.validate()?;
    let injector = Arc::new(HttpFaultInjector::new(config));
    let journal = Arc::new(RecoveryJournal::new(RecoveryJournal::default_path()));
    let executor = Executor::with_defaults_and_journal(journal);
    let handle = executor
        .inject_with(injector.clone(), &Target::System)
        .await?;
    let listen = handle.metadata["listen"]
        .as_str()
        .context("HTTP proxy did not report its listen address")?;

    println!("{}", "=== AI / HTTP Fault Proxy ===".bold().cyan());
    println!("Listen:   http://{}", listen.green());
    println!("Upstream: {}", args.upstream);
    println!("Provider: {:?}", AiProvider::from(args.provider));
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
        "Stopped: {} requests, {} injected, {} stream events dropped, {} contexts truncated",
        metrics.requests,
        metrics.injected_requests,
        metrics.stream_events_dropped,
        metrics.contexts_truncated
    );
    Ok(())
}

fn parse_duration(value: &str, name: &str) -> Result<std::time::Duration> {
    humantime::parse_duration(value)
        .with_context(|| format!("Invalid {} duration '{}'", name, value))
}
