use crate::{
    config::{InjectionConfig, Scenario},
    injector_factory::build_injector,
    scheduler::{Scheduler, SchedulingMode},
};
use chaos_core::{Executor, InjectionHandle};
use std::time::Duration;
use tokio::time::Instant;
use tracing::{info, warn};

pub struct ScenarioRunner {
    executor: Executor,
}

impl ScenarioRunner {
    pub fn new(executor: Executor) -> Self {
        Self { executor }
    }

    pub fn with_defaults() -> Self {
        Self::new(Executor::with_defaults())
    }

    pub async fn run(&self, scenario: &Scenario) -> anyhow::Result<ScenarioResult> {
        info!("Starting scenario: {}", scenario.name);
        scenario.validate().map_err(|e| anyhow::anyhow!(e))?;

        let started_at = chrono::Utc::now();
        let start_time = Instant::now();

        // Phases are always sequential. A phase's `parallel` flag applies to the
        // injections inside that phase, which remain active together.
        let mut scheduler = Scheduler::new(SchedulingMode::Sequential, scenario.seed);

        let mut phases = scheduler.schedule_phases(scenario);

        if let Some(ramp_up) = scenario.ramp_up {
            scheduler.apply_ramp_up(&mut phases, ramp_up);
        }

        let mut phase_results = Vec::new();
        let mut total_attempted = 0;
        let mut total_succeeded = 0;
        let mut total_cleanup_failures = 0;

        // Execute phases
        for scheduled_phase in phases {
            // Wait until phase start time
            let elapsed = start_time.elapsed();
            if let Some(delay) = scheduled_phase.delay_until_start(elapsed) {
                info!(
                    "Waiting {:?} before starting phase '{}'",
                    delay,
                    scheduled_phase.name()
                );
                tokio::time::sleep(delay).await;
            }

            info!(
                "Starting phase '{}' (duration: {:?})",
                scheduled_phase.name(),
                scheduled_phase.duration()
            );

            let phase_start = Instant::now();
            let mut handles = Vec::new();
            let mut injection_failures = Vec::new();
            let mut cleanup_failures = Vec::new();

            // Apply injections
            for injection in &scheduled_phase.phase.injections {
                total_attempted += 1;
                match self.apply_injection(injection).await {
                    Ok(handle) => {
                        info!("Applied injection: {}", injection.r#type);
                        handles.push(handle);
                        total_succeeded += 1;
                    }
                    Err(e) => {
                        warn!("Failed to apply injection '{}': {}", injection.r#type, e);
                        injection_failures.push(InjectionFailure {
                            injection_type: injection.r#type.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            }

            // Wait for phase duration
            let phase_elapsed = phase_start.elapsed();
            if phase_elapsed < scheduled_phase.duration() {
                let remaining = scheduled_phase.duration() - phase_elapsed;
                tokio::time::sleep(remaining).await;
            }

            // Remove injections
            for handle in &handles {
                if let Err(e) = self.executor.remove(handle.clone()).await {
                    warn!("Failed to remove injection '{}': {}", handle.id, e);
                    total_cleanup_failures += 1;
                    cleanup_failures.push(InjectionFailure {
                        injection_type: handle.injector_name.clone(),
                        error: e.to_string(),
                    });
                }
            }

            let phase_duration = phase_start.elapsed();
            info!(
                "Completed phase '{}' in {:?}",
                scheduled_phase.name(),
                phase_duration
            );

            phase_results.push(PhaseResult {
                name: scheduled_phase.name().to_string(),
                duration: phase_duration,
                injection_count: handles.len(),
                attempted_injections: scheduled_phase.phase.injections.len(),
                injection_failures,
                cleanup_failures,
            });
        }

        let total_duration = start_time.elapsed();

        info!(
            "Scenario '{}' completed in {:?}",
            scenario.name, total_duration
        );

        Ok(ScenarioResult {
            scenario_name: scenario.name.clone(),
            started_at,
            total_duration,
            phase_results,
            total_injections: total_succeeded,
            attempted_injections: total_attempted,
            cleanup_failures: total_cleanup_failures,
        })
    }

    async fn apply_injection(
        &self,
        injection: &InjectionConfig,
    ) -> anyhow::Result<InjectionHandle> {
        let target = injection
            .target
            .to_target()
            .map_err(|e| anyhow::anyhow!("Invalid target: {}", e))?;

        let handle = match build_injector(injection)? {
            Some(injector) => self.executor.inject_with(injector, &target).await,
            None => self.executor.inject(&injection.r#type, &target).await,
        }
        .map_err(|e| anyhow::anyhow!("Injection failed: {}", e))?;

        Ok(handle)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScenarioResult {
    pub scenario_name: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    #[serde(with = "humantime_serde")]
    pub total_duration: Duration,
    pub phase_results: Vec<PhaseResult>,
    pub total_injections: usize,
    #[serde(default)]
    pub attempted_injections: usize,
    #[serde(default)]
    pub cleanup_failures: usize,
}

impl ScenarioResult {
    pub fn success_rate(&self) -> f64 {
        let attempted = if self.attempted_injections == 0 {
            self.total_injections
        } else {
            self.attempted_injections
        };

        if attempted == 0 {
            return 1.0;
        }

        self.total_injections as f64 / attempted as f64
    }

    pub fn average_phase_duration(&self) -> Duration {
        if self.phase_results.is_empty() {
            return Duration::ZERO;
        }

        let total: Duration = self.phase_results.iter().map(|p| p.duration).sum();
        total / self.phase_results.len() as u32
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PhaseResult {
    pub name: String,
    #[serde(with = "humantime_serde")]
    pub duration: Duration,
    pub injection_count: usize,
    #[serde(default)]
    pub attempted_injections: usize,
    #[serde(default)]
    pub injection_failures: Vec<InjectionFailure>,
    #[serde(default)]
    pub cleanup_failures: Vec<InjectionFailure>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct InjectionFailure {
    pub injection_type: String,
    pub error: String,
}

pub async fn run_scenario(scenario: &Scenario) -> anyhow::Result<ScenarioResult> {
    let runner = ScenarioRunner::with_defaults();
    runner.run(scenario).await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scenario_runner_creation() {
        let _runner = ScenarioRunner::with_defaults();
        assert!(true); // Runner created successfully
    }

    #[test]
    fn test_scenario_result() {
        let result = ScenarioResult {
            scenario_name: "test".to_string(),
            started_at: chrono::Utc::now(),
            total_duration: Duration::from_secs(100),
            phase_results: vec![
                PhaseResult {
                    name: "phase1".to_string(),
                    duration: Duration::from_secs(50),
                    injection_count: 2,
                    attempted_injections: 2,
                    injection_failures: vec![],
                    cleanup_failures: vec![],
                },
                PhaseResult {
                    name: "phase2".to_string(),
                    duration: Duration::from_secs(50),
                    injection_count: 1,
                    attempted_injections: 2,
                    injection_failures: vec![InjectionFailure {
                        injection_type: "unknown".to_string(),
                        error: "not registered".to_string(),
                    }],
                    cleanup_failures: vec![],
                },
            ],
            total_injections: 3,
            attempted_injections: 4,
            cleanup_failures: 0,
        };

        assert_eq!(result.success_rate(), 0.75);
        assert_eq!(result.average_phase_duration(), Duration::from_secs(50));
    }
}
