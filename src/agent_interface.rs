//! Agent-Centric Telemetry Control Interface for AgentTrace Gateway.
//!
//! Exposes machine-readable, schema-defined endpoints and an MCP-compatible JSON-RPC
//! interface for external AI agents (e.g. Hermes, Claude Code, autonomous watchdogs)
//! to inspect, audit, verify, and govern the telemetry ledger and cluster routing.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
    db::Database,
    delegation_engine::{AgentDelegationEngine, AgentDelegationRequest, OverrideCriteria, RouteOverrideRule, RouteOverrideStore},
    ledger::NodeIdentity,
    registry::ClusterModelRegistry,
    router::classify_and_route_dynamic,
    schema::SecurityVerdict,
    secrets::ClusterSecurityConfig,
};

/// Shared state required by the agent interface.
#[derive(Clone)]
pub struct AgentInterfaceState {
    pub db: Database,
    pub registry: Arc<RwLock<ClusterModelRegistry>>,
    pub security: Arc<ClusterSecurityConfig>,
    pub identity: Arc<NodeIdentity>,
    pub upstream_url: String,
    pub override_store: Arc<RwLock<RouteOverrideStore>>,
    pub delegation_engine: Arc<AgentDelegationEngine>,
    pub policy: Arc<RwLock<crate::policy::PolicyEngine>>,
}

/// Request payload for an agent executing an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentActionRequest {
    /// Identity of the calling AI agent (e.g. "Hermes-Watchdog", "Claude-Code", "Security-Auditor").
    pub agent_id: String,
    /// Tool name to execute.
    pub tool: String,
    /// Arguments for the tool.
    #[serde(default)]
    pub parameters: Value,
}

/// Standardized execution result returned to the calling agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentActionResponse {
    pub success: bool,
    pub tool: String,
    pub agent_id: String,
    pub result: Value,
    pub execution_ms: f64,
    pub error: Option<String>,
}

