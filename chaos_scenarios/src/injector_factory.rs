use crate::config::InjectionConfig;
use anyhow::{anyhow, bail, Context, Result};
use chaos_core::{
    CpuStarvationConfig, CpuStarvationInjector, DiskOperation, DiskSlowConfig, DiskSlowInjector,
    DynInjector, MemoryPressureConfig, MemoryPressureInjector, NetworkLatencyInjector,
    PacketLossConfig, PacketLossInjector,
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
}
