use crate::config::InjectionConfig;
use anyhow::{anyhow, bail, Context, Result};
use chaos_core::{
    AiProvider, ContainerFaultAction, ContainerFaultConfig, ContainerFaultInjector,
    CpuStarvationConfig, CpuStarvationInjector, CryptoFaultConfig, CryptoFaultInjector,
    CryptoFaultType, DatabaseFaultConfig, DatabaseFaultInjector, DatabaseFaultMode,
    DependencyProxyConfig, DependencyProxyInjector, DirectedToxic, DiskOperation, DiskSlowConfig,
    DiskSlowInjector, DnsFaultConfig, DnsFaultInjector, DnsFaultMode, DynInjector, HttpFaultConfig,
    HttpFaultInjector, HttpFaultType, LocalDatabaseEngine, MemoryPressureConfig,
    MemoryPressureInjector, NetworkLatencyInjector, PacketLossConfig, PacketLossInjector,
    ProxyDirection, ProxyToxic, Target, WindowsFaultConfig, WindowsFaultInjector, WindowsFaultMode,
};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};

pub fn build_injector(config: &InjectionConfig) -> Result<Option<DynInjector>> {
    let parameters = &config.parameters;

    match config.r#type.as_str() {
        "cpu_starvation" => {
            ensure_allowed(parameters, &["intensity", "threads"])?;
            let mut injector_config = CpuStarvationConfig::default();
            if let Some(intensity) = number(parameters, "intensity")? {
                injector_config.intensity = probability("intensity", intensity)?;
            }
            if let Some(threads) = u32_list(parameters, "threads")? {
                injector_config.threads = threads;
            }
            Ok(Some(Arc::new(CpuStarvationInjector::new(injector_config))))
        }
        "memory_pressure" => {
            ensure_allowed(parameters, &["target_usage", "failure_rate"])?;
            let mut injector_config = MemoryPressureConfig::default();
            if let Some(target_usage) = number(parameters, "target_usage")? {
                injector_config.target_usage = probability("target_usage", target_usage)?;
            }
            if let Some(failure_rate) = number(parameters, "failure_rate")? {
                injector_config.failure_rate = probability("failure_rate", failure_rate)?;
            }
            Ok(Some(Arc::new(MemoryPressureInjector::new(injector_config))))
        }
        "network_latency" => {
            ensure_allowed(parameters, &["delay", "mean", "jitter", "correlation"])?;
            if parameters.contains_key("delay") && parameters.contains_key("mean") {
                bail!("Use either 'delay' or 'mean' for network_latency, not both");
            }

            let mut builder = NetworkLatencyInjector::builder();
            if let Some(mean) = duration(parameters, "delay")?.or(duration(parameters, "mean")?) {
                builder = builder.mean(mean);
            }
            if let Some(jitter) = duration(parameters, "jitter")? {
                builder = builder.jitter(jitter);
            }
            if let Some(correlation) = number(parameters, "correlation")? {
                builder = builder.correlation(probability("correlation", correlation)?);
            }
            Ok(Some(Arc::new(builder.build())))
        }
        "packet_loss" => {
            ensure_allowed(parameters, &["loss_rate", "rate", "correlation"])?;
            if parameters.contains_key("loss_rate") && parameters.contains_key("rate") {
                bail!("Use either 'loss_rate' or 'rate' for packet_loss, not both");
            }

            let mut injector_config = PacketLossConfig::default();
            if let Some(rate) = number(parameters, "loss_rate")?.or(number(parameters, "rate")?) {
                injector_config.rate = probability("loss_rate", rate)?;
            }
            if let Some(correlation) = number(parameters, "correlation")? {
                injector_config.correlation = probability("correlation", correlation)?;
            }
            Ok(Some(Arc::new(PacketLossInjector::from_config(
                injector_config,
            ))))
        }
        "disk_slow" => {
            ensure_allowed(parameters, &["latency", "operations"])?;
            let mut injector_config = DiskSlowConfig::default();
            if let Some(latency) = duration(parameters, "latency")? {
                injector_config.latency = latency;
            }
            if let Some(operations) = disk_operations(parameters)? {
                injector_config.operations = operations;
            }
            Ok(Some(Arc::new(DiskSlowInjector::new(injector_config))))
        }
        "dependency_proxy" => {
            build_dependency_proxy(config).map(|injector| Some(Arc::new(injector) as DynInjector))
        }
        "http_fault" => {
            build_http_fault(config).map(|injector| Some(Arc::new(injector) as DynInjector))
        }
        "dns_fault" => {
            build_dns_fault(config).map(|injector| Some(Arc::new(injector) as DynInjector))
        }
        "crypto_fault" => {
            build_crypto_fault(config).map(|injector| Some(Arc::new(injector) as DynInjector))
        }
        "container_fault" => {
            build_container_fault(config).map(|injector| Some(Arc::new(injector) as DynInjector))
        }
        "database_fault" => {
            build_database_fault(config).map(|injector| Some(Arc::new(injector) as DynInjector))
        }
        "windows_fault" => {
            build_windows_fault(config).map(|injector| Some(Arc::new(injector) as DynInjector))
        }
        _ if parameters.is_empty() => Ok(None),
        _ => {
            let mut keys: Vec<_> = parameters.keys().cloned().collect();
            keys.sort();
            bail!(
                "Injector '{}' does not support scenario parameters yet: {}",
                config.r#type,
                keys.join(", ")
            )
        }
    }
}

