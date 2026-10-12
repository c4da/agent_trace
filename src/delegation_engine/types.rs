//! Data types and configuration contracts for AI agent delegation.

use crate::router::RoutingIntent;
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Source of a routing decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionSource {
    /// Explicit rule mandated dynamically by an AI agent or operator.
    OverrideRule,
    /// Arbitrated by an AI agent / local model reasoning about task complexity.
    AgentArbitrator,
    /// Matched via dynamic capability registry heuristics.
    DynamicCapability,
    /// Explicit direct model requested by client.
    DirectBypass,
    /// Intercepted external cloud model (gemini*, gpt*, claude*) diverted to local cluster.
    CloudInterception,
    /// Requested model is offline; dynamically switched to active downloaded model.
    FallbackSwitch,
}

pub(crate) fn default_now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

pub(crate) fn default_rule_id() -> String {
    format!("rule-{}", &uuid::Uuid::new_v4().simple().to_string()[..8])
}

/// Criteria used to match a dynamic route override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum OverrideCriteria {
    /// Matches a specific session ID.
    Session(String),
    /// Matches a specific agent ID (e.g. "Hermes-Coder").
    Agent(String),
    /// Matches an arbitrary metadata tag.
    Tag(String),
    /// Matches requested model identifier (e.g. "gemini*", "gpt*").
    Model(String),
    /// Matches keywords or substrings in the prompt (e.g. "*test*", "*rust*").
    PromptPattern(String),
}

/// Dynamic routing override rule set by an AI agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteOverrideRule {
    #[serde(default = "default_rule_id")]
    pub rule_id: String,
    pub criteria: OverrideCriteria,
    pub target_model: String,
    pub target_node: Option<String>,
    pub reason: String,
    #[serde(default = "default_now_rfc3339")]
    pub created_at: String,
}

/// Request to arbitrate and delegate a prompt or task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDelegationRequest {
    pub prompt: String,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub requested_model: Option<String>,
}

/// Final decision returned by the delegation engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDelegationDecision {
    pub selected_model: String,
    pub target_upstream: String,
    pub node_id: String,
    pub decision_source: DecisionSource,
    pub intent: RoutingIntent,
    pub reasoning: String,
    pub execution_ms: f64,
}

/// Configuration for the Agent Delegation and Token Guard Engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationConfig {
    /// Intercept external cloud models and divert them to the local cluster.
    pub intercept_external_models: bool,
    /// Enforce zero external API token economy across all requests.
    pub zero_token_mode: bool,
    /// If requested model is offline, switch to active downloaded cluster model.
    pub auto_switch_offline_models: bool,
    /// Default fallback model ID when diverted or unmapped.
    pub default_fallback_model: String,
}

impl Default for DelegationConfig {
    fn default() -> Self {
        Self {
            intercept_external_models: true,
            zero_token_mode: true,
            auto_switch_offline_models: true,
            default_fallback_model: "auto".to_string(),
        }
    }
}
