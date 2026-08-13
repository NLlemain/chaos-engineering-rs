use chaos_scenarios::Scenario;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::net::SocketAddr;

pub const DISTRIBUTED_API_VERSION: &str = "chaos.engineering/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedExperiment {
    pub api_version: String,
    pub name: String,
    #[serde(default)]
    pub seed: Option<u64>,
    pub max_parallel_targets: usize,
    pub max_blast_radius_percent: u8,
    pub agents: Vec<RemoteAgent>,
    pub phases: Vec<DistributedPhase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteAgent {
    pub id: String,
    pub address: SocketAddr,
    pub server_name: String,
    #[serde(default)]
    pub labels: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DistributedPhase {
    pub name: String,
    pub assignments: Vec<AgentAssignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentAssignment {
    pub target_id: String,
    pub agent_id: String,
    pub scenario: Scenario,
}

impl DistributedExperiment {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.api_version == DISTRIBUTED_API_VERSION,
            "unsupported distributed API version '{}'; expected '{}'",
            self.api_version,
            DISTRIBUTED_API_VERSION
        );
        anyhow::ensure!(
            !self.name.trim().is_empty(),
            "experiment name cannot be empty"
        );
        anyhow::ensure!(
            self.max_parallel_targets > 0,
            "max_parallel_targets must be greater than zero"
        );
        anyhow::ensure!(
            (1..=100).contains(&self.max_blast_radius_percent),
            "max_blast_radius_percent must be between 1 and 100"
        );
        anyhow::ensure!(
            !self.agents.is_empty(),
            "experiment requires at least one agent"
        );
        anyhow::ensure!(
            !self.phases.is_empty(),
            "experiment requires at least one phase"
        );

        let mut agent_ids = HashSet::new();
        for agent in &self.agents {
            anyhow::ensure!(!agent.id.trim().is_empty(), "agent ID cannot be empty");
            anyhow::ensure!(
                agent_ids.insert(agent.id.as_str()),
                "duplicate agent ID '{}'",
                agent.id
            );
            anyhow::ensure!(
                !agent.server_name.trim().is_empty(),
                "agent '{}' server name cannot be empty",
                agent.id
            );
        }

        let mut phase_names = HashSet::new();
        let mut target_ids = HashSet::new();
        for phase in &self.phases {
            anyhow::ensure!(!phase.name.trim().is_empty(), "phase name cannot be empty");
            anyhow::ensure!(
                phase_names.insert(phase.name.as_str()),
                "duplicate distributed phase '{}'",
                phase.name
            );
            anyhow::ensure!(
                !phase.assignments.is_empty(),
                "phase '{}' requires at least one assignment",
                phase.name
            );
            let mut phase_targets = HashSet::new();
            for assignment in &phase.assignments {
                anyhow::ensure!(
                    !assignment.target_id.trim().is_empty(),
                    "phase '{}' has an empty target ID",
                    phase.name
                );
                anyhow::ensure!(
                    phase_targets.insert(assignment.target_id.as_str()),
                    "phase '{}' targets '{}' more than once",
                    phase.name,
                    assignment.target_id
                );
                anyhow::ensure!(
                    agent_ids.contains(assignment.agent_id.as_str()),
                    "phase '{}' references unknown agent '{}'",
                    phase.name,
                    assignment.agent_id
                );
                assignment.scenario.validate().map_err(anyhow::Error::msg)?;
                target_ids.insert(assignment.target_id.as_str());
            }
        }
        anyhow::ensure!(
            !target_ids.is_empty(),
            "experiment requires at least one target"
        );
        Ok(())
    }

    pub fn resolved_seed(&self) -> anyhow::Result<u64> {
        if let Some(seed) = self.seed {
            return Ok(seed);
        }
        let mut canonical = self.clone();
        canonical.seed = None;
        let digest = Sha256::digest(serde_json::to_vec(&canonical)?);
        Ok(u64::from_be_bytes(
            digest[..8].try_into().expect("eight-byte digest slice"),
        ))
    }

    pub fn target_count(&self) -> usize {
        self.phases
            .iter()
            .flat_map(|phase| phase.assignments.iter())
            .map(|assignment| assignment.target_id.as_str())
            .collect::<HashSet<_>>()
            .len()
    }

    pub fn agent(&self, id: &str) -> Option<&RemoteAgent> {
        self.agents.iter().find(|agent| agent.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chaos_scenarios::config::Phase;
    use std::{collections::HashMap, time::Duration};

    fn experiment() -> DistributedExperiment {
        DistributedExperiment {
            api_version: DISTRIBUTED_API_VERSION.into(),
            name: "deterministic".into(),
            seed: None,
            max_parallel_targets: 1,
            max_blast_radius_percent: 50,
            agents: vec![RemoteAgent {
                id: "ams-1".into(),
                address: "127.0.0.1:9443".parse().unwrap(),
                server_name: "ams-1.chaos.test".into(),
                labels: Default::default(),
            }],
            phases: vec![DistributedPhase {
                name: "market-data".into(),
                assignments: vec![AgentAssignment {
                    target_id: "feed-a".into(),
                    agent_id: "ams-1".into(),
                    scenario: Scenario {
                        name: "feed-gap".into(),
                        description: None,
                        seed: None,
                        duration: Duration::from_secs(1),
                        ramp_up: None,
                        phases: vec![Phase {
                            name: "observe".into(),
                            duration: Duration::from_secs(1),
                            injections: Vec::new(),
                            parallel: false,
                        }],
                        labels: HashMap::new(),
                        assertions: Vec::new(),
                    },
                }],
            }],
        }
    }

    #[test]
    fn seed_is_stable_when_not_explicit() {
        let value = experiment();
        value.validate().unwrap();
        assert_eq!(
            value.resolved_seed().unwrap(),
            value.resolved_seed().unwrap()
        );
        assert_eq!(value.target_count(), 1);
    }
}