fn build_windows_fault(config: &InjectionConfig) -> Result<WindowsFaultInjector> {
    let parameters = &config.parameters;
    ensure_allowed(parameters, &["mode", "service", "count", "pipe_name"])?;
    let target = config.target.to_target().map_err(anyhow::Error::msg)?;
    let mode = match required_string(parameters, "mode")?
        .to_ascii_lowercase()
        .replace('-', "_")
        .as_str()
    {
        "service_stop" | "service_outage" => {
            if !matches!(target, Target::System) {
                bail!("Windows service_stop requires target.system");
            }
            WindowsFaultMode::ServiceStop {
                service: required_string(parameters, "service")?.to_string(),
            }
        }
        "file_lock" | "locked_file" => {
            if !matches!(target, Target::File { .. }) {
                bail!("Windows file_lock requires target.file");
            }
            WindowsFaultMode::FileLock
        }
        "handle_exhaustion" | "handle_pressure" => {
            if !matches!(target, Target::System) {
                bail!("Windows handle_exhaustion requires target.system");
            }
            WindowsFaultMode::HandleExhaustion {
                count: usize::try_from(u64_value(parameters, "count")?.unwrap_or(4096))
                    .context("Parameter 'count' is too large")?,
            }
        }
        "named_pipe_blackhole" | "named_pipe_disruption" => {
            if !matches!(target, Target::System) {
                bail!("Windows named_pipe_blackhole requires target.system");
            }
            WindowsFaultMode::NamedPipeBlackhole {
                pipe_name: required_string(parameters, "pipe_name")?.to_string(),
            }
        }
        value => bail!(
            "Parameter 'mode' must be service_stop, file_lock, handle_exhaustion, or named_pipe_blackhole; got '{}'",
            value
        ),
    };
    let windows_config = WindowsFaultConfig { mode };
    windows_config.validate()?;
    Ok(WindowsFaultInjector::new(windows_config))
}

