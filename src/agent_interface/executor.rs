//! Execution engine for AI agent actions.
//!
//! Decomposes agent tool invocations into short, isolated helper functions.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::{json, Value};

use super::{
    cluster_ops::*,
    types::{AgentActionRequest, AgentActionResponse, AgentInterfaceState},
};
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
        "record_project_decision" => exec_record_project_decision(state, &req.agent_id, &req.parameters).await,
        "get_workgroup_rules" => exec_get_workgroup_rules(&req.parameters).await,
        "upsert_workgroup" => exec_upsert_workgroup(&req.parameters).await,
        "attach_workgroup_rule" => exec_attach_workgroup_rule(&req.parameters).await,
        "deploy_cluster_worker" => exec_deploy_cluster_worker(state, &req.agent_id, &req.parameters).await,
        "check_worker_connection" => exec_check_worker_connection(state, &req.parameters).await,
        "check_cluster_health" => exec_check_cluster_health(state, &req.parameters).await,
        "get_cluster_bootstrap_command" => exec_get_cluster_bootstrap_command(state, &req.parameters).await,
        "dispatch_cluster_prompt" => exec_dispatch_cluster_prompt(state, &req.agent_id, &req.parameters).await,
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
        "get_telemetry_handoff" => exec_get_telemetry_handoff(state, &req.parameters).await,
        "get_security_alerts" => exec_get_security_alerts(state, &req.parameters),
        "acknowledge_alert" => exec_acknowledge_alert(state, &req.parameters),
        "list_containment_zones" => exec_list_containment_zones(state).await,
        "upsert_containment_zone" => exec_upsert_containment_zone(state, &req.parameters).await,
        "confine_agent_to_playground" => exec_confine_agent_to_playground(state, &req.parameters).await,
        "intercept_ai_guard" => exec_intercept_ai_guard(state, &req.agent_id, &req.parameters).await,
        "protect_or_revert_cas_file" => exec_protect_or_revert_cas_file(&req.parameters),
        "encrypt_vault_secret" => exec_encrypt_vault_secret(&req.parameters),
        "export_siem_elk" => exec_export_siem_elk(state, &req.parameters),
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

/// HTTP GET handler for `/api/v1/agent/handoff` — single-call telemetry pickup snapshot for another agent.
pub async fn handle_agent_handoff(State(state): State<AgentInterfaceState>) -> Json<Value> {
    let (_, val, _) = exec_get_telemetry_handoff(&state, &json!({ "recent_limit": 15 })).await;
    Json(val)
}

