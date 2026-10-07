//! Autonomous AI Agent Delegation & Arbitration Engine for AgentTrace Gateway.
//!
//! Enables AI agents to dynamically decide routing, override model selections, and
//! arbitrate prompt distribution across distributed cluster nodes at runtime.

use crate::{
    registry::{ClusterModelRegistry, ModelManifest},
    router::{classify_intent, RoutingIntent},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

/// Source of the routing decision.
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
}

fn default_now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn default_rule_id() -> String {
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

/// In-memory store for active route overrides set by AI agents.
#[derive(Debug, Clone, Default)]
pub struct RouteOverrideStore {
    rules: HashMap<String, RouteOverrideRule>,
}

impl RouteOverrideStore {
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
        }
    }

    /// Sets or replaces an override rule. Returns the assigned rule ID.
    pub fn set_rule(&mut self, mut rule: RouteOverrideRule) -> String {
        if rule.rule_id.is_empty() {
            rule.rule_id = default_rule_id();
        }
        let id = rule.rule_id.clone();
        self.rules.insert(id.clone(), rule);
        id
    }

    /// Removes an override rule by ID.
    pub fn remove_rule(&mut self, rule_id: &str) -> bool {
        self.rules.remove(rule_id).is_some()
    }

    /// Lists all currently active override rules.
    pub fn list_rules(&self) -> Vec<RouteOverrideRule> {
        self.rules.values().cloned().collect()
    }

    /// Evaluates whether an override rule matches the incoming context.
    pub fn find_matching_override(
        &self,
        session_id: Option<&str>,
        agent_id: Option<&str>,
        tags: &[String],
    ) -> Option<RouteOverrideRule> {
        for rule in self.rules.values() {
            match &rule.criteria {
                OverrideCriteria::Session(s) => {
                    if let Some(req_s) = session_id {
                        if req_s == s {
                            return Some(rule.clone());
                        }
                    }
                }
                OverrideCriteria::Agent(a) => {
                    if let Some(req_a) = agent_id {
                        if req_a.eq_ignore_ascii_case(a) {
                            return Some(rule.clone());
                        }
                    }
                }
                OverrideCriteria::Tag(t) => {
                    if tags.iter().any(|req_t| req_t.eq_ignore_ascii_case(t)) {
                        return Some(rule.clone());
                    }
                }
            }
        }
        None
    }
}

/// The core AI agent delegation arbitrator.
#[derive(Clone)]
pub struct AgentDelegationEngine {
    override_store: Arc<RwLock<RouteOverrideStore>>,
    registry: Arc<RwLock<ClusterModelRegistry>>,
    default_upstream: String,
}

impl AgentDelegationEngine {
    pub fn new(
        override_store: Arc<RwLock<RouteOverrideStore>>,
        registry: Arc<RwLock<ClusterModelRegistry>>,
        default_upstream: String,
    ) -> Self {
        Self {
            override_store,
            registry,
            default_upstream,
        }
    }

