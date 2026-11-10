//! OpenAI-compatible proxy (`POST /v1/chat/completions`, `GET /v1/models`).
//!
//! Every request is scrubbed for secrets *before* it is forwarded upstream and
//! before it is written to the ledger, then routed to a local model via the
//! delegation engine. Authentication is enforced by the router middleware.

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
};
use serde_json::Value;
use std::sync::Arc;

use crate::{
    db::Database,
    dlp::{self, DlpMode, GuardFinding},
    ledger::NodeIdentity,
    schema::{EventType, SecurityVerdict, TraceEvent},
    secrets::ClusterSecurityConfig,
};

#[derive(Clone)]
pub struct ProxyConfig {
    pub upstream_url: String,
    pub db: Database,
    pub security: Arc<ClusterSecurityConfig>,
    pub identity: Arc<NodeIdentity>,
    pub client: reqwest::Client,
    pub registry: Arc<tokio::sync::RwLock<crate::registry::ClusterModelRegistry>>,
    pub delegation_engine: Arc<crate::delegation_engine::AgentDelegationEngine>,
    pub ledger_writer: Option<crate::ledger_writer::LedgerWriter>,
    pub dlp_mode: DlpMode,
}

/// Result of scrubbing every message in a chat request.
pub struct ScrubResult {
    pub findings: Vec<GuardFinding>,
    /// Concatenated (already redacted) text of the last user-visible message, for logging/routing.
    pub last_prompt: String,
}

/// Redacts secrets in-place across all `messages[*].content` entries (string or
/// `[{type:"text", text}]` parts) and flags prompt-injection markers.
pub fn scrub_chat_request(req: &mut Value) -> ScrubResult {
    let mut findings = Vec::new();
    let mut last_prompt = String::new();

    let scrub = |text: &str, findings: &mut Vec<GuardFinding>| -> String {
        findings.extend(dlp::scan_prompt_injection(text));
        let (f, clean) = dlp::scan_and_redact_secrets(text);
        findings.extend(f);
        clean
    };

    if let Some(messages) = req.get_mut("messages").and_then(|m| m.as_array_mut()) {
        for msg in messages.iter_mut() {
            let Some(content) = msg.get_mut("content") else { continue };
            match content {
                Value::String(s) => {
                    let clean = scrub(s, &mut findings);
                    last_prompt = clean.clone();
                    *s = clean;
                }
                Value::Array(parts) => {
                    let mut joined = Vec::new();
                    for part in parts.iter_mut() {
                        if let Some(Value::String(t)) = part.get_mut("text") {
                            let clean = scrub(t, &mut findings);
                            joined.push(clean.clone());
                            *t = clean;
                        }
                    }
                    last_prompt = joined.join("\n");
                }
                _ => {}
            }
        }
    }

    ScrubResult { findings, last_prompt }
}

fn header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_string)
}

fn record_event(cfg: &ProxyConfig, mut ev: TraceEvent) {
    if let Some(ref writer) = cfg.ledger_writer {
        let _ = writer.try_enqueue(ev);
    } else {
        let db = cfg.db.clone();
        let identity = cfg.identity.clone();
        tokio::task::spawn_blocking(move || {
            let _ = db.insert_event_signed(&mut ev, Some(&*identity));
        });
    }
}

fn new_event(node_id: String, session_id: String, target: String, details: Value, verdict: SecurityVerdict, reasons: Vec<String>, duration_ms: f64) -> TraceEvent {
    TraceEvent {
        event_id: uuid::Uuid::new_v4().simple().to_string(),
        node_id,
        session_id,
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_type: EventType::LlmInference,
        target,
        action_details: details,
        diff: None,
        provenance_sources: vec![],
        security_verdict: verdict,
        security_reasons: reasons,
        duration_ms,
        prev_hash: String::new(),
        event_hash: String::new(),
        signature: None,
        public_key: None,
    }
}

