//! Model Context Protocol (MCP) JSON-RPC 2.0 handler.
//!
//! Enables direct integration with Claude Code, Cursor, Windsurf, and custom MCP clients.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};

use super::{
    executor::handle_agent_action,
    schemas::get_agent_tools,
    types::{AgentActionRequest, AgentInterfaceState},
};

/// Handles JSON-RPC 2.0 requests following MCP specifications.
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
        "initialize" => respond_mcp_init(id),
        "tools/list" => respond_mcp_tools(id).await,
        "tools/call" => call_mcp_tool(state, headers, id, &rpc_req).await,
        _ => respond_mcp_not_found(id, method),
    }
}

/// Returns server handshake and capabilities.
fn respond_mcp_init(id: Value) -> Response {
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

/// Lists all registered tools in MCP format.
async fn respond_mcp_tools(id: Value) -> Response {
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

/// Executes requested MCP tool and formats response.
async fn call_mcp_tool(
    state: AgentInterfaceState,
    headers: HeaderMap,
    id: Value,
    rpc_req: &Value,
) -> Response {
    let params = rpc_req.get("params").cloned().unwrap_or(json!({}));
    let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    let action_req = AgentActionRequest {
        agent_id: "mcp-client".to_string(),
        tool: name.to_string(),
        parameters: arguments,
    };

    match handle_agent_action(State(state), headers, Json(action_req)).await {
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

/// Returns standard method not found error.
fn respond_mcp_not_found(id: Value, method: &str) -> Response {
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