/// Returns the OpenAPI / JSON function definitions of all tools available to AI agents.
pub async fn get_agent_tools() -> Json<Value> {
    let tools = json!({
        "schema_version": "1.0.0",
        "protocol": "AgentTrace-Agent-Interface",
        "description": "Autonomous governance, telemetry auditing, and cluster management tools for AI agents.",
        "tools": [
            {
                "name": "verify_ledger",
                "description": "Cryptographically verifies the entire SHA-256 hash-chain and Ed25519 signatures of the telemetry ledger. Returns block count, chain validity, head hash, and broken block details if tampered.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "query_telemetry",
                "description": "Searches and filters recorded agent actions (file reads, writes, commands, LLM calls) with optional target substring, event type, or limit.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "target_contains": {
                            "type": "string",
                            "description": "Optional substring to filter targets (e.g. '.env', 'rm -rf', 'config.yaml')."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of events to return (default 50, max 200).",
                            "default": 50
                        }
                    }
                }
            },
            {
                "name": "inspect_cluster",
                "description": "Returns current cluster topology, all self-announced models across nodes, parameter tiers, and health status.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "flag_incident",
                "description": "Allows an AI agent auditor to flag a suspicious telemetry event or security policy violation in the ledger.",
                "parameters": {
                    "type": "object",
                    "required": ["event_id", "reason"],
                    "properties": {
                        "event_id": {
                            "type": "string",
                            "description": "The unique event ID of the block being flagged."
                        },
                        "reason": {
                            "type": "string",
                            "description": "Reasoning explaining why this event was flagged as a violation or anomaly."
                        }
                    }
                }
            },
            {
                "name": "explain_route",
                "description": "Tests which cluster model and node would be selected for a given prompt without executing the inference.",
                "parameters": {
                    "type": "object",
                    "required": ["prompt"],
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "The user prompt or instruction to classify and route."
                        },
                        "requested_model": {
                            "type": "string",
                            "description": "Optional model override (e.g. 'auto', 'qwen3.8:27b'). Defaults to 'auto'.",
                            "default": "auto"
                        }
                    }
                }
            },
            {
                "name": "decide_route",
                "description": "Arbitrates and determines optimal prompt routing across the cluster, taking into account model capabilities, node availability, and active agent override rules.",
                "parameters": {
                    "type": "object",
                    "required": ["prompt"],
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "The user prompt or instruction to arbitrate and route."
                        },
                        "session_id": {
                            "type": "string",
                            "description": "Optional session ID to match against session override rules."
                        },
                        "agent_id": {
                            "type": "string",
                            "description": "Optional agent ID to match against agent override rules."
                        },
                        "requested_model": {
                            "type": "string",
                            "description": "Optional model requested (e.g. 'auto', 'qwen3.8:27b'). Defaults to 'auto'."
                        },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional classification or context tags."
                        }
                    }
                }
            },
            {
                "name": "set_route_override",
                "description": "Registers an autonomous routing override rule into the gateway so subsequent prompts matching session, agent, or tag criteria are forced to a specific model or node.",
                "parameters": {
                    "type": "object",
                    "required": ["criteria_type", "criteria_value", "target_model", "reason"],
                    "properties": {
                        "rule_id": {
                            "type": "string",
                            "description": "Optional unique rule ID. Auto-generated if omitted."
                        },
                        "criteria_type": {
                            "type": "string",
                            "description": "Type of criteria to match: 'session', 'agent', 'tag', 'model', or 'prompt_pattern'.",
                            "enum": ["session", "agent", "tag", "model", "prompt_pattern"]
                        },
                        "criteria_value": {
                            "type": "string",
                            "description": "Value to match (e.g. session ID 'sec-101', agent ID 'hermes-coder', model 'gemini*', prompt pattern '*test*')."
                        },
                        "target_model": {
                            "type": "string",
                            "description": "Target model to force routing to (e.g. 'qwen3.8:27b', 'deepseek-r1:1.5b')."
                        },
                        "target_node": {
                            "type": "string",
                            "description": "Optional target cluster node ID (e.g. 'PC-1', 'PC-2')."
                        },
                        "reason": {
                            "type": "string",
                            "description": "Justification for this routing override rule."
                        }
                    }
                }
            },
            {
                "name": "list_route_overrides",
                "description": "Lists all currently active agent routing overrides in the gateway.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "remove_route_override",
                "description": "Removes an active routing override rule by its rule ID.",
                "parameters": {
                    "type": "object",
                    "required": ["rule_id"],
                    "properties": {
                        "rule_id": {
                            "type": "string",
                            "description": "The unique rule ID of the override to remove."
                        }
                    }
                }
            },
            {
                "name": "analyze_telemetry",
                "description": "Computes multi-dimensional telemetry analytics across trace events in the ledger, returning target hotspots, agent behavioral profiles, activity time-series, and security anomalies.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "get_access_policies",
                "description": "Lists all active agent sandbox and whitelist policies governing allowed read, write, and network pull paths.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "set_access_policy",
                "description": "Creates or updates an access policy restricting or permitting where a specific agent can read, write, or pull data.",
                "parameters": {
                    "type": "object",
                    "required": ["agent_id"],
                    "properties": {
                        "agent_id": {
                            "type": "string",
                            "description": "Agent identifier (e.g. 'hermes-agent', 'claude-code', or '*' for global baseline)."
                        },
                        "read_whitelist": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of allowed path prefixes/patterns for reading data."
                        },
                        "write_whitelist": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of allowed path prefixes/patterns for writing data."
                        },
                        "network_whitelist": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of allowed domain or URL prefixes for outbound network requests."
                        },
                        "blocked_patterns": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of forbidden sensitive substrings/patterns (e.g. '.env', 'id_rsa')."
                        },
                        "enforcing": {
                            "type": "boolean",
                            "description": "Whether violations are strictly Blocked (true) or Flagged for audit only (false). Defaults to true."
                        }
                    }
                }
            },
            {
                "name": "evaluate_access_policy",
                "description": "Evaluates whether a proposed data read/write or network pull is permitted under current agent sandbox rules without executing it.",
                "parameters": {
                    "type": "object",
                    "required": ["agent_id", "action", "target"],
                    "properties": {
                        "agent_id": {
                            "type": "string",
                            "description": "Agent ID proposing the action."
                        },
                        "action": {
                            "type": "string",
                            "enum": ["Read", "Write", "Execute", "NetworkPull"],
                            "description": "Type of access action."
                        },
                        "target": {
                            "type": "string",
                            "description": "The file path or URL destination to evaluate."
                        }
                    }
                }
            },
            {
                "name": "get_delegation_config",
                "description": "Retrieves the current cluster delegation and zero-token economy enforcement settings.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "set_delegation_config",
                "description": "Updates cluster delegation and zero-token economy enforcement settings.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "intercept_external_models": {
                            "type": "boolean",
                            "description": "Whether to intercept external cloud models (gemini*, gpt*, claude*) and divert them to the local cluster."
                        },
                        "zero_token_mode": {
                            "type": "boolean",
                            "description": "Whether strict zero-token economy is actively enforced."
                        },
                        "auto_switch_offline_models": {
                            "type": "boolean",
                            "description": "Whether to auto-switch unavailable models to active downloaded cluster candidates."
                        },
                        "default_fallback_model": {
                            "type": "string",
                            "description": "Default fallback model ID."
                        }
                    }
                }
            }
        ]
    });

    Json(tools)
}

