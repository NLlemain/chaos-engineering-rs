//! Distributed experiment coordination with policy, history, and mutual TLS.

mod agent;
mod client;
mod history;
mod manifest;
mod orchestrator;
mod policy;
mod protocol;
mod tls;

pub use agent::{AgentServer, AgentServerConfig};
pub use client::AgentClient;
pub use history::{ArtifactRecord, ExperimentHistory, ExperimentRecord, RetentionPolicy};
pub use manifest::{AgentAssignment, DistributedExperiment, DistributedPhase, RemoteAgent};
pub use orchestrator::{DistributedResult, Orchestrator, PhaseExecutionResult, TargetResult};
pub use policy::{ExperimentPolicy, SchedulePolicy, SloPolicy};
pub use protocol::{AgentState, ControlCommand, ControlRequest, ControlResponse, PROTOCOL_VERSION};
pub use tls::{ClientTlsConfig, ServerTlsConfig};
