//! Execution engine for AI agent actions.
//!
//! Decomposes agent tool invocations into short, isolated helper functions.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::{json, Value};

use super::types::{AgentActionRequest, AgentActionResponse, AgentInterfaceState};
use crate::{
    delegation_engine::{AgentDelegationRequest, OverrideCriteria, RouteOverrideRule},
    policy::{AccessAction, AgentAccessPolicy},
    router::classify_and_route_dynamic,
    schema::{EventType, SecurityVerdict, TraceEvent},
};

/// Entrypoint for agent tool execution requests.
pub async fn handle_agent_action(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
    Json(req): Json<AgentActionRequest>,
) -> Result<Json<AgentActionResponse>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let t0 = std::time::Instant::now();
    let (success, result, error) = dispatch_tool(&state, &req).await;
    let execution_ms = t0.elapsed().as_secs_f64() * 1000.0;

    Ok(Json(AgentActionResponse {
        success,
        tool: req.tool,
        agent_id: req.agent_id,
        result,
        execution_ms,
        error,
    }))
}

/// Routes tool name to its dedicated executor.
async fn dispatch_tool(
    state: &AgentInterfaceState,
    req: &AgentActionRequest,
) -> (bool, Value, Option<String>) {
    match req.tool.as_str() {
        "verify_ledger" => exec_verify_ledger(state),
        "query_telemetry" => exec_query_telemetry(state, &req.parameters),
        "inspect_cluster" => exec_inspect_cluster(state).await,
        "flag_incident" => exec_flag_incident(state, &req.agent_id, &req.parameters),
        "explain_route" => exec_explain_route(state, &req.parameters).await,
        "decide_route" => exec_decide_route(state, &req.parameters).await,
        "set_route_override" => exec_set_route_override(state, &req.parameters).await,
        "list_route_overrides" => exec_list_route_overrides(state).await,
        "remove_route_override" => exec_remove_route_override(state, &req.parameters).await,
        "analyze_telemetry" => exec_analyze_telemetry(state),
        "get_access_policies" => exec_get_access_policies(state).await,
        "set_access_policy" => exec_set_access_policy(state, &req.parameters).await,
        "evaluate_access_policy" => exec_evaluate_access_policy(state, &req.parameters).await,
        "get_delegation_config" => exec_get_delegation_config(state).await,
        "set_delegation_config" => exec_set_delegation_config(state, &req.parameters).await,
        unknown => (false, json!({}), Some(format!("Unknown tool: '{}'", unknown))),
    }
}

/// Verifies cryptographic hash chain in SQLite.
fn exec_verify_ledger(state: &AgentInterfaceState) -> (bool, Value, Option<String>) {
    match state.db.verify_ledger() {
        Ok(report) => (true, serde_json::to_value(report).unwrap_or(json!({})), None),
        Err(e) => (false, json!({}), Some(format!("Ledger verification error: {}", e))),
    }
}

/// Filters recently committed events by optional substring and limit.
fn exec_query_telemetry(
    state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50).clamp(1, 200) as usize;
    let target_sub = params.get("target_contains").and_then(|v| v.as_str()).map(|s| s.to_lowercase());

    match state.db.get_recent_events(limit) {
        Ok(mut events) => {
            if let Some(ref sub) = target_sub {
                events.retain(|e| {
                    e.get("target")
                        .and_then(|t| t.as_str())
                        .map(|t| t.to_lowercase().contains(sub))
                        .unwrap_or(false)
                });
            }
            (true, json!({ "count": events.len(), "events": events }), None)
        }
        Err(e) => (false, json!({}), Some(format!("Query failed: {}", e))),
    }
}

/// Gathers node status and announced models across the cluster.
async fn exec_inspect_cluster(state: &AgentInterfaceState) -> (bool, Value, Option<String>) {
    let reg = state.registry.read().await;
    let active_models = reg.all_active_models();
    let summary = state.db.get_cluster_summary().ok();
    (
        true,
        json!({
            "cluster_summary": summary,
            "active_models": active_models,
            "model_count": active_models.len()
        }),
        None,
    )
}

