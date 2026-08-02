use crate::config::InjectionConfig;
use anyhow::{anyhow, bail, Context, Result};
use chaos_core::{
    CpuStarvationConfig, CpuStarvationInjector, DependencyProxyConfig, DependencyProxyInjector,
    DirectedToxic, DiskOperation, DiskSlowConfig, DiskSlowInjector, DynInjector,
    MemoryPressureConfig, MemoryPressureInjector, NetworkLatencyInjector, PacketLossConfig,
    PacketLossInjector, ProxyDirection, ProxyToxic, Target,
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
}
