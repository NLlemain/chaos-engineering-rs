use chaos_scenarios::{runner::ScenarioResult, Scenario};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_CONTROL_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRequest {
    pub protocol_version: u16,
    pub request_id: String,
    pub command: ControlCommand,
}

impl ControlRequest {
    pub fn new(command: ControlCommand) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: uuid::Uuid::new_v4().to_string(),
            command,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum ControlCommand {
    Ping,
    Prepare {
        execution_id: String,
        seed: u64,
        scenario: Scenario,
    },
    Execute {
        execution_id: String,
    },
    Recover {
        execution_id: String,
    },
    StopAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Ready,
    Prepared,
    Running,
    Completed,
    Recovered,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlResponse {
    pub protocol_version: u16,
    pub request_id: String,
    pub agent_id: String,
    pub accepted: bool,
    pub state: AgentState,
    pub message: String,
    #[serde(default)]
    pub result: Option<ScenarioResult>,
}

impl ControlResponse {
    pub fn accepted(
        request: &ControlRequest,
        agent_id: &str,
        state: AgentState,
        message: impl Into<String>,
        result: Option<ScenarioResult>,
    ) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: request.request_id.clone(),
            agent_id: agent_id.to_string(),
            accepted: true,
            state,
            message: message.into(),
            result,
        }
    }

    pub fn rejected(request: &ControlRequest, agent_id: &str, message: impl Into<String>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: request.request_id.clone(),
            agent_id: agent_id.to_string(),
            accepted: false,
            state: AgentState::Rejected,
            message: message.into(),
            result: None,
        }
    }
}