/// Inserts a signed incident flag event into the ledger.
fn exec_flag_incident(
    state: &AgentInterfaceState,
    agent_id: &str,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let event_id = params.get("event_id").and_then(|v| v.as_str()).unwrap_or("");
    let reason = params.get("reason").and_then(|v| v.as_str()).unwrap_or("");

    if event_id.is_empty() || reason.is_empty() {
        return (false, json!({}), Some("Both 'event_id' and 'reason' are required".to_string()));
    }

    let mut incident_event = TraceEvent {
        event_id: uuid::Uuid::new_v4().to_string(),
        node_id: "GOVERNANCE".to_string(),
        session_id: "agent_governance".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_type: EventType::DecisionStep,
        target: format!("incident_flag:{}", event_id),
        action_details: json!({
            "flagged_by_agent": agent_id,
            "target_event_id": event_id,
            "justification": reason
        }),
        diff: None,
        provenance_sources: vec![agent_id.to_string()],
        security_verdict: SecurityVerdict::Flagged,
        security_reasons: vec![reason.to_string()],
        duration_ms: 0.0,
        prev_hash: "0".repeat(64),
        event_hash: "0".repeat(64),
        signature: None,
        public_key: None,
    };

    let _ = state.db.insert_event_signed(&mut incident_event, Some(&*state.identity));

    (
        true,
        json!({
            "incident_logged": true,
            "incident_event_id": incident_event.event_id,
            "target_event_id": event_id,
            "verdict": "Flagged"
        }),
        None,
    )
}

/// Previews model routing without executing inference.
async fn exec_explain_route(
    state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let prompt = params.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
    let requested_model = params.get("requested_model").and_then(|v| v.as_str()).unwrap_or("auto");

    let reg = state.registry.read().await;
    let decision = classify_and_route_dynamic(
        prompt,
        requested_model,
        false,
        &reg,
        &state.upstream_url,
    );

    (true, serde_json::to_value(decision).unwrap_or(json!({})), None)
}

/// Evaluates prompt against dynamic delegation rules.
async fn exec_decide_route(
    state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let prompt = params.get("prompt").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let session_id = params.get("session_id").and_then(|v| v.as_str()).map(|s| s.to_string());
    let agent_id = params.get("agent_id").and_then(|v| v.as_str()).map(|s| s.to_string());
    let requested_model = params.get("requested_model").and_then(|v| v.as_str()).map(|s| s.to_string());
    let tags = params
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let delegation_req = AgentDelegationRequest {
        prompt,
        session_id,
        agent_id,
        tags,
        requested_model,
    };

    let decision = state.delegation_engine.decide(&delegation_req).await;
    (true, serde_json::to_value(decision).unwrap_or(json!({})), None)
}