fn build_database_fault(config: &InjectionConfig) -> Result<DatabaseFaultInjector> {
    let parameters = &config.parameters;
    ensure_allowed(
        parameters,
        &["engine", "mode", "bytes_per_cycle", "cycle_delay", "files"],
    )?;
    if !matches!(
        config.target.to_target().map_err(anyhow::Error::msg)?,
        Target::File { .. }
    ) {
        bail!("database_fault requires target.file");
    }
    let engine = match required_string(parameters, "engine")?
        .to_ascii_lowercase()
        .replace('-', "_")
        .as_str()
    {
        "duckdb" | "duck_db" => LocalDatabaseEngine::DuckDb,
        "sqlite" | "sqlite3" => LocalDatabaseEngine::Sqlite,
        value => bail!(
            "Parameter 'engine' must be duckdb or sqlite; got '{}'",
            value
        ),
    };
    let mode = match required_string(parameters, "mode")?
        .to_ascii_lowercase()
        .replace('-', "_")
        .as_str()
    {
        "unavailable" => DatabaseFaultMode::Unavailable,
        "read_only" | "readonly" => DatabaseFaultMode::ReadOnly,
        "lock" => DatabaseFaultMode::Lock,
        "io_pressure" => DatabaseFaultMode::IoPressure {
            bytes_per_cycle: usize::try_from(
                u64_value(parameters, "bytes_per_cycle")?.unwrap_or(1024 * 1024),
            )
            .context("Parameter 'bytes_per_cycle' is too large")?,
            cycle_delay: duration(parameters, "cycle_delay")?
                .unwrap_or(Duration::from_millis(10)),
        },
        "inode_pressure" => DatabaseFaultMode::InodePressure {
            files: usize::try_from(u64_value(parameters, "files")?.unwrap_or(1_000))
                .context("Parameter 'files' is too large")?,
        },
        value => bail!(
            "Parameter 'mode' must be unavailable, read_only, lock, io_pressure, or inode_pressure; got '{}'",
            value
        ),
    };
    let database_config = DatabaseFaultConfig { engine, mode };
    database_config.validate()?;
    Ok(DatabaseFaultInjector::new(database_config))
}

fn build_container_fault(config: &InjectionConfig) -> Result<ContainerFaultInjector> {
    let parameters = &config.parameters;
    ensure_allowed(parameters, &["action", "stop_timeout_seconds"])?;
    if !matches!(
        config.target.to_target().map_err(anyhow::Error::msg)?,
        Target::Container { .. } | Target::ComposeService { .. }
    ) {
        bail!("container_fault requires target.container_id or target.compose_service");
    }
    let action = match required_string(parameters, "action")?
        .to_ascii_lowercase()
        .as_str()
    {
        "pause" => ContainerFaultAction::Pause,
        "stop" => ContainerFaultAction::Stop,
        "kill" => ContainerFaultAction::Kill,
        "restart" => ContainerFaultAction::Restart,
        value => bail!(
            "Parameter 'action' must be pause, stop, kill, or restart; got '{}'",
            value
        ),
    };
    Ok(ContainerFaultInjector::new(ContainerFaultConfig {
        action,
        stop_timeout_seconds: u64_value(parameters, "stop_timeout_seconds")?.unwrap_or(10),
    }))
}

fn build_dns_fault(config: &InjectionConfig) -> Result<DnsFaultInjector> {
    let parameters = &config.parameters;
    ensure_allowed(
        parameters,
        &[
            "listen",
            "domain_pattern",
            "mode",
            "delay",
            "fake_ip",
            "failure_rate",
            "upstream_timeout",
        ],
    )?;
    let Target::Network { address: upstream } =
        config.target.to_target().map_err(anyhow::Error::msg)?
    else {
        bail!("dns_fault requires target.address for its upstream resolver");
    };
    let listen = required_string(parameters, "listen")?
        .parse()
        .context("Parameter 'listen' must be a socket address")?;
    let mode = match required_string(parameters, "mode")?
        .to_ascii_lowercase()
        .replace('-', "_")
        .as_str()
    {
        "latency" => DnsFaultMode::Latency {
            delay: duration(parameters, "delay")?
                .context("DNS latency mode requires parameter 'delay'")?,
        },
        "nxdomain" | "nx_domain" => DnsFaultMode::NxDomain,
        "spoof" => DnsFaultMode::Spoof {
            fake_ip: required_string(parameters, "fake_ip")?.to_string(),
        },
        "blackhole" => DnsFaultMode::Blackhole,
        value => bail!(
            "Parameter 'mode' must be latency, nxdomain, spoof, or blackhole; got '{}'",
            value
        ),
    };
    let dns_config = DnsFaultConfig {
        listen,
        upstream,
        domain_pattern: string(parameters, "domain_pattern")?
            .unwrap_or("*")
            .to_string(),
        fault_mode: mode,
        failure_rate: number(parameters, "failure_rate")?
            .map(|value| probability("failure_rate", value))
            .transpose()?
            .unwrap_or(1.0),
        upstream_timeout: duration(parameters, "upstream_timeout")?
            .unwrap_or(Duration::from_secs(2)),
    };
    dns_config.validate()?;
    Ok(DnsFaultInjector::new(dns_config))
}

