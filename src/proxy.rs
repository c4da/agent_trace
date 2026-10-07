use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
};
use std::sync::Arc;
use crate::{
    db::Database,
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
}

pub async fn handle_chat_completions(
    State(cfg): State<Arc<ProxyConfig>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, StatusCode> {
    // Parse request payload to extract prompt, model, and stream mode
    let req_json: Option<serde_json::Value> = serde_json::from_slice(&body).ok();
    let model_name = req_json.as_ref()
        .and_then(|j| j.get("model"))
        .and_then(|m| m.as_str())
        .unwrap_or("unknown")
        .to_string();

    // 1. Dynamic upstream resolution: use X-Upstream-Url header if provided, else auto-route by model!
    let (upstream_base, node_id) = if let Some(custom) = headers.get("x-upstream-url").and_then(|v| v.to_str().ok()) {
        let nid = headers
            .get("x-node-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("PC-1")
            .to_string();
        (custom.trim_end_matches('/').to_string(), nid)
    } else {
        let m_lower = model_name.to_lowercase();
        if m_lower.contains("deepseek") || m_lower.contains("r1") || m_lower.contains("hermes") {
            // Auto-route deepseek-r1 and hermes to PC-2 Worker Node (:8642)
            ("http://192.168.1.102:8642".to_string(), "PC-2".to_string())
        } else {
            // Auto-route Qwen (qwen3.8:27b, qwen3:14b) and others to PC-1 local Ollama (:11434)
            (cfg.upstream_url.trim_end_matches('/').to_string(), "PC-1".to_string())
        }
    };

    // SSRF Prevention: Enforce upstream allowlist
    if !cfg.security.is_upstream_allowed(&upstream_base) {
        eprintln!("[-] [Security Block] Upstream URL not permitted: {}", upstream_base);
        return Err(StatusCode::FORBIDDEN);
    }

    let upstream = format!("{}/v1/chat/completions", upstream_base);

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
    let mut req = cfg.client.post(&upstream).header("content-type", "application/json");
    if let Some(auth) = headers.get("authorization") {
        req = req.header("authorization", auth);
    } else if let Some(ref worker_key) = cfg.security.worker_hermes_key {
        // If connecting to PC-2 or hermes and client didn't supply auth, attach configured worker key
        if upstream_base.contains("192.168.1.102") || upstream_base.contains(":8642") {
            req = req.header("authorization", format!("Bearer {}", worker_key));
        }
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
    let identity = cfg.identity.clone();

    if is_stream {
        let duration_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let content_type = upstream_res
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/event-stream")
            .to_string();

        tokio::spawn(async move {
            let mut ev = TraceEvent {
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
            let _ = db.insert_event_signed(&mut ev, Some(&*identity));
        });

        let stream = upstream_res.bytes_stream();
        let body = Body::from_stream(stream);

        Ok(Response::builder()
            .status(status)
            .header("content-type", content_type)
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
            let mut ev = TraceEvent {
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
            let _ = db.insert_event_signed(&mut ev, Some(&*identity));
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
    _headers: HeaderMap,
) -> Result<Response, StatusCode> {
    // 1. Query PC-1 Local Models (Ollama)
    let mut models_list = Vec::new();

    let pc1_tags_url = format!("{}/api/tags", cfg.upstream_url.trim_end_matches('/'));
    if let Ok(resp) = cfg.client.get(&pc1_tags_url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(arr) = json.get("models").and_then(|m| m.as_array()) {
                for m in arr {
                    if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                        let role = if name.contains("27b") {
                            "Heavy Reasoning & Architecture Lead"
                        } else if name.contains("14b") {
                            "Fast Code Generation & Boilerplate"
                        } else {
                            "Local Offline Executor"
                        };
                        models_list.push(serde_json::json!({
                            "id": name,
                            "object": "model",
                            "created": 1791370000,
                            "owned_by": "PC-1 (VECTOR)",
                            "node_id": "PC-1",
                            "host": "192.168.1.101:11434",
                            "role": role,
                            "permission": []
                        }));
                    }
                }
            }
        }
    }

    // Ensure configured Qwen models are visible in registry even while downloading
    let qwen_keys = ["qwen3.8:27b", "qwen3:14b"];
    for qk in qwen_keys {
        if !models_list.iter().any(|m| m.get("id").and_then(|id| id.as_str()) == Some(qk)) {
            let role = if qk.contains("27b") {
                "Heavy Reasoning & Architecture Lead (Configured)"
            } else {
                "Fast Code Generation & Boilerplate (Configured)"
            };
            models_list.push(serde_json::json!({
                "id": qk,
                "object": "model",
                "created": 1791370000,
                "owned_by": "PC-1 (VECTOR)",
                "node_id": "PC-1",
                "host": "192.168.1.101:11434",
                "role": role,
                "permission": []
            }));
        }
    }

    // 2. Query PC-2 Worker Models (Hermes Agent :8642)
    let pc2_models_url = "http://192.168.1.102:8642/v1/models";
    let mut pc2_found = false;
    let mut req = cfg.client.get(pc2_models_url);
    if let Some(ref key) = cfg.security.worker_hermes_key {
        req = req.header("authorization", format!("Bearer {}", key));
    }
    if let Ok(resp) = req.send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(arr) = json.get("data").and_then(|d| d.as_array()) {
                for m in arr {
                    if let Some(id) = m.get("id").and_then(|i| i.as_str()) {
                        pc2_found = true;
                        models_list.push(serde_json::json!({
                            "id": id,
                            "object": "model",
                            "created": 1791370000,
                            "owned_by": "PC-2 (NODE-02)",
                            "node_id": "PC-2",
                            "host": "192.168.1.102:8642",
                            "role": "CUDA Swarm Worker (Active)",
                            "permission": []
                        }));
                    }
                }
            }
        }
    }

    // Ensure DeepSeek-R1 1.5B is registered in cluster catalog
    if !models_list.iter().any(|m| m.get("id").and_then(|id| id.as_str()) == Some("deepseek-r1:1.5b")) {
        models_list.push(serde_json::json!({
            "id": "deepseek-r1:1.5b",
            "object": "model",
            "created": 1791370000,
            "owned_by": "PC-2 (NODE-02)",
            "node_id": "PC-2",
            "host": "192.168.1.102:8642",
            "role": "Ultra-Fast GPU Reasoning & Sanity Verification",
            "permission": []
        }));
    }

    let payload = serde_json::json!({
        "object": "list",
        "total_cluster_models": models_list.len(),
        "pc2_worker_online": pc2_found,
        "data": models_list
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap_or_default()))
        .unwrap())
}