/// Stores a custom routing override in memory and disk.
async fn exec_set_route_override(
    state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let criteria_type = params.get("criteria_type").and_then(|v| v.as_str()).unwrap_or("session");
    let criteria_value = params.get("criteria_value").and_then(|v| v.as_str()).unwrap_or("");
    let target_model = params.get("target_model").and_then(|v| v.as_str()).unwrap_or("");
    let target_node = params.get("target_node").and_then(|v| v.as_str()).map(|s| s.to_string());
    let reason = params.get("reason").and_then(|v| v.as_str()).unwrap_or("Agent specified route override");
    let rule_id = params.get("rule_id").and_then(|v| v.as_str()).unwrap_or("");

    if criteria_value.is_empty() || target_model.is_empty() {
        return (false, json!({}), Some("Both 'criteria_value' and 'target_model' are required".to_string()));
    }

    let criteria = match criteria_type.to_lowercase().as_str() {
        "agent" => OverrideCriteria::Agent(criteria_value.to_string()),
        "tag" => OverrideCriteria::Tag(criteria_value.to_string()),
        "model" => OverrideCriteria::Model(criteria_value.to_string()),
        "prompt" | "prompt_pattern" | "promptpattern" => OverrideCriteria::PromptPattern(criteria_value.to_string()),
        _ => OverrideCriteria::Session(criteria_value.to_string()),
    };

    let rule = RouteOverrideRule {
        rule_id: rule_id.to_string(),
        criteria,
        target_model: target_model.to_string(),
        target_node,
        reason: reason.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let assigned_id = {
        let mut store = state.override_store.write().await;
        store.set_rule(rule)
    };

    (
        true,
        json!({
            "applied": true,
            "rule_id": assigned_id,
            "criteria_type": criteria_type,
            "criteria_value": criteria_value,
            "target_model": target_model,
            "reason": reason
        }),
        None,
    )
}

/// Lists all active route overrides.
async fn exec_list_route_overrides(state: &AgentInterfaceState) -> (bool, Value, Option<String>) {
    let store = state.override_store.read().await;
    let rules = store.list_rules();
    (true, json!({ "count": rules.len(), "rules": rules }), None)
}

/// Removes a single routing override rule by its ID.
async fn exec_remove_route_override(
    state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let rule_id = params.get("rule_id").and_then(|v| v.as_str()).unwrap_or("");
    if rule_id.is_empty() {
        return (false, json!({}), Some("'rule_id' parameter is required".to_string()));
    }

    let mut store = state.override_store.write().await;
    let removed = store.remove_rule(rule_id);
    (true, json!({ "rule_id": rule_id, "removed": removed }), None)
}

/// Computes telemetry statistics and threat ratios.
fn exec_analyze_telemetry(state: &AgentInterfaceState) -> (bool, Value, Option<String>) {
    match state.db.get_telemetry_analytics() {
        Ok(report) => (true, serde_json::to_value(report).unwrap_or(json!({})), None),
        Err(e) => (false, json!({}), Some(format!("Analytics computation failed: {}", e))),
    }
}

/// Lists active sandbox policies.
async fn exec_get_access_policies(state: &AgentInterfaceState) -> (bool, Value, Option<String>) {
    let guard = state.policy.read().await;
    let policies = guard.list_policies();
    (true, json!({ "count": policies.len(), "policies": policies }), None)
}

/// Sets or updates a policy for an agent.
async fn exec_set_access_policy(
    state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let agent_id = params.get("agent_id").and_then(|v| v.as_str()).unwrap_or("");
    if agent_id.is_empty() {
        return (false, json!({}), Some("'agent_id' parameter is required".to_string()));
    }

    let extract_strings = |key: &str| {
        params
            .get(key)
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|s| s.as_str().map(|x| x.to_string())).collect())
            .unwrap_or_default()
    };

    let policy = AgentAccessPolicy {
        agent_id: agent_id.to_string(),
        read_whitelist: extract_strings("read_whitelist"),
        write_whitelist: extract_strings("write_whitelist"),
        network_whitelist: extract_strings("network_whitelist"),
        blocked_patterns: extract_strings("blocked_patterns"),
        enforcing: params.get("enforcing").and_then(|v| v.as_bool()).unwrap_or(true),
        confine_to_playgrounds: params.get("confine_to_playgrounds").and_then(|v| v.as_bool()).unwrap_or(false),
    };

    {
        let mut guard = state.policy.write().await;
        guard.set_policy(policy.clone());
    }

    (true, serde_json::to_value(policy).unwrap_or(json!({})), None)
}

/// Evaluates if an action is permitted by policy without running it.
async fn exec_evaluate_access_policy(
    state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let agent_id = params.get("agent_id").and_then(|v| v.as_str()).unwrap_or("*");
    let action_str = params.get("action").and_then(|v| v.as_str()).unwrap_or("Read");
    let target = params.get("target").and_then(|v| v.as_str()).unwrap_or("");

    let action = match action_str.to_lowercase().as_str() {
        "write" => AccessAction::Write,
        "execute" => AccessAction::Execute,
        "networkpull" | "network" => AccessAction::NetworkPull,
        _ => AccessAction::Read,
    };

    let guard = state.policy.read().await;
    let decision = guard.evaluate(agent_id, action, target);
    (true, serde_json::to_value(decision).unwrap_or(json!({})), None)
}

/// Returns cluster delegation options.
async fn exec_get_delegation_config(state: &AgentInterfaceState) -> (bool, Value, Option<String>) {
    let cfg = state.delegation_engine.get_config().await;
    (true, serde_json::to_value(cfg).unwrap_or(json!({})), None)
}

/// Updates cluster delegation options.
async fn exec_set_delegation_config(
    state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let mut current = state.delegation_engine.get_config().await;
    if let Some(v) = params.get("intercept_external_models").and_then(|v| v.as_bool()) {
        current.intercept_external_models = v;
    }
    if let Some(v) = params.get("zero_token_mode").and_then(|v| v.as_bool()) {
        current.zero_token_mode = v;
    }
    if let Some(v) = params.get("auto_switch_offline_models").and_then(|v| v.as_bool()) {
        current.auto_switch_offline_models = v;
    }
    if let Some(v) = params.get("default_fallback_model").and_then(|v| v.as_str()) {
        current.default_fallback_model = v.to_string();
    }

    state.delegation_engine.set_config(current.clone()).await;
    (true, serde_json::to_value(current).unwrap_or(json!({})), None)
}
