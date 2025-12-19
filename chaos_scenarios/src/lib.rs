pub mod config;
pub mod parser;
pub mod phase;
pub mod runner;
pub mod scheduler;

pub use config::{Scenario, ScenarioConfig};
pub use parser::{parse_scenario_from_file, parse_scenario_from_str};
pub use phase::Phase;
pub use runner::{run_scenario, ScenarioRunner};
pub use scheduler::{Scheduler, SchedulingMode};

/// Convenience function to parse a scenario from a YAML string
pub fn parse_scenario(yaml: &str) -> anyhow::Result<Scenario> {
    parser::parse_scenario_from_str(yaml, "yaml")
}
