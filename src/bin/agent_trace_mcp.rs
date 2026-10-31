//! Standalone Native Rust Model Context Protocol (MCP) Server for AgentTrace.
//!
//! Provides stdio JSON-RPC 2.0 communication for AI coding agents (Antigravity,
//! Claude Desktop, Cursor, Windsurf, Copilot CLI) to interact natively with
//! the local AgentTrace Gateway daemon.
//!
//! "Here's a tool — let your agent read it, he will thank you later."

use std::env;
use std::io::{self, Write};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gw_url = env::var("AGENTTRACE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string())
        .trim_end_matches('/')
        .to_string();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    eprintln!("[agent_trace_mcp] Started native Rust MCP server stdio bridge -> {}", gw_url);

    while let Ok(Some(line)) = reader.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let rpc_req: Value = match serde_json::from_str(line) {
            Ok(val) => val,
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": {
                        "code": -32700,
                        "message": format!("Parse error: {}", e)
                    }
                });
                send_response(&err_resp);
                continue;
            }
        };

        let method = rpc_req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = rpc_req.get("id").cloned().unwrap_or(Value::Null);

        // JSON-RPC Notifications do not require a response
        if method.starts_with("notifications/") || (method.contains('/') && id.is_null() && !method.starts_with("tools/")) {
            eprintln!("[agent_trace_mcp] Received notification: {}", method);
            continue;
        }

        // Fast-path for initialize handshake
        if method == "initialize" {
            let init_resp = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {
                        "name": "agent-trace-mcp",
                        "version": "0.1.0"
                    },
                    "instructions": "AgentTrace Headless Cluster & Project Book MCP Server. Use agenttrace tools to dispatch tasks to local PC-1/PC-2 clusters, verify SHA-256 cryptographic ledgers, retrieve active governance rules, and record architectural decisions into the living Studio Project Book.",
                    "capabilities": {
                        "tools": {}
                    }
                }
            });
            send_response(&init_resp);
            continue;
        }

        // Forward MCP request to Gateway HTTP endpoint
        let target_endpoint = format!("{}/api/v1/mcp", gw_url);
        let mut req_builder = client.post(&target_endpoint)
            .header("Content-Type", "application/json")
            .json(&rpc_req);
        // Forward the gateway token when AGENTTRACE_REQUIRE_AUTH is enabled on the gateway
        if let Some(tok) = env::var("AGENTTRACE_TOKEN").ok().filter(|t| !t.trim().is_empty()) {
            req_builder = req_builder.header("X-AgentTrace-Token", tok);
        }
        match req_builder.send().await
        {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.json::<Value>().await {
                        Ok(json_body) => send_response(&json_body),
                        Err(e) => {
                            let err_resp = json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": {
                                    "code": -32603,
                                    "message": format!("Invalid JSON from AgentTrace Gateway: {}", e)
                                }
                            });
                            send_response(&err_resp);
                        }
                    }
                } else {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    let err_resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32000,
                            "message": format!("AgentTrace Gateway returned HTTP {}: {}", status, text)
                        }
                    });
                    send_response(&err_resp);
                }
            }
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32000,
                        "message": format!("Failed to reach AgentTrace Gateway at {}: {}. Ensure 'agent_trace_gw' daemon is running.", target_endpoint, e)
                    }
                });
                send_response(&err_resp);
            }
        }
    }

    Ok(())
}

fn send_response(val: &Value) {
    if let Ok(serialized) = serde_json::to_string(val) {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(handle, "{}", serialized);
        let _ = handle.flush();
    }
}