/// Executes an action requested by an AI agent.
pub async fn handle_agent_action(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
    Json(req): Json<AgentActionRequest>,
) -> Result<Json<AgentActionResponse>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let t0 = std::time::Instant::now();

    let (success, result, err_msg) = match req.tool.as_str() {
        "verify_ledger" => {
            match state.db.verify_ledger() {
                Ok(report) => (true, serde_json::to_value(report).unwrap_or(json!({})), None),
                Err(e) => (false, json!({}), Some(format!("Ledger verification error: {}", e))),
            }
        }
        "query_telemetry" => {
            let limit = req.parameters
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(50)
                .clamp(1, 200) as usize;
            let target_sub = req.parameters
                .get("target_contains")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase());

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
        "inspect_cluster" => {
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
        "flag_incident" => {
            let event_id = req.parameters.get("event_id").and_then(|v| v.as_str()).unwrap_or("");
            let reason = req.parameters.get("reason").and_then(|v| v.as_str()).unwrap_or("");

            if event_id.is_empty() || reason.is_empty() {
                (false, json!({}), Some("Both 'event_id' and 'reason' are required".to_string()))
            } else {
                // Insert a governance audit event into the ledger
                let now = chrono::Utc::now().to_rfc3339();
                let mut incident_event = crate::schema::TraceEvent {
                    event_id: uuid::Uuid::new_v4().to_string(),
                    node_id: "GOVERNANCE".to_string(),
                    session_id: "agent_governance".to_string(),
                    timestamp: now,
                    event_type: crate::schema::EventType::DecisionStep,
                    target: format!("incident_flag:{}", event_id),
                    action_details: json!({
                        "flagged_by_agent": req.agent_id,
                        "target_event_id": event_id,
                        "justification": reason
                    }),
                    diff: None,
                    provenance_sources: vec![req.agent_id.clone()],
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
        }
        "explain_route" => {
            let prompt = req.parameters.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
            let requested_model = req.parameters.get("requested_model").and_then(|v| v.as_str()).unwrap_or("auto");

            let reg = state.registry.read().await;
            let decision = classify_and_route_dynamic(
                prompt,
                requested_model,
                false,
                &*reg,
                &state.upstream_url,
            );

            (true, serde_json::to_value(decision).unwrap_or(json!({})), None)
        }
        "decide_route" => {
            let prompt = req.parameters.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
            let session_id = req.parameters.get("session_id").and_then(|v| v.as_str()).map(|s| s.to_string());
            let agent_id = req.parameters.get("agent_id").and_then(|v| v.as_str()).map(|s| s.to_string());
            let requested_model = req.parameters.get("requested_model").and_then(|v| v.as_str()).map(|s| s.to_string());
            let tags = req.parameters.get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();

            let delegation_req = AgentDelegationRequest {
                prompt: prompt.to_string(),
                session_id,
                agent_id,
                tags,
                requested_model,
            };

            let decision = state.delegation_engine.decide(&delegation_req).await;
            (true, serde_json::to_value(decision).unwrap_or(json!({})), None)
        }
        "set_route_override" => {
            let criteria_type = req.parameters.get("criteria_type").and_then(|v| v.as_str()).unwrap_or("session");
            let criteria_value = req.parameters.get("criteria_value").and_then(|v| v.as_str()).unwrap_or("");
            let target_model = req.parameters.get("target_model").and_then(|v| v.as_str()).unwrap_or("");
            let target_node = req.parameters.get("target_node").and_then(|v| v.as_str()).map(|s| s.to_string());
            let reason = req.parameters.get("reason").and_then(|v| v.as_str()).unwrap_or("Agent specified route override");
            let rule_id = req.parameters.get("rule_id").and_then(|v| v.as_str()).unwrap_or("");

            if criteria_value.is_empty() || target_model.is_empty() {
                (false, json!({}), Some("Both 'criteria_value' and 'target_model' are required".to_string()))
            } else {
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
                    store.set_rule(rule.clone())
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
        }
        "list_route_overrides" => {
            let store = state.override_store.read().await;
            let rules = store.list_rules();
            (true, json!({ "count": rules.len(), "rules": rules }), None)
        }
        "remove_route_override" => {
            let rule_id = req.parameters.get("rule_id").and_then(|v| v.as_str()).unwrap_or("");
            if rule_id.is_empty() {
                (false, json!({}), Some("'rule_id' parameter is required".to_string()))
            } else {
                let mut store = state.override_store.write().await;
                let removed = store.remove_rule(rule_id);
                (true, json!({ "rule_id": rule_id, "removed": removed }), None)
            }
        }
        "analyze_telemetry" => {
            match state.db.get_telemetry_analytics() {
                Ok(report) => (true, serde_json::to_value(report).unwrap_or(json!({})), None),
                Err(e) => (false, json!({}), Some(format!("Analytics computation failed: {}", e))),
            }
        }
        "get_access_policies" => {
            let guard = state.policy.read().await;
            let policies = guard.list_policies();
            (true, json!({ "count": policies.len(), "policies": policies }), None)
        }
        "set_access_policy" => {
            let agent_id = req.parameters.get("agent_id").and_then(|v| v.as_str()).unwrap_or("");
            if agent_id.is_empty() {
                (false, json!({}), Some("'agent_id' parameter is required".to_string()))
            } else {
                let read_whitelist = req.parameters.get("read_whitelist")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|s| s.as_str().map(|x| x.to_string())).collect())
                    .unwrap_or_default();
                let write_whitelist = req.parameters.get("write_whitelist")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|s| s.as_str().map(|x| x.to_string())).collect())
                    .unwrap_or_default();
                let network_whitelist = req.parameters.get("network_whitelist")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|s| s.as_str().map(|x| x.to_string())).collect())
                    .unwrap_or_default();
                let blocked_patterns = req.parameters.get("blocked_patterns")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|s| s.as_str().map(|x| x.to_string())).collect())
                    .unwrap_or_default();
                let enforcing = req.parameters.get("enforcing").and_then(|v| v.as_bool()).unwrap_or(true);

                let policy = crate::policy::AgentAccessPolicy {
                    agent_id: agent_id.to_string(),
                    read_whitelist,
                    write_whitelist,
                    network_whitelist,
                    blocked_patterns,
                    enforcing,
                };

                {
                    let mut guard = state.policy.write().await;
                    guard.set_policy(policy.clone());
                }

                (true, serde_json::to_value(policy).unwrap_or(json!({})), None)
            }
        }
        "evaluate_access_policy" => {
            let agent_id = req.parameters.get("agent_id").and_then(|v| v.as_str()).unwrap_or("*");
            let action_str = req.parameters.get("action").and_then(|v| v.as_str()).unwrap_or("Read");
            let target = req.parameters.get("target").and_then(|v| v.as_str()).unwrap_or("");

            let action = match action_str.to_lowercase().as_str() {
                "write" => crate::policy::AccessAction::Write,
                "execute" => crate::policy::AccessAction::Execute,
                "networkpull" | "network" => crate::policy::AccessAction::NetworkPull,
                _ => crate::policy::AccessAction::Read,
            };

            let guard = state.policy.read().await;
            let decision = guard.evaluate(agent_id, action, target);
            (true, serde_json::to_value(decision).unwrap_or(json!({})), None)
        }
        "get_delegation_config" => {
            let cfg = state.delegation_engine.get_config().await;
            (true, serde_json::to_value(cfg).unwrap_or(json!({})), None)
        }
        "set_delegation_config" => {
            let mut current = state.delegation_engine.get_config().await;
            if let Some(v) = req.parameters.get("intercept_external_models").and_then(|v| v.as_bool()) {
                current.intercept_external_models = v;
            }
            if let Some(v) = req.parameters.get("zero_token_mode").and_then(|v| v.as_bool()) {
                current.zero_token_mode = v;
            }
            if let Some(v) = req.parameters.get("auto_switch_offline_models").and_then(|v| v.as_bool()) {
                current.auto_switch_offline_models = v;
            }
            if let Some(v) = req.parameters.get("default_fallback_model").and_then(|v| v.as_str()) {
                current.default_fallback_model = v.to_string();
            }
            state.delegation_engine.set_config(current.clone()).await;
            (true, serde_json::to_value(current).unwrap_or(json!({})), None)
        }
        unknown => (false, json!({}), Some(format!("Unknown tool: '{}'", unknown))),
    };

    let execution_ms = t0.elapsed().as_secs_f64() * 1000.0;

    Ok(Json(AgentActionResponse {
        success,
        tool: req.tool,
        agent_id: req.agent_id,
        result,
        execution_ms,
        error: err_msg,
    }))
}

