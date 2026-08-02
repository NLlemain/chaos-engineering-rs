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
    #[serde(default)]
    pub assertions: Vec<SloAssertionConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SloAssertionConfig {
    pub name: String,
    pub url: String,
    #[serde(default = "default_expected_status")]
    pub expected_status: u16,
    #[serde(with = "humantime_serde", default = "default_probe_interval")]
    pub interval: Duration,
    #[serde(with = "humantime_serde", default = "default_probe_timeout")]
    pub timeout: Duration,
    #[serde(default)]
    pub max_error_rate: f64,
    #[serde(with = "humantime_serde_option", default)]
    pub max_p95_latency: Option<Duration>,
    #[serde(default = "default_min_requests")]
    pub min_requests: usize,
}

fn default_expected_status() -> u16 {
    200
}

fn default_probe_interval() -> Duration {
    Duration::from_secs(1)
}

fn default_probe_timeout() -> Duration {
    Duration::from_secs(2)
}

fn default_min_requests() -> usize {
    1
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
#[serde(deny_unknown_fields)]
pub struct TargetConfig {
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub container_id: Option<String>,
    #[serde(default)]
    pub compose_service: Option<String>,
    #[serde(default)]
    pub compose_file: Option<std::path::PathBuf>,
    #[serde(default)]
    pub compose_project: Option<String>,
    #[serde(default)]
    pub file: Option<std::path::PathBuf>,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub process_name: Option<String>,
    #[serde(default)]
    pub system: bool,
}

impl TargetConfig {
    pub fn to_target(&self) -> Result<chaos_core::Target, String> {
        let target_count = [
            self.pid.is_some(),
            self.address.is_some(),
            self.container_id.is_some(),
            self.compose_service.is_some(),
            self.file.is_some(),
            self.pattern.is_some(),
            self.process_name.is_some(),
            self.system,
        ]
        .into_iter()
        .filter(|is_set| *is_set)
        .count();

        if target_count > 1 {
            return Err("Specify exactly one target field".to_string());
        }

        if self.compose_service.is_none()
            && (self.compose_file.is_some() || self.compose_project.is_some())
        {
            return Err("compose_file and compose_project require compose_service".to_string());
        }

        if let Some(pid) = self.pid {
            Ok(chaos_core::Target::process(pid))
        } else if let Some(addr) = &self.address {
            let socket_addr = addr
                .parse()
                .map_err(|e| format!("Invalid address '{}': {}", addr, e))?;
            Ok(chaos_core::Target::network(socket_addr))
        } else if let Some(id) = &self.container_id {
            Ok(chaos_core::Target::container(id.clone()))
        } else if let Some(service) = &self.compose_service {
            Ok(chaos_core::Target::compose_service(
                service.clone(),
                self.compose_file
                    .clone()
                    .unwrap_or_else(|| "compose.yaml".into()),
                self.compose_project.clone(),
            ))
        } else if let Some(path) = &self.file {
            Ok(chaos_core::Target::file(path))
        } else if let Some(pattern) = &self.pattern {
            Ok(chaos_core::Target::process_pattern(pattern.clone()))
        } else if let Some(process_name) = &self.process_name {
            Ok(chaos_core::Target::process_pattern(process_name.clone()))
        } else {
            Ok(chaos_core::Target::system())
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

        if self.duration.is_zero() {
            return Err("Scenario duration must be > 0".to_string());
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

                injection.target.to_target().map_err(|error| {
                    format!(
                        "Injection {} in phase '{}' has an invalid target: {}",
                        j, phase.name, error
                    )
                })?;
            }
        }

        for assertion in &self.assertions {
            if assertion.name.trim().is_empty() {
                return Err("SLO assertion name cannot be empty".to_string());
            }
            if !assertion.url.starts_with("http://") && !assertion.url.starts_with("https://") {
                return Err(format!(
                    "SLO assertion '{}' URL must use http or https",
                    assertion.name
                ));
            }
            if !(100..=599).contains(&assertion.expected_status) {
                return Err(format!(
                    "SLO assertion '{}' has an invalid expected status",
                    assertion.name
                ));
            }
            if assertion.interval.is_zero() || assertion.timeout.is_zero() {
                return Err(format!(
                    "SLO assertion '{}' interval and timeout must be greater than zero",
                    assertion.name
                ));
            }
            if !assertion.max_error_rate.is_finite()
                || !(0.0..=1.0).contains(&assertion.max_error_rate)
            {
                return Err(format!(
                    "SLO assertion '{}' max_error_rate must be between 0.0 and 1.0",
                    assertion.name
                ));
            }
            if assertion.min_requests == 0 {
                return Err(format!(
                    "SLO assertion '{}' min_requests must be greater than zero",
                    assertion.name
                ));
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
    assertions: Vec<SloAssertionConfig>,
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

    pub fn add_assertion(mut self, assertion: SloAssertionConfig) -> Self {
        self.assertions.push(assertion);
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
            assertions: self.assertions,
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

    #[test]
    fn compose_target_preserves_service_file_and_project() {
        let target: TargetConfig = serde_json::from_value(serde_json::json!({
            "compose_service": "api",
            "compose_file": "deploy/compose.yaml",
            "compose_project": "demo"
        }))
        .unwrap();
        assert!(matches!(
            target.to_target().unwrap(),
            chaos_core::Target::ComposeService { service, file, project }
                if service == "api"
                    && file == std::path::Path::new("deploy/compose.yaml")
                    && project.as_deref() == Some("demo")
        ));
    }

    #[test]
    fn process_name_target_maps_to_process_pattern() {
        let target = TargetConfig {
            process_name: Some("api-service".to_string()),
            ..TargetConfig::default()
        };

        assert_eq!(
            target.to_target(),
            Ok(chaos_core::Target::process_pattern("api-service"))
        );
    }

    #[test]
    fn empty_target_maps_to_system() {
        assert_eq!(
            TargetConfig::default().to_target(),
            Ok(chaos_core::Target::system())
        );
    }

    #[test]
    fn conflicting_target_fields_are_rejected() {
        let target = TargetConfig {
            pid: Some(42),
            system: true,
            ..TargetConfig::default()
        };

        assert!(target.to_target().is_err());
    }
}
