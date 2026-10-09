use agent_trace_gw::agent_interface::get_agent_tools;
use serde_json::Value;

#[tokio::test]
async fn test_agent_tools_schema_conformance() {
    let tools_json = get_agent_tools().await;
    let data: Value = tools_json.0;

    assert_eq!(data["schema_version"], "1.0.0");
    assert_eq!(data["protocol"], "AgentTrace-Agent-Interface");

    let tools = data["tools"].as_array().expect("Tools must be an array");
    assert!(!tools.is_empty(), "Must expose at least one tool");

    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    assert!(tool_names.contains(&"verify_ledger"), "verify_ledger must be exposed");
    assert!(tool_names.contains(&"query_telemetry"), "query_telemetry must be exposed");
    assert!(tool_names.contains(&"inspect_cluster"), "inspect_cluster must be exposed");
    assert!(tool_names.contains(&"set_route_override"), "set_route_override must be exposed");
    assert!(tool_names.contains(&"get_access_policies"), "get_access_policies must be exposed");

    for tool in tools {
        assert!(tool.get("description").is_some(), "Every tool must have a description");
        assert!(tool.get("parameters").is_some(), "Every tool must have parameters object");
        assert_eq!(tool["parameters"]["type"], "object");
    }
}