/// JSON-RPC 2.0 / MCP (Model Context Protocol) Endpoint.
/// Enables seamless integration with MCP clients (Claude Code, Cursor, Windsurf, custom agents).
pub async fn handle_mcp_rpc(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
    Json(rpc_req): Json<Value>,
) -> Response {
    if !state.security.is_authorized(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let id = rpc_req.get("id").cloned().unwrap_or(json!(null));
    let method = rpc_req.get("method").and_then(|m| m.as_str()).unwrap_or("");

    match method {
        "initialize" => {
            let resp = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {
                        "name": "agent-trace-mcp",
                        "version": "0.1.0"
                    },
                    "capabilities": {
                        "tools": {}
                    }
                }
            });
            Json(resp).into_response()
        }
        "tools/list" => {
            let tools_json = get_agent_tools().await.0;
            let tools_arr = tools_json.get("tools").cloned().unwrap_or(json!([]));
            let resp = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": tools_arr
                }
            });
            Json(resp).into_response()
        }
        "tools/call" => {
            let params = rpc_req.get("params").cloned().unwrap_or(json!({}));
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            let action_req = AgentActionRequest {
                agent_id: "mcp-client".to_string(),
                tool: name.to_string(),
                parameters: arguments,
            };

            let action_res = handle_agent_action(State(state), headers, Json(action_req)).await;
            match action_res {
                Ok(Json(res)) => {
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": serde_json::to_string_pretty(&res.result).unwrap_or_default()
                                }
                            ],
                            "isError": !res.success
                        }
                    });
                    Json(resp).into_response()
                }
                Err(status) => status.into_response(),
            }
        }
        _ => {
            let resp = json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {}", method)
                }
            });
            Json(resp).into_response()
        }
    }
}

