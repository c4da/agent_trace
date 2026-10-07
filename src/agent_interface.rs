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