fn build_crypto_fault(config: &InjectionConfig) -> Result<CryptoFaultInjector> {
    let parameters = &config.parameters;
    ensure_allowed(parameters, &["listen", "domain", "mode", "delay"])?;
    if !matches!(
        config.target.to_target().map_err(anyhow::Error::msg)?,
        Target::System
    ) {
        bail!("crypto_fault requires target.system");
    }
    let fault_type = match required_string(parameters, "mode")?
        .to_ascii_lowercase()
        .replace('-', "_")
        .as_str()
    {
        "cert_expired" | "expired" => CryptoFaultType::CertExpired,
        "untrusted_ca" | "untrusted" => CryptoFaultType::UntrustedCa,
        "incomplete_chain" => CryptoFaultType::IncompleteChain,
        "handshake_abort" | "abort" => CryptoFaultType::HandshakeAbort,
        "handshake_delay" | "delay" => CryptoFaultType::HandshakeDelay {
            delay: duration(parameters, "delay")?
                .context("TLS handshake delay mode requires parameter 'delay'")?,
        },
        value => bail!(
            "Parameter 'mode' must be cert_expired, untrusted_ca, incomplete_chain, handshake_abort, or handshake_delay; got '{}'",
            value
        ),
    };
    let crypto_config = CryptoFaultConfig {
        listen: required_string(parameters, "listen")?
            .parse()
            .context("Parameter 'listen' must be a socket address")?,
        fault_type,
        target_cert_domain: string(parameters, "domain")?
            .unwrap_or("localhost")
            .to_string(),
    };
    crypto_config.validate()?;
    Ok(CryptoFaultInjector::new(crypto_config))
}

