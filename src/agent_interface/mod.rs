//! Agent-Centric Telemetry Control Interface for AgentTrace Gateway.
//!
//! Provides schema-defined endpoints and an MCP-compatible JSON-RPC interface
//! for external AI agents (Hermes, Claude Code, Copilot, watchdogs) to inspect,
//! verify, and govern the telemetry ledger and cluster routing.

pub mod cluster_ops;
pub mod executor;
pub mod mcp;
pub mod routes;
pub mod schemas;
pub mod types;

// Re-export public types and handlers for clean gateway access
pub use executor::{handle_agent_action, handle_agent_handoff};
pub use mcp::handle_mcp_rpc;
pub use routes::{
    handle_delegate_decide, handle_delegate_get_config, handle_delegate_list_overrides,
    handle_delegate_override, handle_delegate_remove_override, handle_delegate_set_config,
};
pub use schemas::get_agent_tools;
pub use types::{AgentActionRequest, AgentActionResponse, AgentInterfaceState};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_tools_registration() {
        let axum::Json(body) = get_agent_tools().await;
        let tools = body["tools"].as_array().expect("Expected tools array");
        assert!(tools.len() >= 17, "Expected at least 17 registered AI agent tools");

        let tool_names: Vec<String> = tools
            .iter()
            .filter_map(|t| t["name"].as_str().map(|s| s.to_string()))
            .collect();

        assert!(tool_names.contains(&"verify_ledger".to_string()));
        assert!(tool_names.contains(&"query_telemetry".to_string()));
        assert!(tool_names.contains(&"inspect_cluster".to_string()));
        assert!(tool_names.contains(&"flag_incident".to_string()));
        assert!(tool_names.contains(&"set_route_override".to_string()));
        assert!(tool_names.contains(&"analyze_telemetry".to_string()));
        assert!(tool_names.contains(&"get_access_policies".to_string()));
        assert!(tool_names.contains(&"set_access_policy".to_string()));
        assert!(tool_names.contains(&"evaluate_access_policy".to_string()));
        assert!(tool_names.contains(&"get_delegation_config".to_string()));
        assert!(tool_names.contains(&"set_delegation_config".to_string()));
        assert!(tool_names.contains(&"get_telemetry_handoff".to_string()));
        assert!(tool_names.contains(&"get_security_alerts".to_string()));
        assert!(tool_names.contains(&"acknowledge_alert".to_string()));
        assert!(tool_names.contains(&"list_containment_zones".to_string()));
        assert!(tool_names.contains(&"upsert_containment_zone".to_string()));
        assert!(tool_names.contains(&"confine_agent_to_playground".to_string()));
        assert!(tool_names.contains(&"check_cluster_health".to_string()));
        assert!(tool_names.contains(&"intercept_ai_guard".to_string()));
        assert!(tool_names.contains(&"protect_or_revert_cas_file".to_string()));
        assert!(tool_names.contains(&"encrypt_vault_secret".to_string()));
        assert!(tool_names.contains(&"export_siem_elk".to_string()));
        assert!(tool_names.contains(&"manage_admin_model_governance".to_string()));

        for t in tools {
            assert!(t["description"].is_string());
            assert_eq!(t["parameters"]["type"], "object");
        }
    }
}
