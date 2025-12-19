use crate::config::{Phase, Scenario};
use rand::{rngs::StdRng, seq::SliceRandom, SeedableRng};
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingMode {
    Sequential,
    Randomized,
    Parallel,
}

pub struct Scheduler {
    mode: SchedulingMode,
    rng: Option<StdRng>,
}

impl Scheduler {
    pub fn new(mode: SchedulingMode, seed: Option<u64>) -> Self {
        let rng = seed.map(StdRng::seed_from_u64);
        Self { mode, rng }
    }

    pub fn sequential() -> Self {
        Self::new(SchedulingMode::Sequential, None)
    }

    pub fn randomized(seed: u64) -> Self {
        Self::new(SchedulingMode::Randomized, Some(seed))
    }

    pub fn parallel() -> Self {
        Self::new(SchedulingMode::Parallel, None)
    }

    pub fn schedule_phases(&mut self, scenario: &Scenario) -> Vec<ScheduledPhase> {
        let mut phases: Vec<ScheduledPhase> = scenario
            .phases
            .iter()
            .enumerate()
            .map(|(index, phase)| {
                let start_time = if index == 0 {
                    Duration::ZERO
                } else {
                    scenario.phases[..index].iter().map(|p| p.duration).sum()
                };

                ScheduledPhase {
                    phase: phase.clone(),
                    index,
                    start_time,
                    end_time: start_time + phase.duration,
                }
            })
            .collect();

        match self.mode {
            SchedulingMode::Sequential => {
                // Phases are already in order
            }
            SchedulingMode::Randomized => {
                if let Some(rng) = &mut self.rng {
                    phases.shuffle(rng);
                    // Recalculate start/end times after shuffling
                    let mut current_time = Duration::ZERO;
                    for scheduled in &mut phases {
                        scheduled.start_time = current_time;
                        scheduled.end_time = current_time + scheduled.phase.duration;
                        current_time = scheduled.end_time;
                    }
                }
            }
            SchedulingMode::Parallel => {
                // All phases start at the same time
                for scheduled in &mut phases {
                    scheduled.start_time = Duration::ZERO;
                    scheduled.end_time = scheduled.phase.duration;
                }
            }
        }

        info!("Scheduled {} phases in {:?} mode", phases.len(), self.mode);

        phases
    }

    pub fn apply_ramp_up(&self, phases: &mut [ScheduledPhase], ramp_up: Duration) {
        if ramp_up.is_zero() || phases.is_empty() {
            return;
        }

        info!("Applying ramp-up period: {:?}", ramp_up);

        // Delay all phases by the ramp-up duration
        for phase in phases.iter_mut() {
            phase.start_time += ramp_up;
            phase.end_time += ramp_up;
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScheduledPhase {
    pub phase: Phase,
    pub index: usize,
    pub start_time: Duration,
    pub end_time: Duration,
}

impl ScheduledPhase {
    pub fn duration(&self) -> Duration {
        self.phase.duration
    }

    pub fn name(&self) -> &str {
        &self.phase.name
    }

    pub fn delay_until_start(&self, current_time: Duration) -> Option<Duration> {
        if current_time < self.start_time {
            Some(self.start_time - current_time)
        } else {
            None
        }
    }

    pub fn is_active(&self, current_time: Duration) -> bool {
        current_time >= self.start_time && current_time < self.end_time
    }

    pub fn has_started(&self, current_time: Duration) -> bool {
        current_time >= self.start_time
    }

    pub fn has_ended(&self, current_time: Duration) -> bool {
        current_time >= self.end_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Phase, Scenario};

    #[test]
    fn test_sequential_scheduling() {
        let scenario = Scenario::builder()
            .name("test")
            .add_phase(
                Phase::builder()
                    .name("p1")
                    .duration(Duration::from_secs(10))
                    .build(),
            )
            .add_phase(
                Phase::builder()
                    .name("p2")
                    .duration(Duration::from_secs(20))
                    .build(),
            )
            .build();

        let mut scheduler = Scheduler::sequential();
        let phases = scheduler.schedule_phases(&scenario);

        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].start_time, Duration::ZERO);
        assert_eq!(phases[0].end_time, Duration::from_secs(10));
        assert_eq!(phases[1].start_time, Duration::from_secs(10));
        assert_eq!(phases[1].end_time, Duration::from_secs(30));
    }

    #[test]
    fn test_parallel_scheduling() {
        let scenario = Scenario::builder()
            .name("test")
            .add_phase(
                Phase::builder()
                    .name("p1")
                    .duration(Duration::from_secs(10))
                    .build(),
            )
            .add_phase(
                Phase::builder()
                    .name("p2")
                    .duration(Duration::from_secs(20))
                    .build(),
            )
            .build();

        let mut scheduler = Scheduler::parallel();
        let phases = scheduler.schedule_phases(&scenario);

        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].start_time, Duration::ZERO);
        assert_eq!(phases[1].start_time, Duration::ZERO);
    }

    #[test]
    fn test_ramp_up() {
        let scenario = Scenario::builder()
            .name("test")
            .add_phase(
                Phase::builder()
                    .name("p1")
                    .duration(Duration::from_secs(10))
                    .build(),
            )
            .build();

        let mut scheduler = Scheduler::sequential();
        let mut phases = scheduler.schedule_phases(&scenario);
        scheduler.apply_ramp_up(&mut phases, Duration::from_secs(5));

        assert_eq!(phases[0].start_time, Duration::from_secs(5));
        assert_eq!(phases[0].end_time, Duration::from_secs(15));
    }

    #[test]
    fn test_scheduled_phase_status() {
        let phase = ScheduledPhase {
            phase: Phase::builder()
                .name("test")
                .duration(Duration::from_secs(10))
                .build(),
            index: 0,
            start_time: Duration::from_secs(5),
            end_time: Duration::from_secs(15),
        };

        assert!(!phase.has_started(Duration::from_secs(3)));
        assert!(phase.has_started(Duration::from_secs(5)));
        assert!(phase.is_active(Duration::from_secs(10)));
        assert!(!phase.is_active(Duration::from_secs(20)));
        assert!(phase.has_ended(Duration::from_secs(20)));
    }
}