fn build_http_fault(config: &InjectionConfig) -> Result<HttpFaultInjector> {
    let parameters = &config.parameters;
    ensure_allowed(
        parameters,
        &[
            "listen",
            "upstream",
            "provider",
            "path_pattern",
            "rate",
            "status",
            "status_body",
            "latency",
            "stream_delay",
            "stream_abort",
            "malformed_tool_call",
            "context_keep",
            "truncate_body",
            "replace_body",
            "replace_content_type",
            "malformed_json",
            "malformed_headers",
            "empty_response",
            "strip_headers",
            "slowloris",
        ],
    )?;

    let listen = required_string(parameters, "listen")?
        .parse()
        .context("Parameter 'listen' must be a socket address")?;
    let provider = string(parameters, "provider")?
        .unwrap_or("generic")
        .parse::<AiProvider>()?;
    let mut faults = Vec::new();

    if let Some(status) = u64_value(parameters, "status")? {
        faults.push(HttpFaultType::Status {
            code: u16::try_from(status).context("Parameter 'status' must fit in u16")?,
            body: string(parameters, "status_body")?
                .unwrap_or_default()
                .to_string(),
        });
    } else if parameters.contains_key("status_body") {
        bail!("Parameter 'status_body' requires 'status'");
    }
    if let Some(delay) = duration(parameters, "latency")? {
        faults.push(HttpFaultType::Latency { delay });
    }
    if let Some(chunk_delay) = duration(parameters, "stream_delay")? {
        faults.push(HttpFaultType::StreamDelay { chunk_delay });
    }
    if let Some(after_events) = u64_value(parameters, "stream_abort")? {
        faults.push(HttpFaultType::StreamAbort {
            after_events: usize::try_from(after_events)
                .context("Parameter 'stream_abort' is too large")?,
        });
    }
    if boolean(parameters, "malformed_tool_call")?.unwrap_or(false) {
        faults.push(HttpFaultType::MalformedToolCall);
    }
    if let Some(keep_last_items) = u64_value(parameters, "context_keep")? {
        faults.push(HttpFaultType::ContextTruncate {
            keep_last_items: usize::try_from(keep_last_items)
                .context("Parameter 'context_keep' is too large")?,
        });
    }
    if let Some(bytes) = u64_value(parameters, "truncate_body")? {
        faults.push(HttpFaultType::TruncateBody {
            bytes: usize::try_from(bytes).context("Parameter 'truncate_body' is too large")?,
        });
    }
    if let Some(body) = string(parameters, "replace_body")? {
        faults.push(HttpFaultType::ReplaceBody {
            body: body.to_string(),
            content_type: string(parameters, "replace_content_type")?
                .unwrap_or("application/octet-stream")
                .to_string(),
        });
    } else if parameters.contains_key("replace_content_type") {
        bail!("Parameter 'replace_content_type' requires 'replace_body'");
    }
    if boolean(parameters, "malformed_json")?.unwrap_or(false) {
        faults.push(HttpFaultType::MalformedJson);
    }
    if boolean(parameters, "malformed_headers")?.unwrap_or(false) {
        faults.push(HttpFaultType::MalformedHeaders);
    }
    if boolean(parameters, "empty_response")?.unwrap_or(false) {
        faults.push(HttpFaultType::EmptyResponse);
    }
    if let Some(headers) = string_list(parameters, "strip_headers")? {
        faults.push(HttpFaultType::StripHeaders { headers });
    }
    if let Some(chunk_delay) = duration(parameters, "slowloris")? {
        faults.push(HttpFaultType::Slowloris { chunk_delay });
    }

    let http_config = HttpFaultConfig {
        listen,
        upstream_url: required_string(parameters, "upstream")?.to_string(),
        path_pattern: string(parameters, "path_pattern")?
            .unwrap_or("/*")
            .to_string(),
        provider,
        faults,
        rate: number(parameters, "rate")?
            .map(|value| probability("rate", value))
            .transpose()?
            .unwrap_or(1.0),
    };
    http_config.validate()?;
    Ok(HttpFaultInjector::new(http_config))
}

fn build_dependency_proxy(config: &InjectionConfig) -> Result<DependencyProxyInjector> {
    let parameters = &config.parameters;
    ensure_allowed(
        parameters,
        &[
            "listen",
            "direction",
            "toxicity",
            "latency",
            "jitter",
            "bandwidth_bps",
            "timeout",
            "slow_close",
            "limit_bytes",
            "max_connections",
            "partition",
            "corruption_rate",
            "duplicate_rate",
            "reorder_rate",
            "reorder_delay",
        ],
    )?;

    let Target::Network { address: upstream } =
        config.target.to_target().map_err(anyhow::Error::msg)?
    else {
        bail!("dependency_proxy requires target.address");
    };
    let listen = required_string(parameters, "listen")?
        .parse()
        .context("Parameter 'listen' must be a socket address")?;
    let direction = match string(parameters, "direction")?
        .unwrap_or("both")
        .to_ascii_lowercase()
        .as_str()
    {
        "upstream" => ProxyDirection::Upstream,
        "downstream" => ProxyDirection::Downstream,
        "both" => ProxyDirection::Both,
        _ => bail!("Parameter 'direction' must be upstream, downstream, or both"),
    };
    let toxicity = number(parameters, "toxicity")?
        .map(|value| probability("toxicity", value))
        .transpose()?
        .unwrap_or(1.0);
    let mut proxy = DependencyProxyConfig::new(listen, upstream);
    let mut push = |toxic| {
        proxy
            .toxics
            .push(DirectedToxic::new(direction, toxic).with_toxicity(toxicity));
    };

    if let Some(delay) = duration(parameters, "latency")? {
        let jitter = duration(parameters, "jitter")?.unwrap_or_default();
        push(ProxyToxic::Latency {
            delay_ms: duration_as_millis("latency", delay)?,
            jitter_ms: duration_as_millis("jitter", jitter)?,
        });
    } else if parameters.contains_key("jitter") {
        bail!("Parameter 'jitter' requires 'latency'");
    }
    if let Some(bytes_per_second) = u64_value(parameters, "bandwidth_bps")? {
        push(ProxyToxic::Bandwidth { bytes_per_second });
    }
    if let Some(timeout) = duration(parameters, "timeout")? {
        push(ProxyToxic::Timeout {
            timeout_ms: duration_as_millis("timeout", timeout)?,
        });
    }
    if let Some(delay) = duration(parameters, "slow_close")? {
        push(ProxyToxic::SlowClose {
            delay_ms: duration_as_millis("slow_close", delay)?,
        });
    }
    if let Some(bytes) = u64_value(parameters, "limit_bytes")? {
        push(ProxyToxic::LimitData { bytes });
    }
    if let Some(connections) = u64_value(parameters, "max_connections")? {
        push(ProxyToxic::ConnectionLimit { connections });
    }
    if boolean(parameters, "partition")?.unwrap_or(false) {
        push(ProxyToxic::Partition);
    }
    if let Some(value) = number(parameters, "corruption_rate")? {
        push(ProxyToxic::Corrupt {
            probability: probability("corruption_rate", value)?,
        });
    }
    if let Some(value) = number(parameters, "duplicate_rate")? {
        push(ProxyToxic::Duplicate {
            probability: probability("duplicate_rate", value)?,
        });
    }
    if let Some(value) = number(parameters, "reorder_rate")? {
        let delay =
            duration(parameters, "reorder_delay")?.unwrap_or_else(|| Duration::from_millis(10));
        push(ProxyToxic::Reorder {
            probability: probability("reorder_rate", value)?,
            delay_ms: duration_as_millis("reorder_delay", delay)?,
        });
    } else if parameters.contains_key("reorder_delay") {
        bail!("Parameter 'reorder_delay' requires 'reorder_rate'");
    }

    proxy.validate()?;
    Ok(DependencyProxyInjector::new(proxy))
}