    /// Arbitrates routing for a request: checks active agent overrides first,
    /// then matches dynamic capabilities across the cluster.
    pub async fn decide(&self, req: &AgentDelegationRequest) -> AgentDelegationDecision {
        let t0 = std::time::Instant::now();

        // 1. Check for Active Agent Overrides
        {
            let store = self.override_store.read().await;
            if let Some(rule) = store.find_matching_override(
                req.session_id.as_deref(),
                req.agent_id.as_deref(),
                &req.tags,
            ) {
                let reg = self.registry.read().await;
                let (target_upstream, node_id) = if let Some(m) = reg.find_model(&rule.target_model) {
                    (m.endpoint, m.node_id)
                } else {
                    let nid = rule.target_node.unwrap_or_else(|| "OVERRIDE".to_string());
                    (self.default_upstream.clone(), nid)
                };

                let execution_ms = t0.elapsed().as_secs_f64() * 1000.0;
                return AgentDelegationDecision {
                    selected_model: rule.target_model,
                    target_upstream,
                    node_id,
                    decision_source: DecisionSource::OverrideRule,
                    intent: RoutingIntent::GeneralExecution,
                    reasoning: format!("Agent override applied (rule {}): {}", rule.rule_id, rule.reason),
                    execution_ms,
                };
            }
        }

        // 2. Explicit Model Direct Route Bypass (if specified and not "auto"/"agent")
        let req_model = req.requested_model.as_deref().unwrap_or("auto");
        let lower = req_model.to_lowercase();
        let is_auto = lower == "auto" || lower == "agent" || lower == "delegate" || lower.is_empty();

        if !is_auto {
            let reg = self.registry.read().await;
            let (target_upstream, node_id, sel_model) = if let Some(m) = reg.find_model(req_model) {
                (m.endpoint, m.node_id, m.model_id)
            } else {
                (self.default_upstream.clone(), "DEFAULT".to_string(), req_model.to_string())
            };

            let execution_ms = t0.elapsed().as_secs_f64() * 1000.0;
            return AgentDelegationDecision {
                selected_model: sel_model,
                target_upstream,
                node_id,
                decision_source: DecisionSource::DirectBypass,
                intent: RoutingIntent::GeneralExecution,
                reasoning: format!("Direct model '{}' selected without arbitration", req_model),
                execution_ms,
            };
        }

        // 3. Dynamic Capability Matching via Cluster Registry
        let intent = classify_intent(&req.prompt);
        let reg = self.registry.read().await;
        let candidate: Option<ModelManifest> = reg.find_best_candidate(&intent);

        let execution_ms = t0.elapsed().as_secs_f64() * 1000.0;

        if let Some(m) = candidate {
            AgentDelegationDecision {
                selected_model: m.model_id.clone(),
                target_upstream: m.endpoint,
                node_id: m.node_id.clone(),
                decision_source: DecisionSource::DynamicCapability,
                intent: intent.clone(),
                reasoning: format!(
                    "Arbitrated as {:?}: matched node {} hosting model '{}' ({:?})",
                    intent, m.node_id, m.model_id, m.parameter_tier
                ),
                execution_ms,
            }
        } else {
            AgentDelegationDecision {
                selected_model: "default".to_string(),
                target_upstream: self.default_upstream.clone(),
                node_id: "LOCAL".to_string(),
                decision_source: DecisionSource::DynamicCapability,
                intent,
                reasoning: "No online candidate in cluster registry; routed to default upstream".to_string(),
                execution_ms,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ModelCapability;

    #[tokio::test]
    async fn test_agent_override_by_session() {
        let override_store = Arc::new(RwLock::new(RouteOverrideStore::new()));
        let registry = Arc::new(RwLock::new(ClusterModelRegistry::new()));
        
        {
            let mut reg = registry.write().await;
            reg.register_single_model("PC-1", "http://127.0.0.1:11434", "qwen3.8:27b", Some(vec![ModelCapability::ComplexReasoning]));
            reg.register_single_model("PC-2", "http://192.168.1.102:8642", "deepseek-r1:1.5b", Some(vec![ModelCapability::SanityVerification]));
        }

        let engine = AgentDelegationEngine::new(
            override_store.clone(),
            registry.clone(),
            "http://127.0.0.1:11434".to_string(),
        );

        // 1. Without override: a math proof routes to PC-1 (qwen3.8:27b)
        let req_normal = AgentDelegationRequest {
            prompt: "Provide formal mathematical proof of vector stability".to_string(),
            session_id: Some("session_general".to_string()),
            agent_id: None,
            tags: vec![],
            requested_model: Some("auto".to_string()),
        };
        let dec_normal = engine.decide(&req_normal).await;
        assert_eq!(dec_normal.node_id, "PC-1");
        assert_eq!(dec_normal.selected_model, "qwen3.8:27b");
        assert_eq!(dec_normal.decision_source, DecisionSource::DynamicCapability);

        // 2. Set an active override for session_pinned
        {
            let mut store = override_store.write().await;
            store.set_rule(RouteOverrideRule {
                rule_id: "rule-pin-pc2".to_string(),
                criteria: OverrideCriteria::Session("session_pinned".to_string()),
                target_model: "deepseek-r1:1.5b".to_string(),
                target_node: Some("PC-2".to_string()),
                reason: "Enforce low-latency GPU node for this sensitive session".to_string(),
                created_at: Utc::now().to_rfc3339(),
            });
        }

        // 3. With override: the same math prompt in session_pinned must be routed to PC-2!
        let req_pinned = AgentDelegationRequest {
            prompt: "Provide formal mathematical proof of vector stability".to_string(),
            session_id: Some("session_pinned".to_string()),
            agent_id: None,
            tags: vec![],
            requested_model: Some("auto".to_string()),
        };
        let dec_pinned = engine.decide(&req_pinned).await;
        assert_eq!(dec_pinned.node_id, "PC-2");
        assert_eq!(dec_pinned.selected_model, "deepseek-r1:1.5b");
        assert_eq!(dec_pinned.decision_source, DecisionSource::OverrideRule);
        assert!(dec_pinned.reasoning.contains("Enforce low-latency GPU node"));
    }

    #[tokio::test]
    async fn test_agent_override_by_agent_identity() {
        let override_store = Arc::new(RwLock::new(RouteOverrideStore::new()));
        let registry = Arc::new(RwLock::new(ClusterModelRegistry::new()));

        {
            let mut reg = registry.write().await;
            reg.register_single_model("PC-1", "http://127.0.0.1:11434", "qwen3.8:27b", None);
        }

        let engine = AgentDelegationEngine::new(
            override_store.clone(),
            registry.clone(),
            "http://127.0.0.1:11434".to_string(),
        );

        // Set rule pinning any traffic from "Hermes-Code-Agent"
        {
            let mut store = override_store.write().await;
            store.set_rule(RouteOverrideRule {
                rule_id: "rule-hermes-coder".to_string(),
                criteria: OverrideCriteria::Agent("Hermes-Code-Agent".to_string()),
                target_model: "qwen3.8:27b".to_string(),
                target_node: Some("PC-1".to_string()),
                reason: "Hermes coder assigned to PC-1 reasoning lead".to_string(),
                created_at: Utc::now().to_rfc3339(),
            });
        }

        let req = AgentDelegationRequest {
            prompt: "Any random prompt".to_string(),
            session_id: None,
            agent_id: Some("hermes-code-agent".to_string()),
            tags: vec![],
            requested_model: Some("auto".to_string()),
        };

        let dec = engine.decide(&req).await;
        assert_eq!(dec.decision_source, DecisionSource::OverrideRule);
        assert_eq!(dec.selected_model, "qwen3.8:27b");
    }
}
