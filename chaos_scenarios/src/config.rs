use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(with = "humantime_serde")]
    pub duration: Duration,
    #[serde(with = "humantime_serde_option", default)]
    pub ramp_up: Option<Duration>,
    #[serde(default)]
    pub phases: Vec<Phase>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phase {
    pub name: String,
    #[serde(with = "humantime_serde")]
    pub duration: Duration,
    #[serde(default)]
    pub injections: Vec<InjectionConfig>,
    #[serde(default)]
    pub parallel: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionConfig {
    pub r#type: String,
    #[serde(default)]
    pub target: TargetConfig,
    #[serde(flatten)]
    pub parameters: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TargetConfig {
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub container_id: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
}

impl TargetConfig {
    pub fn to_target(&self) -> Result<chaos_core::Target, String> {
        if let Some(pid) = self.pid {
            Ok(chaos_core::Target::process(pid))
        } else if let Some(addr) = &self.address {
            let socket_addr = addr
                .parse()
                .map_err(|e| format!("Invalid address '{}': {}", addr, e))?;
            Ok(chaos_core::Target::network(socket_addr))
        } else if let Some(id) = &self.container_id {
            Ok(chaos_core::Target::container(id.clone()))
        } else if let Some(pattern) = &self.pattern {
            Ok(chaos_core::Target::process_pattern(pattern.clone()))
        } else {
            Err("No target specified".to_string())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioConfig {
    pub scenario: Scenario,
}

impl Scenario {
    pub fn builder() -> ScenarioBuilder {
        ScenarioBuilder::default()
    }

    pub fn total_duration(&self) -> Duration {
        self.phases.iter().map(|p| p.duration).sum()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("Scenario name cannot be empty".to_string());
        }

        if self.phases.is_empty() {
            return Err("Scenario must have at least one phase".to_string());
        }

        for (i, phase) in self.phases.iter().enumerate() {
            if phase.name.is_empty() {
                return Err(format!("Phase {} name cannot be empty", i));
            }

            if phase.duration.is_zero() {
                return Err(format!("Phase '{}' duration must be > 0", phase.name));
            }

            for (j, injection) in phase.injections.iter().enumerate() {
                if injection.r#type.is_empty() {
                    return Err(format!(
                        "Injection {} in phase '{}' must have a type",
                        j, phase.name
                    ));
                }
            }
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct ScenarioBuilder {
    name: Option<String>,
    description: Option<String>,
    seed: Option<u64>,
    duration: Option<Duration>,
    ramp_up: Option<Duration>,
    phases: Vec<Phase>,
    labels: HashMap<String, String>,
}

impl ScenarioBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn ramp_up(mut self, ramp_up: Duration) -> Self {
        self.ramp_up = Some(ramp_up);
        self
    }

    pub fn add_phase(mut self, phase: Phase) -> Self {
        self.phases.push(phase);
        self
    }

    pub fn label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> Scenario {
        let duration = self
            .duration
            .unwrap_or_else(|| self.phases.iter().map(|p| p.duration).sum());

        Scenario {
            name: self.name.unwrap_or_else(|| "unnamed".to_string()),
            description: self.description,
            seed: self.seed,
            duration,
            ramp_up: self.ramp_up,
            phases: self.phases,
            labels: self.labels,
        }
    }
}

impl Phase {
    pub fn builder() -> PhaseBuilder {
        PhaseBuilder::default()
    }
}

#[derive(Default)]
pub struct PhaseBuilder {
    name: Option<String>,
    duration: Option<Duration>,
    injections: Vec<InjectionConfig>,
    parallel: bool,
}

impl PhaseBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn add_injection(mut self, injection: InjectionConfig) -> Self {
        self.injections.push(injection);
        self
    }

    pub fn parallel(mut self, parallel: bool) -> Self {
        self.parallel = parallel;
        self
    }

    pub fn build(self) -> Phase {
        Phase {
            name: self.name.unwrap_or_else(|| "unnamed".to_string()),
            duration: self.duration.unwrap_or(Duration::from_secs(60)),
            injections: self.injections,
            parallel: self.parallel,
        }
    }
}

mod humantime_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&humantime::format_duration(*duration).to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        humantime::parse_duration(&s).map_err(serde::de::Error::custom)
    }
}

mod humantime_serde_option {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration {
            Some(d) => serializer.serialize_some(&humantime::format_duration(*d).to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(deserializer)?;
        opt.map(|s| humantime::parse_duration(&s).map_err(serde::de::Error::custom))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_builder() {
        let scenario = Scenario::builder()
            .name("test")
            .duration(Duration::from_secs(120))
            .add_phase(
                Phase::builder()
                    .name("phase1")
                    .duration(Duration::from_secs(60))
                    .build(),
            )
            .build();

        assert_eq!(scenario.name, "test");
        assert_eq!(scenario.phases.len(), 1);
    }

    #[test]
    fn test_scenario_validation() {
        let scenario = Scenario::builder()
            .name("valid")
            .add_phase(
                Phase::builder()
                    .name("phase1")
                    .duration(Duration::from_secs(60))
                    .build(),
            )
            .build();

        assert!(scenario.validate().is_ok());

        let invalid = Scenario::builder().build();
        assert!(invalid.validate().is_err());
    }
}