fn ensure_allowed(parameters: &HashMap<String, Value>, allowed: &[&str]) -> Result<()> {
    let mut unknown: Vec<_> = parameters
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .cloned()
        .collect();
    unknown.sort();

    if unknown.is_empty() {
        Ok(())
    } else {
        bail!("Unsupported parameter(s): {}", unknown.join(", "))
    }
}

fn number(parameters: &HashMap<String, Value>, key: &str) -> Result<Option<f64>> {
    parameters
        .get(key)
        .map(|value| {
            value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| anyhow!("Parameter '{}' must be a finite number", key))
        })
        .transpose()
}

fn string<'a>(parameters: &'a HashMap<String, Value>, key: &str) -> Result<Option<&'a str>> {
    parameters
        .get(key)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| anyhow!("Parameter '{}' must be a string", key))
        })
        .transpose()
}

fn required_string<'a>(parameters: &'a HashMap<String, Value>, key: &str) -> Result<&'a str> {
    string(parameters, key)?.with_context(|| format!("Missing required parameter '{}'", key))
}

fn boolean(parameters: &HashMap<String, Value>, key: &str) -> Result<Option<bool>> {
    parameters
        .get(key)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| anyhow!("Parameter '{}' must be true or false", key))
        })
        .transpose()
}

fn u64_value(parameters: &HashMap<String, Value>, key: &str) -> Result<Option<u64>> {
    parameters
        .get(key)
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| anyhow!("Parameter '{}' must be a positive integer", key))
        })
        .transpose()
}

fn string_list(parameters: &HashMap<String, Value>, key: &str) -> Result<Option<Vec<String>>> {
    parameters
        .get(key)
        .map(|value| {
            value
                .as_array()
                .ok_or_else(|| anyhow!("Parameter '{}' must be an array", key))?
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| anyhow!("Parameter '{}' must contain strings", key))
                })
                .collect()
        })
        .transpose()
}

fn duration_as_millis(name: &str, duration: Duration) -> Result<u64> {
    u64::try_from(duration.as_millis())
        .with_context(|| format!("Parameter '{}' duration is too large", name))
}

