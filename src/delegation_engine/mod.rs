//! Autonomous AI Agent Delegation & Arbitration Engine for AgentTrace Gateway.
//!
//! Enables AI agents to dynamically decide routing, override model selections, and
//! arbitrate prompt distribution across distributed cluster nodes at runtime.

pub mod engine;
pub mod pattern;
pub mod store;
pub mod types;

pub use engine::AgentDelegationEngine;
pub use pattern::{is_external_cloud_model, matches_pattern_str};
pub use store::RouteOverrideStore;
pub use types::{
    AgentDelegationDecision, AgentDelegationRequest, DecisionSource, DelegationConfig,
    OverrideCriteria, RouteOverrideRule,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{ClusterModelRegistry, ModelCapability};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn sample_registry() -> Arc<RwLock<ClusterModelRegistry>> {
        let mut reg = ClusterModelRegistry::new();
        reg.register_single_model(
            "PC-1",
            "http://127.0.0.1:11434",
            "qwen3.8:27b",
            Some(vec![ModelCapability::ComplexReasoning]),
        );
        reg.register_single_model(
            "PC-1",
            "http://127.0.0.1:11434",
            "qwen3:14b",
            Some(vec![ModelCapability::FastCodeGeneration]),
        );
        reg.register_single_model(
            "PC-2",
            "http://127.0.0.1:8642",
            "deepseek-r1:1.5b",
            Some(vec![ModelCapability::SanityVerification]),
        );
        Arc::new(RwLock::new(reg))
    }

    #[tokio::test]
    async fn test_agent_override_by_session() {
        let store = Arc::new(RwLock::new(RouteOverrideStore::new()));
        let reg = sample_registry();
        let engine = AgentDelegationEngine::new(store.clone(), reg, "http://127.0.0.1:11434".to_string());

        let rule = RouteOverrideRule {
            rule_id: "test-sess-1".to_string(),
            criteria: OverrideCriteria::Session("sec-session-99".to_string()),
            target_model: "deepseek-r1:1.5b".to_string(),
            target_node: Some("PC-2".to_string()),
            reason: "Security audit requires deepseek verification".to_string(),
            created_at: "".to_string(),
        };
        store.write().await.set_rule(rule);

        let req = AgentDelegationRequest {
            prompt: "Refactor this database query".to_string(),
            session_id: Some("sec-session-99".to_string()),
            agent_id: None,
            tags: vec![],
            requested_model: Some("auto".to_string()),
        };

        let decision = engine.decide(&req).await;
        assert_eq!(decision.selected_model, "deepseek-r1:1.5b");
        assert_eq!(decision.node_id, "PC-2");
        assert_eq!(decision.decision_source, DecisionSource::OverrideRule);
    }

    #[tokio::test]
    async fn test_agent_override_by_agent_identity() {
        let store = Arc::new(RwLock::new(RouteOverrideStore::new()));
        let reg = sample_registry();
        let engine = AgentDelegationEngine::new(store.clone(), reg, "http://127.0.0.1:11434".to_string());

        let rule = RouteOverrideRule {
            rule_id: "hermes-rule".to_string(),
            criteria: OverrideCriteria::Agent("hermes*".to_string()),
            target_model: "qwen3.8:27b".to_string(),
            target_node: Some("PC-1".to_string()),
            reason: "Hermes agent routed to heavy model".to_string(),
            created_at: "".to_string(),
        };
        store.write().await.set_rule(rule);

        let req = AgentDelegationRequest {
            prompt: "Simple task".to_string(),
            session_id: None,
            agent_id: Some("hermes-coder".to_string()),
            tags: vec![],
            requested_model: Some("auto".to_string()),
        };

        let decision = engine.decide(&req).await;
        assert_eq!(decision.selected_model, "qwen3.8:27b");
        assert_eq!(decision.decision_source, DecisionSource::OverrideRule);
    }

    #[tokio::test]
    async fn test_agent_override_by_prompt_pattern() {
        let store = Arc::new(RwLock::new(RouteOverrideStore::new()));
        let reg = sample_registry();
        let engine = AgentDelegationEngine::new(store.clone(), reg, "http://127.0.0.1:11434".to_string());

        let rule = RouteOverrideRule {
            rule_id: "test-pattern".to_string(),
            criteria: OverrideCriteria::PromptPattern("*cargo test*".to_string()),
            target_model: "qwen3:14b".to_string(),
            target_node: Some("PC-1".to_string()),
            reason: "Fast test runner routing".to_string(),
            created_at: "".to_string(),
        };
        store.write().await.set_rule(rule);

        let req = AgentDelegationRequest {
            prompt: "Please run cargo test --lib".to_string(),
            session_id: None,
            agent_id: None,
            tags: vec![],
            requested_model: Some("auto".to_string()),
        };

        let decision = engine.decide(&req).await;
        assert_eq!(decision.selected_model, "qwen3:14b");
        assert_eq!(decision.decision_source, DecisionSource::OverrideRule);
    }

    #[tokio::test]
    async fn test_agent_override_by_model_pattern() {
        let store = Arc::new(RwLock::new(RouteOverrideStore::new()));
        let reg = sample_registry();
        let engine = AgentDelegationEngine::new(store.clone(), reg, "http://127.0.0.1:11434".to_string());

        let rule = RouteOverrideRule {
            rule_id: "force-gpt-to-pc2".to_string(),
            criteria: OverrideCriteria::Model("gpt*".to_string()),
            target_model: "deepseek-r1:1.5b".to_string(),
            target_node: Some("PC-2".to_string()),
            reason: "Intercept GPT models to PC-2".to_string(),
            created_at: "".to_string(),
        };
        store.write().await.set_rule(rule);

        let req = AgentDelegationRequest {
            prompt: "Summarize this".to_string(),
            session_id: None,
            agent_id: None,
            tags: vec![],
            requested_model: Some("gpt-4o-mini".to_string()),
        };

        let decision = engine.decide(&req).await;
        assert_eq!(decision.selected_model, "deepseek-r1:1.5b");
        assert_eq!(decision.decision_source, DecisionSource::OverrideRule);
    }

    #[tokio::test]
    async fn test_cloud_interception_gemini_flash() {
        let store = Arc::new(RwLock::new(RouteOverrideStore::new()));
        let reg = sample_registry();
        let engine = AgentDelegationEngine::new(store.clone(), reg, "http://127.0.0.1:11434".to_string());

        let req = AgentDelegationRequest {
            prompt: "Write a Rust unit test for ledger verification".to_string(),
            session_id: None,
            agent_id: None,
            tags: vec![],
            requested_model: Some("gemini-2.5-flash".to_string()),
        };

        let decision = engine.decide(&req).await;
        assert_eq!(decision.decision_source, DecisionSource::CloudInterception);
        assert_eq!(decision.selected_model, "qwen3:14b");
    }

    #[tokio::test]
    async fn test_offline_model_fallback_switch() {
        let store = Arc::new(RwLock::new(RouteOverrideStore::new()));
        let reg = sample_registry();
        let engine = AgentDelegationEngine::new(store.clone(), reg, "http://127.0.0.1:11434".to_string());

        let req = AgentDelegationRequest {
            prompt: "Calculate mathematical eigenvalues".to_string(),
            session_id: None,
            agent_id: None,
            tags: vec![],
            requested_model: Some("llama3:70b-nonexistent".to_string()),
        };

        let decision = engine.decide(&req).await;
        assert_eq!(decision.decision_source, DecisionSource::FallbackSwitch);
        assert_eq!(decision.selected_model, "qwen3.8:27b");
    }
}
