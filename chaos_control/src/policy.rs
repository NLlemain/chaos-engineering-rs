use chaos_core::Target;
use chaos_scenarios::Scenario;
use chrono::{DateTime, Datelike, Timelike, Utc, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExperimentPolicy {
    pub allowed_injectors: Vec<String>,
    pub allowed_target_kinds: Vec<String>,
    pub allowed_target_patterns: Vec<String>,
    pub max_targets: usize,
    pub max_parallel_targets: usize,
    pub max_blast_radius_percent: u8,
    #[serde(with = "duration_serde")]
    pub max_scenario_duration: Duration,
    pub schedule: SchedulePolicy,
    pub slo: SloPolicy,
}

impl Default for ExperimentPolicy {
    fn default() -> Self {
        Self {
            allowed_injectors: vec!["*".to_string()],
            allowed_target_kinds: vec![
                "process".into(),
                "network".into(),
                "container".into(),
                "compose_service".into(),
                "file".into(),
                "kubernetes".into(),
                "thread".into(),
                "process_pattern".into(),
                "system".into(),
            ],
            allowed_target_patterns: vec!["*".to_string()],
            max_targets: 10,
            max_parallel_targets: 2,
            max_blast_radius_percent: 25,
            max_scenario_duration: Duration::from_secs(15 * 60),
            schedule: SchedulePolicy::default(),
            slo: SloPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SchedulePolicy {
    pub not_before: Option<DateTime<Utc>>,
    pub not_after: Option<DateTime<Utc>>,
    pub allowed_utc_hours: Vec<u8>,
    pub denied_weekdays: Vec<String>,
}

impl Default for SchedulePolicy {
    fn default() -> Self {
        Self {
            not_before: None,
            not_after: None,
            allowed_utc_hours: (0..24).collect(),
            denied_weekdays: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SloPolicy {
    pub max_assertions: usize,
    pub max_error_rate: f64,
    #[serde(with = "optional_duration_serde")]
    pub max_p95_latency: Option<Duration>,
}

impl Default for SloPolicy {
    fn default() -> Self {
        Self {
            max_assertions: 20,
            max_error_rate: 0.25,
            max_p95_latency: Some(Duration::from_secs(5)),
        }
    }
}

impl ExperimentPolicy {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.allowed_injectors.is_empty(),
            "policy must allow at least one injector"
        );
        anyhow::ensure!(
            !self.allowed_target_kinds.is_empty(),
            "policy must allow at least one target kind"
        );
        anyhow::ensure!(
            !self.allowed_target_patterns.is_empty(),
            "policy must allow at least one target pattern"
        );
        anyhow::ensure!(
            self.max_targets > 0,
            "policy max_targets must be greater than zero"
        );
        anyhow::ensure!(
            self.max_parallel_targets > 0,
            "policy max_parallel_targets must be greater than zero"
        );
        anyhow::ensure!(
            (1..=100).contains(&self.max_blast_radius_percent),
            "policy blast radius must be between 1 and 100 percent"
        );
        anyhow::ensure!(
            !self.max_scenario_duration.is_zero(),
            "policy scenario duration must be greater than zero"
        );
        anyhow::ensure!(
            self.slo.max_assertions > 0,
            "policy SLO assertion limit must be greater than zero"
        );
        anyhow::ensure!(
            self.slo.max_error_rate.is_finite() && (0.0..=1.0).contains(&self.slo.max_error_rate),
            "policy SLO error rate must be between 0.0 and 1.0"
        );
        anyhow::ensure!(
            self.schedule
                .allowed_utc_hours
                .iter()
                .all(|hour| *hour < 24),
            "policy schedule hours must be between 0 and 23"
        );
        for weekday in &self.schedule.denied_weekdays {
            parse_weekday(weekday)?;
        }
        if let (Some(start), Some(end)) = (self.schedule.not_before, self.schedule.not_after) {
            anyhow::ensure!(
                start <= end,
                "policy schedule not_before must not exceed not_after"
            );
        }
        Ok(())
    }

    pub fn validate_at(&self, scenario: &Scenario, now: DateTime<Utc>) -> anyhow::Result<()> {
        self.validate()?;
        scenario.validate().map_err(anyhow::Error::msg)?;
        anyhow::ensure!(
            scenario.duration <= self.max_scenario_duration,
            "scenario '{}' duration {:?} exceeds policy maximum {:?}",
            scenario.name,
            scenario.duration,
            self.max_scenario_duration
        );
        anyhow::ensure!(
            scenario.assertions.len() <= self.slo.max_assertions,
            "scenario '{}' has {} SLO assertions; policy allows {}",
            scenario.name,
            scenario.assertions.len(),
            self.slo.max_assertions
        );
        self.validate_schedule(now)?;

        for phase in &scenario.phases {
            for injection in &phase.injections {
                anyhow::ensure!(
                    self.allowed_injectors
                        .iter()
                        .any(|pattern| wildcard_match(pattern, &injection.r#type)),
                    "injector '{}' is denied by policy",
                    injection.r#type
                );
                let target = injection.target.to_target().map_err(anyhow::Error::msg)?;
                let kind = target_kind(&target);
                anyhow::ensure!(
                    self.allowed_target_kinds
                        .iter()
                        .any(|allowed| allowed == kind),
                    "target kind '{}' is denied by policy",
                    kind
                );
                let description = target.description();
                anyhow::ensure!(
                    self.allowed_target_patterns
                        .iter()
                        .any(|pattern| wildcard_match(pattern, &description)),
                    "target '{}' is denied by policy",
                    description
                );
            }
        }

        for assertion in &scenario.assertions {
            anyhow::ensure!(
                assertion.max_error_rate <= self.slo.max_error_rate,
                "SLO '{}' permits error rate {:.4}; policy maximum is {:.4}",
                assertion.name,
                assertion.max_error_rate,
                self.slo.max_error_rate
            );
            if let (Some(actual), Some(limit)) =
                (assertion.max_p95_latency, self.slo.max_p95_latency)
            {
                anyhow::ensure!(
                    actual <= limit,
                    "SLO '{}' p95 budget {:?} exceeds policy maximum {:?}",
                    assertion.name,
                    actual,
                    limit
                );
            }
        }
        Ok(())
    }

    pub fn effective_parallel_limit(
        &self,
        requested: usize,
        total_targets: usize,
        requested_radius: u8,
    ) -> anyhow::Result<usize> {
        anyhow::ensure!(
            total_targets > 0,
            "distributed experiment requires at least one target"
        );
        anyhow::ensure!(
            (1..=100).contains(&requested_radius),
            "blast radius must be between 1 and 100 percent"
        );
        anyhow::ensure!(
            requested_radius <= self.max_blast_radius_percent,
            "requested blast radius {}% exceeds policy limit {}%",
            requested_radius,
            self.max_blast_radius_percent
        );
        anyhow::ensure!(
            requested <= self.max_parallel_targets,
            "requested parallelism {} exceeds policy limit {}",
            requested,
            self.max_parallel_targets
        );
        anyhow::ensure!(
            total_targets <= self.max_targets,
            "experiment has {} targets; policy allows {}",
            total_targets,
            self.max_targets
        );
        let radius_targets = total_targets
            .saturating_mul(requested_radius as usize)
            .div_ceil(100)
            .max(1);
        Ok(requested.min(radius_targets).max(1))
    }

    fn validate_schedule(&self, now: DateTime<Utc>) -> anyhow::Result<()> {
        if let Some(start) = self.schedule.not_before {
            anyhow::ensure!(now >= start, "experiments are denied before {start}");
        }
        if let Some(end) = self.schedule.not_after {
            anyhow::ensure!(now <= end, "experiments are denied after {end}");
        }
        anyhow::ensure!(
            self.schedule
                .allowed_utc_hours
                .contains(&(now.hour() as u8)),
            "experiments are denied at UTC hour {}",
            now.hour()
        );
        let denied: HashSet<_> = self
            .schedule
            .denied_weekdays
            .iter()
            .map(|value| parse_weekday(value))
            .collect::<anyhow::Result<_>>()?;
        anyhow::ensure!(
            !denied.contains(&now.weekday()),
            "experiments are denied on {}",
            now.weekday()
        );
        Ok(())
    }
}

fn target_kind(target: &Target) -> &'static str {
    match target {
        Target::Process { .. } => "process",
        Target::Network { .. } => "network",
        Target::Container { .. } => "container",
        Target::ComposeService { .. } => "compose_service",
        Target::File { .. } => "file",
        Target::Kubernetes { .. } => "kubernetes",
        Target::Thread { .. } => "thread",
        Target::ProcessPattern { .. } => "process_pattern",
        Target::System => "system",
    }
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let parts: Vec<_> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == value;
    }
    let mut remainder = value;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        let Some(position) = remainder.find(part) else {
            return false;
        };
        if index == 0 && !pattern.starts_with('*') && position != 0 {
            return false;
        }
        remainder = &remainder[position + part.len()..];
    }
    pattern.ends_with('*') || remainder.is_empty()
}

fn parse_weekday(value: &str) -> anyhow::Result<Weekday> {
    match value.trim().to_ascii_lowercase().as_str() {
        "mon" | "monday" => Ok(Weekday::Mon),
        "tue" | "tuesday" => Ok(Weekday::Tue),
        "wed" | "wednesday" => Ok(Weekday::Wed),
        "thu" | "thursday" => Ok(Weekday::Thu),
        "fri" | "friday" => Ok(Weekday::Fri),
        "sat" | "saturday" => Ok(Weekday::Sat),
        "sun" | "sunday" => Ok(Weekday::Sun),
        _ => anyhow::bail!("unknown weekday '{value}'"),
    }
}

mod duration_serde {
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
        let value = String::deserialize(deserializer)?;
        humantime::parse_duration(&value).map_err(serde::de::Error::custom)
    }
}

mod optional_duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration
            .map(|value| humantime::format_duration(value).to_string())
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| humantime::parse_duration(&value).map_err(serde::de::Error::custom))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chaos_scenarios::config::{InjectionConfig, Phase, TargetConfig};
    use std::collections::HashMap;

    fn scenario(injector: &str) -> Scenario {
        Scenario {
            name: "policy-test".into(),
            description: None,
            seed: Some(7),
            duration: Duration::from_secs(1),
            ramp_up: None,
            phases: vec![Phase {
                name: "fault".into(),
                duration: Duration::from_secs(1),
                injections: vec![InjectionConfig {
                    r#type: injector.into(),
                    target: TargetConfig::default(),
                    parameters: HashMap::new(),
                }],
                parallel: false,
            }],
            labels: HashMap::new(),
            assertions: Vec::new(),
        }
    }

    #[test]
    fn policy_rejects_denied_injectors_and_excess_blast_radius() {
        let policy = ExperimentPolicy {
            allowed_injectors: vec!["dependency_*".into()],
            ..ExperimentPolicy::default()
        };
        assert!(policy
            .validate_at(&scenario("dependency_proxy"), Utc::now())
            .is_ok());
        assert!(policy
            .validate_at(&scenario("process_kill"), Utc::now())
            .is_err());
        assert!(policy.effective_parallel_limit(2, 10, 50).is_err());
        assert_eq!(policy.effective_parallel_limit(2, 10, 20).unwrap(), 2);
    }
}