pub async fn handle_chat_completions(
    State(cfg): State<Arc<ProxyConfig>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, StatusCode> {
    let mut req_json: Value = serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
    let model_name = req_json.get("model").and_then(|m| m.as_str()).unwrap_or("auto").to_string();
    let is_stream = req_json.get("stream").and_then(|s| s.as_bool()).unwrap_or(false);
    let session_id = header_str(&headers, "x-session-id")
        .or_else(|| req_json.get("session_id").and_then(|s| s.as_str()).map(str::to_string))
        .unwrap_or_else(|| "default".to_string());
    let agent_id = header_str(&headers, "x-agent-id");

    // 1. DLP: scrub a copy; only forward the original when DLP is explicitly off.
    let original = req_json.clone();
    let scrub = scrub_chat_request(&mut req_json);
    let mode = match header_str(&headers, "x-guard-mode").as_deref() {
        Some(m) if m.eq_ignore_ascii_case("hard") || m.eq_ignore_ascii_case("block") => DlpMode::Block,
        _ => cfg.dlp_mode,
    };
    let reasons: Vec<String> = scrub.findings.iter().map(|f| format!("[{}] {}", f.rule_id, f.description)).collect();
    let node_label = agent_id.clone().unwrap_or_else(|| "proxy".to_string());

    if mode == DlpMode::Block && !scrub.findings.is_empty() {
        record_event(&cfg, new_event(
            node_label,
            session_id,
            format!("DLP block [{}]", model_name),
            serde_json::json!({ "status": 403, "model": model_name, "prompt": scrub.last_prompt, "findings": scrub.findings }),
            SecurityVerdict::Blocked,
            reasons,
            0.0,
        ));
        return Err(StatusCode::FORBIDDEN);
    }
    if mode == DlpMode::Off {
        req_json = original;
    }

    // 2. Routing: explicit upstream header (allowlisted) or delegation engine.
    let (upstream_base, node_id, effective_model, routing) = if let Some(custom) = header_str(&headers, "x-upstream-url") {
        let nid = header_str(&headers, "x-node-id").unwrap_or_else(|| "CUSTOM".to_string());
        (custom.trim_end_matches('/').to_string(), nid, model_name.clone(), None)
    } else {
        let tags: Vec<String> = header_str(&headers, "x-tags")
            .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        let decision = cfg.delegation_engine.decide(&crate::delegation_engine::AgentDelegationRequest {
            prompt: scrub.last_prompt.clone(),
            session_id: Some(session_id.clone()),
            agent_id: agent_id.clone(),
            tags,
            requested_model: Some(model_name.clone()),
        }).await;
        (decision.target_upstream.clone(), decision.node_id.clone(), decision.selected_model.clone(), Some(decision))
    };

    if !cfg.security.is_upstream_allowed(&upstream_base) {
        tracing::warn!("upstream not in allowlist: {}", upstream_base);
        return Err(StatusCode::FORBIDDEN);
    }
    if effective_model != model_name {
        req_json["model"] = Value::String(effective_model.clone());
    }

    // 3. Forward. Client Authorization is passed through; otherwise attach the
    //    worker key for non-default (remote worker) upstreams.
    let upstream = format!("{}/v1/chat/completions", upstream_base);
    let mut req = cfg.client.post(&upstream).header("content-type", "application/json");
    if let Some(auth) = headers.get("authorization") {
        req = req.header("authorization", auth);
    } else if let Some(ref key) = cfg.security.worker_hermes_key {
        if upstream_base != cfg.upstream_url.trim_end_matches('/') {
            req = req.header("authorization", format!("Bearer {}", key));
        }
    }

    let t0 = std::time::Instant::now();
    let forward_body = serde_json::to_vec(&req_json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let upstream_res = req.body(forward_body).send().await.map_err(|e| {
        tracing::warn!("upstream error ({}): {}", upstream, e);
        if e.is_timeout() { StatusCode::GATEWAY_TIMEOUT } else { StatusCode::BAD_GATEWAY }
    })?;
    let status = StatusCode::from_u16(upstream_res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let verdict = if scrub.findings.is_empty() { SecurityVerdict::Allowed } else { SecurityVerdict::Flagged };

    let mut builder = Response::builder().status(status).header("x-routed-model", &effective_model);
    if let Some(ref dec) = routing {
        builder = builder
            .header("x-routed-intent", format!("{:?}", dec.intent))
            .header("x-routed-node", &dec.node_id)
            .header("x-decision-source", format!("{:?}", dec.decision_source));
    }
    let target = format!("{} [{}]", upstream, effective_model);

    if is_stream {
        let content_type = upstream_res.headers().get("content-type")
            .and_then(|v| v.to_str().ok()).unwrap_or("text/event-stream").to_string();
        record_event(&cfg, new_event(node_id, session_id, target, serde_json::json!({
            "status": status.as_u16(),
            "model": effective_model,
            "requested_model": model_name,
            "stream": true,
            "prompt": scrub.last_prompt,
            "dlp_findings": scrub.findings,
            "routing": routing,
        }), verdict, reasons, t0.elapsed().as_secs_f64() * 1000.0));
        return builder
            .header("content-type", content_type)
            .body(Body::from_stream(upstream_res.bytes_stream()))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    }

    let resp_bytes = upstream_res.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let duration_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let resp_json: Option<Value> = serde_json::from_slice(&resp_bytes).ok();
    let completion = resp_json.as_ref()
        .and_then(|j| j.pointer("/choices/0/message/content"))
        .and_then(|c| c.as_str())
        .map(|c| dlp::scan_and_redact_secrets(c).1)
        .unwrap_or_default();
    let usage = resp_json.as_ref().and_then(|j| j.get("usage")).cloned();

    record_event(&cfg, new_event(node_id, session_id, target, serde_json::json!({
        "status": status.as_u16(),
        "model": effective_model,
        "requested_model": model_name,
        "stream": false,
        "prompt": scrub.last_prompt,
        "completion": completion,
        "usage": usage,
        "dlp_findings": scrub.findings,
        "routing": routing,
    }), verdict, reasons, duration_ms));

    builder
        .header("content-type", "application/json")
        .body(Body::from(resp_bytes))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// `GET /v1/models` — the virtual `auto` router plus every model in the live registry.
pub async fn handle_models(State(cfg): State<Arc<ProxyConfig>>) -> axum::Json<Value> {
    let mut data = vec![serde_json::json!({
        "id": "auto",
        "object": "model",
        "owned_by": "agenttrace-router",
        "node_id": "CLUSTER",
    })];
    for m in cfg.registry.read().await.all_active_models() {
        data.push(serde_json::json!({
            "id": m.model_id,
            "object": "model",
            "owned_by": m.node_id,
            "node_id": m.node_id,
            "endpoint": m.endpoint,
            "capabilities": m.capabilities,
            "healthy": m.is_healthy,
        }));
    }
    axum::Json(serde_json::json!({ "object": "list", "data": data }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrubs_every_message_not_just_the_last() {
        let mut req = serde_json::json!({
            "model": "auto",
            "messages": [
                { "role": "system", "content": "env: AWS key AKIAIOSFODNN7EXAMPLE" },
                { "role": "user", "content": [{ "type": "text", "text": "db postgres://u:hunter2@h/x" }] },
                { "role": "user", "content": "hello" }
            ]
        });
        let res = scrub_chat_request(&mut req);
        let out = req.to_string();
        assert!(!out.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(!out.contains("hunter2"));
        assert_eq!(res.findings.len(), 2);
        assert_eq!(res.last_prompt, "hello");
    }
}
