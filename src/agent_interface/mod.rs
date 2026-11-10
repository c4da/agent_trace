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
        assert_eq!(tools.len(), 17, "Expected exactly 17 registered tools");

        let tool_names: Vec<String> = tools
            .iter()
            .filter_map(|t| t["name"].as_str().map(|s| s.to_string()))
            .collect();

        assert!(tool_names.contains(&"verify_ledger".to_string()), "missing verify_ledger");
        assert!(tool_names.contains(&"query_telemetry".to_string()), "missing query_telemetry");
        assert!(tool_names.contains(&"inspect_cluster".to_string()), "missing inspect_cluster");
        assert!(tool_names.contains(&"flag_incident".to_string()), "missing flag_incident");
        assert!(tool_names.contains(&"set_route_override".to_string()), "missing set_route_override");
        assert!(tool_names.contains(&"analyze_telemetry".to_string()), "missing analyze_telemetry");
        assert!(tool_names.contains(&"get_delegation_config".to_string()), "missing get_delegation_config");
        assert!(tool_names.contains(&"set_delegation_config".to_string()), "missing set_delegation_config");
        assert!(tool_names.contains(&"get_telemetry_handoff".to_string()), "missing get_telemetry_handoff");
        assert!(tool_names.contains(&"get_security_alerts".to_string()), "missing get_security_alerts");
        assert!(tool_names.contains(&"acknowledge_alert".to_string()), "missing acknowledge_alert");
        assert!(tool_names.contains(&"record_project_decision".to_string()), "missing record_project_decision");
        assert!(tool_names.contains(&"dispatch_cluster_prompt".to_string()), "missing dispatch_cluster_prompt");
        assert!(tool_names.contains(&"explain_route".to_string()), "missing explain_route");
        assert!(tool_names.contains(&"decide_route".to_string()), "missing decide_route");
        assert!(tool_names.contains(&"list_route_overrides".to_string()), "missing list_route_overrides");
        assert!(tool_names.contains(&"remove_route_override".to_string()), "missing remove_route_override");

        for t in tools {
            assert!(t["description"].is_string());
            assert_eq!(t["parameters"]["type"], "object");
        }
    }
}