fn probability(key: &str, value: f64) -> Result<f64> {
    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        bail!("Parameter '{}' must be between 0.0 and 1.0", key)
    }
}

fn u32_list(parameters: &HashMap<String, Value>, key: &str) -> Result<Option<Vec<u32>>> {
    parameters
        .get(key)
        .map(|value| {
            let values = value
                .as_array()
                .ok_or_else(|| anyhow!("Parameter '{}' must be an array", key))?;
            values
                .iter()
                .map(|value| {
                    let value = value
                        .as_u64()
                        .ok_or_else(|| anyhow!("Parameter '{}' must contain integers", key))?;
                    u32::try_from(value)
                        .with_context(|| format!("Parameter '{}' contains a value above u32", key))
                })
                .collect()
        })
        .transpose()
}

fn duration(parameters: &HashMap<String, Value>, key: &str) -> Result<Option<Duration>> {
    parameters
        .get(key)
        .map(|value| {
            let value = value
                .as_str()
                .ok_or_else(|| anyhow!("Parameter '{}' must be a duration string", key))?;
            humantime::parse_duration(value)
                .with_context(|| format!("Invalid duration for parameter '{}': {}", key, value))
        })
        .transpose()
}

fn disk_operations(parameters: &HashMap<String, Value>) -> Result<Option<Vec<DiskOperation>>> {
    let Some(value) = parameters.get("operations") else {
        return Ok(None);
    };
    let values = value
        .as_array()
        .ok_or_else(|| anyhow!("Parameter 'operations' must be an array"))?;
    let operations = values
        .iter()
        .map(
            |value| match value.as_str().map(str::to_ascii_lowercase).as_deref() {
                Some("read") => Ok(DiskOperation::Read),
                Some("write") => Ok(DiskOperation::Write),
                Some("fsync") => Ok(DiskOperation::Fsync),
                Some("open") => Ok(DiskOperation::Open),
                Some("all") => Ok(DiskOperation::All),
                _ => bail!("Disk operation must be read, write, fsync, open, or all"),
            },
        )
        .collect::<Result<Vec<_>>>()?;

    if operations.is_empty() {
        bail!("Parameter 'operations' cannot be empty");
    }
    Ok(Some(operations))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_scenario_parameters_build_injectors() {
        for yaml in [
            include_str!("../../scenarios/quick_test.yaml"),
            include_str!("../../scenarios/stress_test.yaml"),
        ] {
            let scenario = crate::parse_scenario_from_str(yaml, "yaml").unwrap();
            for injection in scenario.phases.iter().flat_map(|phase| &phase.injections) {
                assert!(build_injector(injection).unwrap().is_some());
            }
        }
    }

    #[test]
    fn unknown_parameter_is_rejected() {
        let config: InjectionConfig = serde_json::from_value(serde_json::json!({
            "type": "cpu_starvation",
            "mystery": 1
        }))
        .unwrap();

        let error = match build_injector(&config) {
            Err(error) => error,
            Ok(_) => panic!("unknown parameter was accepted"),
        };
        assert!(error.to_string().contains("mystery"));
    }

    #[test]
    fn invalid_probability_is_rejected() {
        let config: InjectionConfig = serde_json::from_value(serde_json::json!({
            "type": "packet_loss",
            "loss_rate": 1.5
        }))
        .unwrap();

        assert!(build_injector(&config).is_err());
    }

    #[test]
    fn dependency_proxy_parameters_build_real_toxics() {
        let config: InjectionConfig = serde_json::from_value(serde_json::json!({
            "type": "dependency_proxy",
            "target": { "address": "127.0.0.1:5432" },
            "listen": "127.0.0.1:15432",
            "direction": "downstream",
            "latency": "75ms",
            "bandwidth_bps": 4096,
            "limit_bytes": 8192
        }))
        .unwrap();

        assert!(build_injector(&config).unwrap().is_some());
    }

    #[test]
    fn ai_http_parameters_support_multiple_provider_faults() {
        let config: InjectionConfig = serde_json::from_value(serde_json::json!({
            "type": "http_fault",
            "target": { "system": true },
            "listen": "127.0.0.1:18080",
            "upstream": "https://api.anthropic.com",
            "provider": "anthropic",
            "stream_delay": "100ms",
            "stream_abort": 3,
            "malformed_tool_call": true,
            "context_keep": 2
        }))
        .unwrap();

        assert!(build_injector(&config).unwrap().is_some());
    }

    #[test]
    fn bundled_ai_packs_parse_and_build() {
        for yaml in [
            include_str!("../../scenario-packs/ai/openai-compatible.yaml"),
            include_str!("../../scenario-packs/ai/anthropic.yaml"),
            include_str!("../../scenario-packs/ai/gemini.yaml"),
            include_str!("../../scenario-packs/ai/openrouter.yaml"),
            include_str!("../../scenario-packs/ai/ollama.yaml"),
        ] {
            let scenario = crate::parse_scenario_from_str(yaml, "yaml").unwrap();
            for injection in scenario.phases.iter().flat_map(|phase| &phase.injections) {
                assert!(build_injector(injection).unwrap().is_some());
            }
        }
    }

    #[test]
    fn bundled_dns_and_authentication_packs_parse_and_build() {
        for yaml in [
            include_str!("../../scenario-packs/network/dns-spoof.yaml"),
            include_str!("../../scenario-packs/authentication/expired-certificate.yaml"),
            include_str!("../../scenario-packs/authentication/incomplete-chain.yaml"),
            include_str!("../../scenario-packs/authentication/jwks-outage.yaml"),
            include_str!("../../scenario-packs/authentication/oauth-refresh-failure.yaml"),
        ] {
            let scenario = crate::parse_scenario_from_str(yaml, "yaml").unwrap();
            for injection in scenario.phases.iter().flat_map(|phase| &phase.injections) {
                assert!(build_injector(injection).unwrap().is_some());
            }
        }
    }

    #[test]
    fn bundled_container_pack_parses_and_builds() {
        let scenario = crate::parse_scenario_from_str(
            include_str!("../../scenario-packs/containers/compose-pause.yaml"),
            "yaml",
        )
        .unwrap();
        for injection in scenario.phases.iter().flat_map(|phase| &phase.injections) {
            assert!(build_injector(injection).unwrap().is_some());
        }
    }

    #[test]
    fn bundled_database_packs_parse_and_build() {
        for yaml in [
            include_str!("../../scenario-packs/databases/duckdb-unavailable.yaml"),
            include_str!("../../scenario-packs/databases/duckdb-io-pressure.yaml"),
            include_str!("../../scenario-packs/databases/sqlite-read-only.yaml"),
            include_str!("../../scenario-packs/databases/postgres-disconnect.yaml"),
            include_str!("../../scenario-packs/databases/mysql-slow-queries.yaml"),
            include_str!("../../scenario-packs/databases/pool-exhaustion.yaml"),
        ] {
            let scenario = crate::parse_scenario_from_str(yaml, "yaml").unwrap();
            for injection in scenario.phases.iter().flat_map(|phase| &phase.injections) {
                assert!(build_injector(injection).unwrap().is_some());
            }
        }
    }

    #[test]
    fn every_downloadable_scenario_pack_parses_and_builds() {
        fn yaml_files(directory: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    yaml_files(&path, files);
                } else if matches!(
                    path.extension().and_then(|value| value.to_str()),
                    Some("yaml" | "yml")
                ) {
                    files.push(path);
                }
            }
        }

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scenario-packs");
        let mut files = Vec::new();
        yaml_files(&root, &mut files);
        assert!(files.len() >= 30, "expected the complete scenario catalog");

        for file in files {
            let yaml = std::fs::read_to_string(&file).unwrap();
            let scenario = crate::parse_scenario_from_str(&yaml, "yaml")
                .unwrap_or_else(|error| panic!("{}: {}", file.display(), error));
            for injection in scenario.phases.iter().flat_map(|phase| &phase.injections) {
                build_injector(injection).unwrap_or_else(|error| {
                    panic!("{} ({}): {}", file.display(), injection.r#type, error)
                });
            }
        }
    }
}
