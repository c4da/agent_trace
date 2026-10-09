use agent_trace_gw::delegation_engine::{
    AgentDelegationEngine, AgentDelegationRequest, DecisionSource, OverrideCriteria, RouteOverrideRule, RouteOverrideStore,
};
use agent_trace_gw::registry::{ClusterModelRegistry, ModelCapability};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_delegation_rule_tag_matching() {
    let override_store = Arc::new(RwLock::new(RouteOverrideStore::new()));
    let registry = Arc::new(RwLock::new(ClusterModelRegistry::new()));

    {
        let mut reg = registry.write().await;
        reg.register_single_model("PC-1", "http://127.0.0.1:11434", "qwen3.8:27b", Some(vec![ModelCapability::ComplexReasoning]));
    }

    {
        let mut store = override_store.write().await;
        store.set_rule(RouteOverrideRule {
            rule_id: "rule-security-audit".to_string(),
            criteria: OverrideCriteria::Tag("security_audit".to_string()),
            target_model: "qwen3.8:27b".to_string(),
            target_node: Some("PC-1".to_string()),
            reason: "Route security audits to Qwen 27B".to_string(),
            created_at: Utc::now().to_rfc3339(),
        });
    }

    let engine = AgentDelegationEngine::new(
        override_store.clone(),
        registry.clone(),
        "http://127.0.0.1:11434".to_string(),
    );

    let req_matching = AgentDelegationRequest {
        prompt: "Check access permissions".to_string(),
        session_id: None,
        agent_id: None,
        tags: vec!["production".to_string(), "security_audit".to_string()],
        requested_model: Some("auto".to_string()),
    };

    let dec = engine.decide(&req_matching).await;
    assert_eq!(dec.decision_source, DecisionSource::OverrideRule);
    assert_eq!(dec.selected_model, "qwen3.8:27b");
    assert_eq!(dec.node_id, "PC-1");

    let req_non_matching = AgentDelegationRequest {
        prompt: "General question".to_string(),
        session_id: None,
        agent_id: None,
        tags: vec!["unrelated_tag".to_string()],
        requested_model: Some("auto".to_string()),
    };

    let dec_non = engine.decide(&req_non_matching).await;
    assert_ne!(dec_non.decision_source, DecisionSource::OverrideRule);
}

#[tokio::test]
async fn test_delegation_remove_rule() {
    let override_store = Arc::new(RwLock::new(RouteOverrideStore::new()));
    let _registry = Arc::new(RwLock::new(ClusterModelRegistry::new()));

    {
        let mut store = override_store.write().await;
        store.set_rule(RouteOverrideRule {
            rule_id: "temp-rule".to_string(),
            criteria: OverrideCriteria::Agent("temp-agent".to_string()),
            target_model: "gemma4:latest".to_string(),
            target_node: Some("PC-1".to_string()),
            reason: "Temporary agent rule".to_string(),
            created_at: Utc::now().to_rfc3339(),
        });
        assert_eq!(store.list_rules().len(), 1);

        let removed = store.remove_rule("temp-rule");
        assert!(removed);
        assert_eq!(store.list_rules().len(), 0);

        let removed_again = store.remove_rule("temp-rule");
        assert!(!removed_again);
    }
}
