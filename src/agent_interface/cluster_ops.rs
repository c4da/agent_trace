//! Cluster Operations, Project Book & Workgroup Tool Executors (`src/agent_interface/cluster_ops.rs`).

use serde_json::{json, Value};
use crate::{
    agent_interface::AgentInterfaceState,
    schema::{EventType, SecurityVerdict, TraceEvent},
};

/// Records an architectural decision / milestone / learning summary in the ledger & project book.
pub async fn exec_record_project_decision(
    state: &AgentInterfaceState,
    agent_id: &str,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let title = params.get("title").and_then(|v| v.as_str()).unwrap_or("Architectural Decision");
    let rationale = params.get("rationale").and_then(|v| v.as_str()).unwrap_or("");
    let tags = params.get("tags").cloned().unwrap_or_else(|| json!(["#architecture"]));
    let lessons = params.get("lessons_learned").and_then(|v| v.as_str()).unwrap_or("");
    let rules_verified = params.get("rules_verified").cloned().unwrap_or_else(|| json!([]));
    let diff = params.get("diff").and_then(|v| v.as_str()).map(|s| s.to_string());

    let timestamp = chrono::Utc::now().to_rfc3339();
    let event_id = format!("dec-{}", &uuid::Uuid::new_v4().to_string()[..8]);

    let action_details = json!({
        "title": title,
        "rationale": rationale,
        "tags": tags,
        "lessons_learned": lessons,
        "rules_verified": rules_verified,
        "recorded_by": agent_id,
    });

    let mut event = TraceEvent {
        event_id: event_id.clone(),
        node_id: "CHRONICLE".to_string(),
        session_id: "project-book".to_string(),
        timestamp: timestamp.clone(),
        event_type: EventType::DecisionStep,
        target: title.to_string(),
        action_details,
        diff,
        provenance_sources: vec![format!("agent:{}", agent_id)],
        security_verdict: SecurityVerdict::Allowed,
        security_reasons: vec![],
        duration_ms: 0.0,
        prev_hash: "0".repeat(64),
        event_hash: "0".repeat(64),
        signature: None,
        public_key: None,
    };

    match state.db.insert_event_signed(&mut event, Some(&*state.identity)) {
        Ok(_) => (
            true,
            json!({
                "status": "committed_to_ledger",
                "event_id": event.event_id,
                "event_hash": event.event_hash,
                "title": title,
                "timestamp": timestamp,
            }),
            None,
        ),
        Err(e) => (false, json!({}), Some(format!("Failed to record decision to ledger: {}", e))),
    }
}

/// Dispatches a prompt to the cluster through the gateway's own
/// `/v1/chat/completions` (DLP scrubbing, smart routing and ledger logging apply).
pub async fn exec_dispatch_cluster_prompt(
    state: &AgentInterfaceState,
    agent_id: &str,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let prompt = match params.get("prompt").and_then(|v| v.as_str()) {
        Some(p) if !p.trim().is_empty() => p,
        _ => return (false, json!({}), Some("Missing required parameter: 'prompt'".to_string())),
    };
    let model = params.get("requested_model").and_then(|v| v.as_str()).unwrap_or("auto");

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let url = format!("http://127.0.0.1:{}/v1/chat/completions", port);
    let payload = json!({
        "model": model,
        "stream": false,
        "messages": [{ "role": "user", "content": prompt }],
    });

    let mut req = reqwest::Client::new()
        .post(&url)
        .header("x-agent-id", agent_id)
        .json(&payload);
    if let Some(ref tok) = state.security.auth_token {
        req = req.header("x-agenttrace-token", tok);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => return (false, json!({}), Some(format!("Failed to reach gateway at {}: {}", url, e))),
    };
    let status = resp.status();
    let routed_model = resp.headers().get("x-routed-model").and_then(|v| v.to_str().ok()).map(str::to_string);
    let routed_node = resp.headers().get("x-routed-node").and_then(|v| v.to_str().ok()).map(str::to_string);
    if !status.is_success() {
        let txt = resp.text().await.unwrap_or_default();
        return (false, json!({}), Some(format!("Gateway returned HTTP {}: {}", status, txt)));
    }
    match resp.json::<Value>().await {
        Ok(body) => {
            let content = body.pointer("/choices/0/message/content").and_then(|c| c.as_str()).unwrap_or("").to_string();
            (true, json!({
                "primary_response": content,
                "model": routed_model,
                "node": routed_node,
                "usage": body.get("usage"),
            }), None)
        }
        Err(e) => (false, json!({}), Some(format!("Invalid response JSON from upstream: {}", e))),
    }
}
