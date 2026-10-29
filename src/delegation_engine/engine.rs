//! Core delegation engine orchestrating prompt arbitration.

use super::{
    pattern::is_external_cloud_model,
    store::RouteOverrideStore,
    types::{
        AgentDelegationDecision, AgentDelegationRequest, DecisionSource, DelegationConfig,
    },
};
use crate::{
    registry::ClusterModelRegistry,
    router::classify_intent,
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Core AI agent delegation arbitrator.
#[derive(Clone)]
pub struct AgentDelegationEngine {
    override_store: Arc<RwLock<RouteOverrideStore>>,
    registry: Arc<RwLock<ClusterModelRegistry>>,
    default_upstream: String,
    config: Arc<RwLock<DelegationConfig>>,
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
            config: Arc::new(RwLock::new(DelegationConfig::default())),
        }
    }

    pub fn with_config(
        override_store: Arc<RwLock<RouteOverrideStore>>,
        registry: Arc<RwLock<ClusterModelRegistry>>,
        default_upstream: String,
        config: DelegationConfig,
    ) -> Self {
        Self {
            override_store,
            registry,
            default_upstream,
            config: Arc::new(RwLock::new(config)),
        }
    }

    pub async fn get_config(&self) -> DelegationConfig {
        self.config.read().await.clone()
    }

    pub async fn set_config(&self, cfg: DelegationConfig) {
        let mut guard = self.config.write().await;
        *guard = cfg;
    }

    /// Arbitrates routing: checks overrides, token guards, and capability heuristics.
    pub async fn decide(&self, req: &AgentDelegationRequest) -> AgentDelegationDecision {
        let t0 = std::time::Instant::now();
        let cfg = self.config.read().await.clone();

        // 1. Check active overrides
        if let Some(decision) = self.try_override(req, t0).await {
            return decision;
        }

        // 2. Intercept external cloud models
        if let Some(decision) = self.try_cloud_interception(req, &cfg, t0).await {
            return decision;
        }

        // 3. Resolve explicit local requests
        if let Some(decision) = self.try_direct_or_fallback(req, &cfg, t0).await {
            return decision;
        }

        // 4. Dynamic intent and capability match
        self.dynamic_capability_route(req, t0).await
    }

    /// Checks if an active override rule matches the request.
    async fn try_override(
        &self,
        req: &AgentDelegationRequest,
        t0: std::time::Instant,
    ) -> Option<AgentDelegationDecision> {
        let store = self.override_store.read().await;
        let rule = store.find_matching_override(req)?;

        let reg = self.registry.read().await;
        let (target_upstream, node_id, sel_model) = if let Some(m) = reg.find_model(&rule.target_model) {
            (m.endpoint, m.node_id, m.model_id)
        } else if rule.target_model.eq_ignore_ascii_case("auto") {
            let intent = classify_intent(&req.prompt);
            if let Some(cand) = reg.find_best_candidate(&intent) {
                (cand.endpoint, cand.node_id, cand.model_id)
            } else {
                (
                    self.default_upstream.clone(),
                    rule.target_node.clone().unwrap_or_else(|| "OVERRIDE".to_string()),
                    "auto".to_string(),
                )
            }
        } else {
            let nid = rule.target_node.unwrap_or_else(|| "OVERRIDE".to_string());
            (self.default_upstream.clone(), nid, rule.target_model.clone())
        };

        Some(AgentDelegationDecision {
            selected_model: sel_model,
            target_upstream,
            node_id,
            decision_source: DecisionSource::OverrideRule,
            intent: classify_intent(&req.prompt),
            reasoning: format!("Route override rule applied ({}): {}", rule.rule_id, rule.reason),
            execution_ms: t0.elapsed().as_secs_f64() * 1000.0,
        })
    }

    /// Intercepts commercial cloud models to save external API tokens.
    async fn try_cloud_interception(
        &self,
        req: &AgentDelegationRequest,
        cfg: &DelegationConfig,
        t0: std::time::Instant,
    ) -> Option<AgentDelegationDecision> {
        let req_model = req.requested_model.as_deref().unwrap_or("auto");
        if req_model.eq_ignore_ascii_case("auto") || !is_external_cloud_model(req_model) {
            return None;
        }
        if !cfg.intercept_external_models && !cfg.zero_token_mode {
            return None;
        }

        let intent = classify_intent(&req.prompt);
        let reg = self.registry.read().await;
        let candidate = reg.find_best_candidate(&intent);
        let execution_ms = t0.elapsed().as_secs_f64() * 1000.0;

        if let Some(m) = candidate {
            Some(AgentDelegationDecision {
                selected_model: m.model_id.clone(),
                target_upstream: m.endpoint,
                node_id: m.node_id.clone(),
                decision_source: DecisionSource::CloudInterception,
                intent: intent.clone(),
                reasoning: format!(
                    "Zero-Token Guard intercepted '{}': switched to '{}' on {} ({:?})",
                    req_model, m.model_id, m.node_id, intent
                ),
                execution_ms,
            })
        } else {
            Some(AgentDelegationDecision {
                selected_model: "qwen3.8:27b".to_string(),
                target_upstream: self.default_upstream.clone(),
                node_id: "LOCAL".to_string(),
                decision_source: DecisionSource::CloudInterception,
                intent,
                reasoning: format!(
                    "Zero-Token Guard intercepted '{}': routed to default local upstream",
                    req_model
                ),
                execution_ms,
            })
        }
    }

    /// Resolves explicit model requests or falls back to an available model if offline.
    async fn try_direct_or_fallback(
        &self,
        req: &AgentDelegationRequest,
        cfg: &DelegationConfig,
        t0: std::time::Instant,
    ) -> Option<AgentDelegationDecision> {
        let req_model = req.requested_model.as_deref().unwrap_or("auto");
        if req_model.eq_ignore_ascii_case("auto")
            || req_model.eq_ignore_ascii_case("agent")
            || req_model.eq_ignore_ascii_case("delegate")
            || req_model.is_empty()
        {
            return None;
        }

        let reg = self.registry.read().await;
        if let Some(m) = reg.find_model(req_model) {
            return Some(AgentDelegationDecision {
                selected_model: m.model_id,
                target_upstream: m.endpoint,
                node_id: m.node_id.clone(),
                decision_source: DecisionSource::DirectBypass,
                intent: classify_intent(&req.prompt),
                reasoning: format!("Direct model '{}' matched node {}", req_model, m.node_id),
                execution_ms: t0.elapsed().as_secs_f64() * 1000.0,
            });
        }

        if cfg.auto_switch_offline_models {
            let intent = classify_intent(&req.prompt);
            if let Some(cand) = reg.find_best_candidate(&intent) {
                return Some(AgentDelegationDecision {
                    selected_model: cand.model_id.clone(),
                    target_upstream: cand.endpoint,
                    node_id: cand.node_id.clone(),
                    decision_source: DecisionSource::FallbackSwitch,
                    intent: intent.clone(),
                    reasoning: format!(
                        "Model '{}' unavailable; switched to '{}' on {}",
                        req_model, cand.model_id, cand.node_id
                    ),
                    execution_ms: t0.elapsed().as_secs_f64() * 1000.0,
                });
            }
        }

        Some(AgentDelegationDecision {
            selected_model: req_model.to_string(),
            target_upstream: self.default_upstream.clone(),
            node_id: "DEFAULT".to_string(),
            decision_source: DecisionSource::DirectBypass,
            intent: classify_intent(&req.prompt),
            reasoning: format!("Direct model '{}' selected without arbitration", req_model),
            execution_ms: t0.elapsed().as_secs_f64() * 1000.0,
        })
    }

    /// Selects cluster model based on prompt classification intent.
    async fn dynamic_capability_route(
        &self,
        req: &AgentDelegationRequest,
        t0: std::time::Instant,
    ) -> AgentDelegationDecision {
        let intent = classify_intent(&req.prompt);
        let reg = self.registry.read().await;
        let candidate = reg.find_best_candidate(&intent);
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
                selected_model: "gemma4:latest".to_string(),
                target_upstream: self.default_upstream.clone(),
                node_id: "PC-1".to_string(),
                decision_source: DecisionSource::DynamicCapability,
                intent,
                reasoning: "No registered cluster models; routed to default local model".to_string(),
                execution_ms,
            }
        }
    }
}
