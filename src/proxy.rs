use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
};
use std::sync::Arc;
use crate::{db::Database, schema::{EventType, SecurityVerdict, TraceEvent}};

#[derive(Clone)]
pub struct ProxyConfig {
    pub upstream_url: String,
    pub db: Database,
}

pub async fn handle_chat_completions(
    State(cfg): State<Arc<ProxyConfig>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, StatusCode> {
    let node_id = headers
        .get("x-node-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("PC-1")
        .to_string();

    // 1. Dynamic upstream resolution: use X-Upstream-Url header if provided, else default
    let upstream_base = headers
        .get("x-upstream-url")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(&cfg.upstream_url)
        .trim_end_matches('/')
        .to_string();

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let upstream = format!("{}/v1/chat/completions", upstream_base);

    // Parse request payload to extract prompt, model, and stream mode
    let req_json: Option<serde_json::Value> = serde_json::from_slice(&body).ok();
    let model_name = req_json.as_ref()
        .and_then(|j| j.get("model"))
        .and_then(|m| m.as_str())
        .unwrap_or("unknown")
        .to_string();

    let is_stream = req_json.as_ref()
        .and_then(|j| j.get("stream"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    let last_prompt = req_json.as_ref()
        .and_then(|j| j.get("messages"))
        .and_then(|msgs| msgs.as_array())
        .and_then(|arr| arr.last())
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    let t0 = std::time::Instant::now();

    // 2. Build upstream request and pass through Authorization header if present
    let mut req = client.post(&upstream).header("content-type", "application/json");
    if let Some(auth) = headers.get("authorization") {
        req = req.header("authorization", auth);
    }

    let upstream_res = match req.body(body.clone()).send().await {
        Ok(res) => res,
        Err(e) => {
            let status = if e.is_timeout() {
                StatusCode::GATEWAY_TIMEOUT
            } else {
                StatusCode::BAD_GATEWAY
            };
            eprintln!("[-] Error connecting to upstream LLM ({}): {}", upstream, e);
            return Err(status);
        }
    };

    let status = StatusCode::from_u16(upstream_res.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let db = cfg.db.clone();
    let nid = node_id.clone();
    let logged_upstream = upstream.clone();

    if is_stream {
        let duration_ms = t0.elapsed().as_secs_f64() * 1000.0;
        tokio::spawn(async move {
            let ev = TraceEvent {
                event_id: uuid::Uuid::new_v4().simple().to_string(),
                node_id: nid,
                session_id: "active_session".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                event_type: EventType::LlmInference,
                target: format!("{} [{}]", logged_upstream, model_name),
                action_details: serde_json::json!({
                    "status": status.as_u16(),
                    "duration_ms": duration_ms,
                    "model": model_name,
                    "stream": true,
                    "prompt": last_prompt,
                }),
                diff: None,
                provenance_sources: vec![],
                security_verdict: SecurityVerdict::Allowed,
                security_reasons: vec![],
                duration_ms,
                prev_hash: String::new(),
                event_hash: String::new(),
                signature: None,
                public_key: None,
            };
            let mut ev_mut = ev;
            let _ = db.insert_event(&mut ev_mut);
        });

        let stream = upstream_res.bytes_stream();
        let body = Body::from_stream(stream);

        Ok(Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(body)
            .unwrap())
    } else {
        let resp_bytes = match upstream_res.bytes().await {
            Ok(b) => b,
            Err(e) => {
                eprintln!("[-] Error buffering upstream response: {}", e);
                return Err(StatusCode::BAD_GATEWAY);
            }
        };

        let duration_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let resp_json: Option<serde_json::Value> = serde_json::from_slice(&resp_bytes).ok();
        let completion_text = resp_json.as_ref()
            .and_then(|j| j.get("choices"))
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        let usage = resp_json.as_ref().and_then(|j| j.get("usage")).cloned();

        tokio::spawn(async move {
            let ev = TraceEvent {
                event_id: uuid::Uuid::new_v4().simple().to_string(),
                node_id: nid,
                session_id: "active_session".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                event_type: EventType::LlmInference,
                target: format!("{} [{}]", logged_upstream, model_name),
                action_details: serde_json::json!({
                    "status": status.as_u16(),
                    "duration_ms": duration_ms,
                    "model": model_name,
                    "stream": false,
                    "prompt": last_prompt,
                    "completion": completion_text,
                    "usage": usage,
                }),
                diff: None,
                provenance_sources: vec![],
                security_verdict: SecurityVerdict::Allowed,
                security_reasons: vec![],
                duration_ms,
                prev_hash: String::new(),
                event_hash: String::new(),
                signature: None,
                public_key: None,
            };
            let mut ev_mut = ev;
            let _ = db.insert_event(&mut ev_mut);
        });

        Ok(Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(Body::from(resp_bytes))
            .unwrap())
    }
}

pub async fn handle_models(
    State(cfg): State<Arc<ProxyConfig>>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let upstream_base = headers
        .get("x-upstream-url")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(&cfg.upstream_url)
        .trim_end_matches('/')
        .to_string();

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let upstream = format!("{}/v1/models", upstream_base);

    let mut req = client.get(&upstream);
    if let Some(auth) = headers.get("authorization") {
        req = req.header("authorization", auth);
    }

    let upstream_res = match req.send().await {
        Ok(res) => res,
        Err(e) => {
            let status = if e.is_timeout() {
                StatusCode::GATEWAY_TIMEOUT
            } else {
                StatusCode::BAD_GATEWAY
            };
            eprintln!("[-] Error connecting to upstream models endpoint ({}): {}", upstream, e);
            return Err(status);
        }
    };

    let status = StatusCode::from_u16(upstream_res.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let stream = upstream_res.bytes_stream();
    let body = Body::from_stream(stream);

    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(body)
        .unwrap())
}
