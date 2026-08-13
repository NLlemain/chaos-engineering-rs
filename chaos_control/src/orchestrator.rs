use crate::client::AgentClient;
use crate::history::ExperimentHistory;
use crate::manifest::{AgentAssignment, DistributedExperiment, RemoteAgent};
use crate::policy::ExperimentPolicy;
use crate::protocol::{ControlCommand, ControlResponse};
use crate::tls::ClientTlsConfig;
use chaos_scenarios::runner::ScenarioResult;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedResult {
    pub experiment_id: String,
    pub experiment_name: String,
    pub seed: u64,
    pub parallel_limit: usize,
    pub phases: Vec<PhaseExecutionResult>,
    pub succeeded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseExecutionResult {
    pub name: String,
    pub targets: Vec<TargetResult>,
    pub succeeded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetResult {
    pub execution_id: String,
    pub target_id: String,
    pub agent_id: String,
    pub seed: u64,
    pub succeeded: bool,
    pub result: ScenarioResult,
}

pub struct Orchestrator {
    client: AgentClient,
    policy: ExperimentPolicy,
    history: Arc<ExperimentHistory>,
}

impl Orchestrator {
    pub fn new(
        tls: ClientTlsConfig,
        policy: ExperimentPolicy,
        history: Arc<ExperimentHistory>,
    ) -> Self {
        Self {
            client: AgentClient::new(tls),
            policy,
            history,
        }
    }

    pub async fn run(
        &self,
        experiment: &DistributedExperiment,
    ) -> anyhow::Result<DistributedResult> {
        experiment.validate()?;
        self.policy.validate()?;
        let seed = experiment.resolved_seed()?;
        let target_count = experiment.target_count();
        let parallel_limit = self.policy.effective_parallel_limit(
            experiment.max_parallel_targets,
            target_count,
            experiment.max_blast_radius_percent,
        )?;
        for phase in &experiment.phases {
            for assignment in &phase.assignments {
                self.policy
                    .validate_at(&assignment.scenario, chrono::Utc::now())?;
            }
        }

        let experiment_id = uuid::Uuid::new_v4().to_string();
        self.history.begin(
            &experiment_id,
            &experiment.name,
            seed,
            target_count,
            experiment,
            &self.policy,
        )?;

        match self
            .run_prepared(experiment, &experiment_id, seed, parallel_limit)
            .await
        {
            Ok(result) => {
                let status = if result.succeeded {
                    "succeeded"
                } else {
                    "failed"
                };
                self.history.attach_artifact(
                    &experiment_id,
                    "distributed-result.json",
                    "application/json",
                    &serde_json::to_vec_pretty(&result)?,
                )?;
                self.history.finish(&experiment_id, status)?;
                Ok(result)
            }
            Err(error) => {
                self.history.attach_artifact(
                    &experiment_id,
                    "error.json",
                    "application/json",
                    &serde_json::to_vec_pretty(&serde_json::json!({
                        "error": error.to_string(),
                        "seed": seed,
                    }))?,
                )?;
                self.history.finish(&experiment_id, "failed")?;
                Err(error)
            }
        }
    }

    async fn run_prepared(
        &self,
        experiment: &DistributedExperiment,
        experiment_id: &str,
        seed: u64,
        parallel_limit: usize,
    ) -> anyhow::Result<DistributedResult> {
        self.ping_agents(experiment).await?;
        let mut phases = Vec::new();

        for (phase_index, phase) in experiment.phases.iter().enumerate() {
            let prepared: Vec<_> = phase
                .assignments
                .iter()
                .enumerate()
                .map(|(target_index, assignment)| PreparedTarget {
                    execution_id: format!("{experiment_id}-{phase_index}-{target_index}"),
                    seed: derive_seed(seed, phase_index, target_index, &assignment.target_id),
                    assignment,
                    agent: experiment
                        .agent(&assignment.agent_id)
                        .expect("manifest validation resolved agent"),
                })
                .collect();

            let preparation = join_all(prepared.iter().map(|target| async move {
                self.send(
                    target.agent,
                    ControlCommand::Prepare {
                        execution_id: target.execution_id.clone(),
                        seed: target.seed,
                        scenario: target.assignment.scenario.clone(),
                    },
                )
                .await
            }))
            .await;
            if let Some(error) = first_failure(preparation) {
                self.recover_all(&prepared).await;
                return Err(error.context(format!("phase '{}' preparation failed", phase.name)));
            }

            let mut targets = Vec::new();
            let mut phase_failed = false;
            for batch in prepared.chunks(parallel_limit) {
                let responses = join_all(batch.iter().map(|target| async move {
                    (
                        target,
                        self.send(
                            target.agent,
                            ControlCommand::Execute {
                                execution_id: target.execution_id.clone(),
                            },
                        )
                        .await,
                    )
                }))
                .await;
                for (target, response) in responses {
                    let response = match response {
                        Ok(response) => response,
                        Err(error) => {
                            self.recover_all(&prepared).await;
                            return Err(error.context(format!(
                                "target '{}' on agent '{}' failed",
                                target.assignment.target_id, target.assignment.agent_id
                            )));
                        }
                    };
                    let result = response.result.ok_or_else(|| {
                        anyhow::anyhow!(
                            "agent '{}' completed '{}' without a scenario result",
                            response.agent_id,
                            target.execution_id
                        )
                    })?;
                    let succeeded = target_succeeded(&result);
                    phase_failed |= !succeeded;
                    let target_result = TargetResult {
                        execution_id: target.execution_id.clone(),
                        target_id: target.assignment.target_id.clone(),
                        agent_id: target.assignment.agent_id.clone(),
                        seed: target.seed,
                        succeeded,
                        result,
                    };
                    let artifact_name =
                        format!("phase-{phase_index:03}-target-{:03}.json", targets.len());
                    self.history.attach_artifact(
                        experiment_id,
                        &artifact_name,
                        "application/json",
                        &serde_json::to_vec_pretty(&target_result)?,
                    )?;
                    targets.push(target_result);
                }
                if phase_failed {
                    break;
                }
            }
            if phase_failed {
                self.recover_all(&prepared).await;
            }
            phases.push(PhaseExecutionResult {
                name: phase.name.clone(),
                succeeded: !phase_failed,
                targets,
            });
            if phase_failed {
                break;
            }
        }

        let succeeded =
            phases.len() == experiment.phases.len() && phases.iter().all(|phase| phase.succeeded);
        Ok(DistributedResult {
            experiment_id: experiment_id.to_string(),
            experiment_name: experiment.name.clone(),
            seed,
            parallel_limit,
            phases,
            succeeded,
        })
    }

    async fn ping_agents(&self, experiment: &DistributedExperiment) -> anyhow::Result<()> {
        let responses = join_all(
            experiment
                .agents
                .iter()
                .map(|agent| self.send(agent, ControlCommand::Ping)),
        )
        .await;
        if let Some(error) = first_failure(responses) {
            return Err(error.context("one or more remote agents are unavailable"));
        }
        Ok(())
    }

    async fn recover_all(&self, targets: &[PreparedTarget<'_>]) {
        let responses = join_all(targets.iter().map(|target| {
            self.send(
                target.agent,
                ControlCommand::Recover {
                    execution_id: target.execution_id.clone(),
                },
            )
        }))
        .await;
        for response in responses {
            if let Err(error) = response {
                tracing::warn!(%error, "remote recovery failed");
            }
        }
    }

    async fn send(
        &self,
        agent: &RemoteAgent,
        command: ControlCommand,
    ) -> anyhow::Result<ControlResponse> {
        self.client.request(agent, command).await
    }
}

struct PreparedTarget<'a> {
    execution_id: String,
    seed: u64,
    assignment: &'a AgentAssignment,
    agent: &'a RemoteAgent,
}

fn first_failure(responses: Vec<anyhow::Result<ControlResponse>>) -> Option<anyhow::Error> {
    responses.into_iter().find_map(Result::err)
}

fn derive_seed(base: u64, phase: usize, target: usize, target_id: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(base.to_be_bytes());
    hasher.update((phase as u64).to_be_bytes());
    hasher.update((target as u64).to_be_bytes());
    hasher.update(target_id.as_bytes());
    let digest = hasher.finalize();
    u64::from_be_bytes(digest[..8].try_into().expect("eight-byte digest slice"))
}

fn target_succeeded(result: &ScenarioResult) -> bool {
    !result.cancelled
        && result.attempted_injections == result.total_injections
        && result.cleanup_failures == 0
        && result.slos_passed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_target_seeds_are_stable_and_distinct() {
        assert_eq!(
            derive_seed(42, 1, 2, "feed-a"),
            derive_seed(42, 1, 2, "feed-a")
        );
        assert_ne!(
            derive_seed(42, 1, 2, "feed-a"),
            derive_seed(42, 1, 3, "feed-b")
        );
    }
}
