//! Types and shared state for the agent interface.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
    db::Database,
    delegation_engine::{AgentDelegationEngine, RouteOverrideStore},
    ledger::NodeIdentity,
    policy::PolicyEngine,
    registry::ClusterModelRegistry,
    secrets::ClusterSecurityConfig,
};

/// Shared state required across agent interface routes.
#[derive(Clone)]
pub struct AgentInterfaceState {
    pub db: Database,
    pub registry: Arc<RwLock<ClusterModelRegistry>>,
    pub security: Arc<ClusterSecurityConfig>,
    pub identity: Arc<NodeIdentity>,
    pub upstream_url: String,
    pub override_store: Arc<RwLock<RouteOverrideStore>>,
    pub delegation_engine: Arc<AgentDelegationEngine>,
    pub policy: Arc<RwLock<PolicyEngine>>,
}

/// Request payload for an agent executing an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentActionRequest {
    /// Identity of the calling agent (e.g. "Hermes-Watchdog", "Claude-Code").
    pub agent_id: String,
    /// Name of the tool to execute.
    pub tool: String,
    /// Tool arguments.
    #[serde(default)]
    pub parameters: Value,
}

/// Standardized result returned to the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentActionResponse {
    pub success: bool,
    pub tool: String,
    pub agent_id: String,
    pub result: Value,
    pub execution_ms: f64,
    pub error: Option<String>,
}