/// Builds a compact, high-density Telemetry Pickup & Handoff Snapshot for another AI agent.
async fn exec_get_telemetry_handoff(
    state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let limit = params
        .get("recent_limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(15)
        .clamp(1, 100) as usize;

    let summary = state.db.get_cluster_summary().ok();
    let ledger = state.db.verify_ledger().ok();
    let active_alerts = state.db.list_security_alerts(false).unwrap_or_default();
    let recent_events = state.db.get_recent_events(limit * 2).unwrap_or_default();

    let recent_diffs: Vec<Value> = recent_events
        .iter()
        .filter(|e| e.get("diff").and_then(|d| d.as_str()).map(|s| !s.is_empty()).unwrap_or(false))
        .take(limit)
        .map(|e| {
            json!({
                "event_id": e.get("event_id"),
                "timestamp": e.get("timestamp"),
                "node_id": e.get("node_id"),
                "target": e.get("target"),
                "security_verdict": e.get("security_verdict"),
                "diff": e.get("diff")
            })
        })
        .collect();

    let (zones, policies) = {
        let guard = state.policy.read().await;
        (guard.list_zones(), guard.list_policies())
    };

    let reg = state.registry.read().await;
    let active_models = reg.all_active_models();

    let snapshot = json!({
        "protocol": "AgentTrace-Telemetry-Handoff-v1",
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "cluster_summary": summary,
        "ledger_integrity": ledger,
        "active_security_alerts_count": active_alerts.len(),
        "active_security_alerts": active_alerts.into_iter().take(limit).collect::<Vec<_>>(),
        "recent_code_diffs": recent_diffs,
        "recent_events": recent_events.into_iter().take(limit).collect::<Vec<_>>(),
        "containment_zones": zones,
        "agent_policies": policies,
        "active_cluster_models": active_models,
        "agent_pickup_endpoints": {
            "handoff_digest_get": "GET /api/v1/agent/handoff",
            "tool_schemas_get": "GET /api/v1/agent/tools",
            "tool_execute_post": "POST /api/v1/agent/action",
            "mcp_jsonrpc_post": "POST /api/v1/mcp",
            "alerts_get": "GET /api/v1/alerts",
            "zones_get_post": "GET|POST /api/v1/zones",
            "playground_confine_post": "POST /api/v1/zones/confine"
        }
    });

    (true, snapshot, None)
}

/// Lists unresolved or historical security alerts for an auditing agent.
fn exec_get_security_alerts(
    state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let include_acked = params
        .get("include_acked")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    match state.db.list_security_alerts(include_acked) {
        Ok(alerts) => (
            true,
            json!({
                "include_acked": include_acked,
                "count": alerts.len(),
                "alerts": alerts
            }),
            None,
        ),
        Err(e) => (false, json!({}), Some(format!("Failed to list alerts: {}", e))),
    }
}

/// Non-destructively acknowledges a single alert or resets all active alerts.
fn exec_acknowledge_alert(
    state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let reset_all = params
        .get("reset_all")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if reset_all {
        match state.db.acknowledge_all_alerts() {
            Ok(count) => (
                true,
                json!({ "status": "reset_all", "acknowledged_count": count }),
                None,
            ),
            Err(e) => (false, json!({}), Some(format!("Reset alerts error: {}", e))),
        }
    } else {
        let event_id = params.get("event_id").and_then(|v| v.as_str()).unwrap_or("");
        if event_id.is_empty() {
            return (
                false,
                json!({}),
                Some("Provide 'event_id' or set 'reset_all': true".to_string()),
            );
        }
        match state.db.acknowledge_alert(event_id) {
            Ok(updated) => (
                true,
                json!({ "status": "acknowledged", "event_id": event_id, "updated": updated }),
                None,
            ),
            Err(e) => (false, json!({}), Some(format!("Acknowledge error: {}", e))),
        }
    }
}

/// Lists all configured Safe Zones and Danger Zone Playgrounds.
async fn exec_list_containment_zones(
    state: &AgentInterfaceState,
) -> (bool, Value, Option<String>) {
    let guard = state.policy.read().await;
    let zones = guard.list_zones();
    (true, json!({ "count": zones.len(), "zones": zones }), None)
}

/// Adds or updates a Safe Zone or Danger Zone Playground.
async fn exec_upsert_containment_zone(
    state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let path_prefix = params.get("path_prefix").and_then(|v| v.as_str()).unwrap_or("");
    let kind_str = params.get("kind").and_then(|v| v.as_str()).unwrap_or("VaultNoAccess");

    if name.is_empty() || path_prefix.is_empty() {
        return (
            false,
            json!({}),
            Some("'name' and 'path_prefix' are required".to_string()),
        );
    }

    let kind = match kind_str {
        "GuardedReadOnly" => crate::zones::ZoneKind::GuardedReadOnly,
        "AgentPlayground" => crate::zones::ZoneKind::AgentPlayground,
        _ => crate::zones::ZoneKind::VaultNoAccess,
    };

    let applies_to = params
        .get("applies_to")
        .and_then(|v| v.as_str())
        .unwrap_or("*")
        .to_string();
    let zone_id = params
        .get("zone_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let exempt_agents = params
        .get("exempt_agents")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let zone = crate::zones::ContainmentZone {
        zone_id,
        name: name.to_string(),
        path_prefix: path_prefix.to_string(),
        kind,
        applies_to,
        exempt_agents,
        description: String::new(),
    };

    let saved = {
        let mut guard = state.policy.write().await;
        guard.upsert_zone(zone)
    };
    (true, serde_json::to_value(saved).unwrap_or(json!({})), None)
}

/// Locks or unlocks an agent (or '*' all agents) to Danger Zone Playgrounds.
async fn exec_confine_agent_to_playground(
    state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let agent_id = params.get("agent_id").and_then(|v| v.as_str()).unwrap_or("*");
    let confined = params
        .get("confined")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let updated = {
        let mut guard = state.policy.write().await;
        guard.set_playground_confinement(agent_id, confined)
    };
    (true, serde_json::to_value(updated).unwrap_or(json!({})), None)
}


/// Runs synchronous Pre-Execution Hard AI Guard inspection via MCP/Agent interface.
async fn exec_intercept_ai_guard(
    state: &AgentInterfaceState,
    agent_id: &str,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("LlmInference").to_string();
    let target_path = params.get("target_path").and_then(|v| v.as_str()).map(|s| s.to_string());
    let command = params.get("command").and_then(|v| v.as_str()).map(|s| s.to_string());
    let prompt_text = params.get("prompt_text").and_then(|v| v.as_str()).map(|s| s.to_string());
    let mode_override = match params.get("mode").and_then(|v| v.as_str()).unwrap_or("HardBlock") {
        "RedactAndForward" => Some(crate::ai_guard::GuardEnforcementMode::RedactAndForward),
        "AuditOnly" => Some(crate::ai_guard::GuardEnforcementMode::AuditOnly),
        _ => Some(crate::ai_guard::GuardEnforcementMode::HardBlock),
    };

    let req = crate::ai_guard::GuardInterceptRequest {
        agent_id: agent_id.to_string(),
        action,
        target_path,
        command,
        prompt_text,
        mode_override,
    };

    let res = {
        let p_guard = state.policy.read().await;
        crate::ai_guard::evaluate_hard_guard(&req, &p_guard, crate::ai_guard::GuardEnforcementMode::HardBlock)
    };
    (true, serde_json::to_value(res).unwrap_or(json!({})), None)
}

/// Snapshots, enforces/reverts, or lists files in the Pre-Image CAS Guard.
fn exec_protect_or_revert_cas_file(params: &Value) -> (bool, Value, Option<String>) {
    let op = params.get("operation").and_then(|v| v.as_str()).unwrap_or("list");
    let guard = crate::cas_guard::global_cas_guard();

    match op {
        "protect" => {
            let Some(path_str) = params.get("path").and_then(|v| v.as_str()) else {
                return (false, json!({}), Some("'path' is required for operation='protect'".to_string()));
            };
            let strictly = params.get("strictly_guarded").and_then(|v| v.as_bool()).unwrap_or(true);
            match guard.protect_file(std::path::Path::new(path_str), strictly) {
                Ok(sha256) => (true, json!({ "status": "protected", "path": path_str, "sha256": sha256, "strictly_guarded": strictly }), None),
                Err(e) => (false, json!({}), Some(format!("Failed to snapshot '{}': {}", path_str, e))),
            }
        }
        "enforce" => {
            let Some(path_str) = params.get("path").and_then(|v| v.as_str()) else {
                return (false, json!({}), Some("'path' is required for operation='enforce'".to_string()));
            };
            let t0 = std::time::Instant::now();
            let verdict = guard.inspect_and_enforce(std::path::Path::new(path_str), false);
            let elapsed_us = t0.elapsed().as_micros() as u64;
            (true, json!({ "verdict": verdict, "evaluation_latency_us": elapsed_us }), None)
        }
        _ => {
            let files = guard.list_protected_files();
            (true, json!({ "protected_count": files.len(), "files": files }), None)
        }
    }
}

/// Encrypts a plaintext secret using the ChaCha20-Poly1305 AEAD Vault.
fn exec_encrypt_vault_secret(params: &Value) -> (bool, Value, Option<String>) {
    let Some(plaintext) = params.get("plaintext").and_then(|v| v.as_str()) else {
        return (false, json!({}), Some("'plaintext' parameter is required".to_string()));
    };
    let vault = crate::security_hardening::EncryptedSecretVault::load_default();
    match vault.encrypt(plaintext) {
        Ok(enc) => (true, json!({ "cipher": "ChaCha20-Poly1305 (AEAD)", "encrypted_value": enc }), None),
        Err(e) => (false, json!({}), Some(e)),
    }
}

/// Exports recent telemetry in Elastic Common Schema (ECS 8.11.0) NDJSON.
fn exec_export_siem_elk(state: &AgentInterfaceState, params: &Value) -> (bool, Value, Option<String>) {
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(25).clamp(1, 500) as usize;
    let fmt = params.get("format").and_then(|v| v.as_str()).unwrap_or("bulk");
    match state.db.get_recent_events(limit) {
        Ok(raw_events) => {
            let typed: Vec<TraceEvent> = raw_events.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect();
            let ndjson = if fmt == "stream" {
                crate::siem_exporter::to_ecs_ndjson_stream(&typed)
            } else {
                crate::siem_exporter::to_elasticsearch_bulk_ndjson(&typed, "agenttrace-security-events")
            };
            (true, json!({ "exported_events": typed.len(), "format": fmt, "ecs_version": "8.11.0", "ndjson": ndjson }), None)
        }
        Err(e) => (false, json!({}), Some(format!("Failed to query events for SIEM export: {}", e))),
    }
}