/// Direct REST API endpoint for arbitrating a prompt or task routing decision.
pub async fn handle_delegate_decide(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
    Json(req): Json<AgentDelegationRequest>,
) -> Result<Json<crate::delegation_engine::AgentDelegationDecision>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let decision = state.delegation_engine.decide(&req).await;
    Ok(Json(decision))
}

/// Direct REST API endpoint to create/set a dynamic routing override rule.
pub async fn handle_delegate_override(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
    Json(rule): Json<RouteOverrideRule>,
) -> Result<Json<RouteOverrideRule>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut store = state.override_store.write().await;
    let _ = store.set_rule(rule.clone());
    Ok(Json(rule))
}

/// Direct REST API endpoint to list active dynamic routing overrides.
pub async fn handle_delegate_list_overrides(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let store = state.override_store.read().await;
    let rules = store.list_rules();
    Ok(Json(json!({ "count": rules.len(), "rules": rules })))
}

/// Direct REST API endpoint to remove a dynamic routing override rule by ID.
pub async fn handle_delegate_remove_override(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
    axum::extract::Path(rule_id): axum::extract::Path<String>,
) -> Result<Json<Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut store = state.override_store.write().await;
    let removed = store.remove_rule(&rule_id);
    Ok(Json(json!({ "rule_id": rule_id, "removed": removed })))
}

/// Direct REST API endpoint to retrieve delegation engine and token guard configuration.
pub async fn handle_delegate_get_config(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
) -> Result<Json<crate::delegation_engine::DelegationConfig>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let cfg = state.delegation_engine.get_config().await;
    Ok(Json(cfg))
}

/// Direct REST API endpoint to update delegation engine and token guard configuration.
pub async fn handle_delegate_set_config(
    State(state): State<AgentInterfaceState>,
    headers: HeaderMap,
    Json(cfg): Json<crate::delegation_engine::DelegationConfig>,
) -> Result<Json<crate::delegation_engine::DelegationConfig>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    state.delegation_engine.set_config(cfg.clone()).await;
    Ok(Json(cfg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_tools_registration() {
        let axum::Json(body) = get_agent_tools().await;
        let tools = body["tools"].as_array().expect("Expected tools array");
        assert!(tools.len() >= 7, "Expected at least 7 registered AI agent tools");

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

        for t in tools {
            assert!(t["description"].is_string());
            assert_eq!(t["parameters"]["type"], "object");
        }
    }
}


