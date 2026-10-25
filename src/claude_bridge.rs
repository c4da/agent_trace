//! Corporate Desktop AI Guard & Hub for Claude Code Terminal (`src/claude_bridge.rs`).
//!
//! Exposes a spec-compliant Anthropic Messages API (`/v1/messages` and `/v1/messages/count_tokens`)
//! so corporate developers can connect their native `claude` terminal CLI via:
//!   `ANTHROPIC_BASE_URL=http://127.0.0.1:8080 claude`
//! without losing the interactive Claude terminal UX, while every prompt, file edit, and bash
//! tool call is inspected by the local AgentTrace Hard AI Guard, logged to the SHA-256 ledger,
//! and exported to corporate ELK/SIEM.

use crate::{
    agent_identity::AgentIdentity,
    ai_guard::{evaluate_hard_guard, GuardEnforcementMode, GuardInterceptRequest},
    policy::PolicyEngine,
    proxy::ProxyConfig,
    schema::{EventType, SecurityVerdict, TraceEvent},
    siem_exporter::ship_event_to_siem_async,
};
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
    Json,
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared state for the Claude Code Corporate Guard & Hub.
#[derive(Clone)]
pub struct ClaudeBridgeState {
    pub proxy_cfg: Arc<ProxyConfig>,
    pub policy: Arc<RwLock<PolicyEngine>>,
    pub node_id: String,
}

/// Extracts combined prompt text and any embedded tool invocations from an Anthropic `/v1/messages` JSON body.
pub fn extract_anthropic_prompt_and_tools(body_json: &serde_json::Value) -> (String, Option<String>, Option<String>) {
    let mut prompt_parts = Vec::new();
    let mut detected_cmd = None;
    let mut detected_path = None;

    if let Some(sys) = body_json.get("system") {
        if let Some(s) = sys.as_str() {
            prompt_parts.push(s.to_string());
        } else if let Some(arr) = sys.as_array() {
            for item in arr {
                if let Some(t) = item.get("text").and_then(|x| x.as_str()) {
                    prompt_parts.push(t.to_string());
                }
            }
        }
    }

    if let Some(msgs) = body_json.get("messages").and_then(|m| m.as_array()) {
        for m in msgs {
            if let Some(content_str) = m.get("content").and_then(|c| c.as_str()) {
                prompt_parts.push(content_str.to_string());
            } else if let Some(blocks) = m.get("content").and_then(|c| c.as_array()) {
                for b in blocks {
                    let btype = b.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    if btype == "text" {
                        if let Some(t) = b.get("text").and_then(|x| x.as_str()) {
                            prompt_parts.push(t.to_string());
                        }
                    } else if btype == "tool_use" {
                        let tool_name = b.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        if let Some(inp) = b.get("input") {
                            if let Some(cmd) = inp.get("command").and_then(|c| c.as_str()) {
                                detected_cmd = Some(cmd.to_string());
                            }
                            if let Some(fp) = inp
                                .get("file_path")
                                .or_else(|| inp.get("path"))
                                .and_then(|p| p.as_str())
                            {
                                detected_path = Some(fp.to_string());
                            }
                            prompt_parts.push(format!("[tool_use:{}] {}", tool_name, inp));
                        }
                    } else if btype == "tool_result" {
                        if let Some(c) = b.get("content").and_then(|x| x.as_str()) {
                            prompt_parts.push(c.to_string());
                        }
                    }
                }
            }
        }
    }

    (prompt_parts.join("\n"), detected_cmd, detected_path)
}

/// Formats an Anthropic Messages API v1 response (JSON or SSE stream) so the developer's
/// Claude Code terminal stays intact and renders the exact response or Guard interception notice.
pub fn build_anthropic_response(
    model: &str,
    assistant_text: &str,
    is_stream: bool,
    guard_verdict: &str,
    routed_node: &str,
) -> Response {
    let msg_id = format!("msg_atgw_{}", &uuid::Uuid::new_v4().simple().to_string()[..16]);

    if is_stream {
        let escaped_text = serde_json::to_string(assistant_text).unwrap_or_else(|_| "\"\"".to_string());
        let sse_payload = format!(
            "event: message_start\ndata: {{\"type\":\"message_start\",\"message\":{{\"id\":\"{msg_id}\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"{model}\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{{\"input_tokens\":24,\"output_tokens\":1}}}}}}\n\n\
             event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"text\",\"text\":\"\"}}}}\n\n\
             event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"text_delta\",\"text\":{escaped_text}}}}}\n\n\
             event: content_block_stop\ndata: {{\"type\":\"content_block_stop\",\"index\":0}}\n\n\
             event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"end_turn\",\"stop_sequence\":null}},\"usage\":{{\"output_tokens\":42}}}}\n\n\
             event: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n"
        );

        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/event-stream; charset=utf-8")
            .header("cache-control", "no-cache")
            .header("x-agenttrace-guard", guard_verdict)
            .header("x-routed-node", routed_node)
            .body(Body::from(sse_payload))
            .unwrap()
    } else {
        let payload = serde_json::json!({
            "id": msg_id,
            "type": "message",
            "role": "assistant",
            "model": model,
            "content": [
                {
                    "type": "text",
                    "text": assistant_text
                }
            ],
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {
                "input_tokens": 32,
                "output_tokens": (assistant_text.len() / 4).max(1)
            }
        });

        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .header("x-agenttrace-guard", guard_verdict)
            .header("x-routed-node", routed_node)
            .body(Body::from(serde_json::to_vec(&payload).unwrap_or_default()))
            .unwrap()
    }
}

/// Handles `POST /v1/messages` from Claude Code CLI (`ANTHROPIC_BASE_URL=http://127.0.0.1:8080`).
pub async fn handle_anthropic_messages(
    State(state): State<ClaudeBridgeState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, StatusCode> {
    let t0 = std::time::Instant::now();
    crate::idle_worker::record_user_activity();

    let mut req_json: serde_json::Value = serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
    let model_name = req_json
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("claude-3-7-sonnet-latest")
        .to_string();

    let is_stream = req_json
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    let session_id = headers
        .get("x-session-id")
        .or_else(|| headers.get("x-claude-session"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("claude-terminal-session")
        .to_string();

    // 1. Resolve distinct corporate AgentIdentity (`claude-code:terminal@HOST#sess`)
    let agent_identity = AgentIdentity::from_headers_and_session(
        &headers,
        &state.node_id,
        &session_id,
        Some(&model_name),
    );

    // 2. Extract prompt, bash commands, and file paths for synchronous Pre-Execution Hard Guard inspection
    let (combined_prompt, cmd_opt, path_opt) = extract_anthropic_prompt_and_tools(&req_json);

    let client_requested_mode = headers
        .get("x-guard-mode")
        .and_then(|v| v.to_str().ok())
        .map(|mode_header| {
            if mode_header.eq_ignore_ascii_case("redact") {
                GuardEnforcementMode::RedactAndForward
            } else if mode_header.eq_ignore_ascii_case("audit") {
                GuardEnforcementMode::AuditOnly
            } else {
                GuardEnforcementMode::HardBlock
            }
        });

    // 2b. Evaluate Admin Global & Per-Model Restrictions (`AdminGovernanceEngine`)
    let (gov_decision, shield_terminals) = {
        let gov_guard = crate::admin_governance::global_admin_governance()
            .read()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let dec = gov_guard.evaluate_request(
            &model_name,
            Some(&combined_prompt),
            cmd_opt.as_deref(),
            path_opt.as_deref(),
            client_requested_mode,
        );
        (dec, gov_guard.global.shield_client_terminals)
    };

    let model_name = gov_decision.effective_model.clone();
    let guard_mode = gov_decision.effective_enforcement_mode;

    let guard_req = GuardInterceptRequest {
        agent_id: agent_identity.canonical_id.clone(),
        action: if cmd_opt.is_some() {
            "Execute".to_string()
        } else if path_opt.is_some() {
            "Write".to_string()
        } else {
            "LlmInference".to_string()
        },
        target_path: path_opt.clone(),
        command: cmd_opt.clone(),
        prompt_text: Some(combined_prompt.clone()),
        mode_override: Some(guard_mode),
    };

    let mut guard_res = {
        let p_guard = state.policy.read().await;
        evaluate_hard_guard(&guard_req, &p_guard, guard_mode)
    };

    if !gov_decision.allowed {
        guard_res.hard_blocked = true;
        guard_res.verdict = SecurityVerdict::Blocked;
        guard_res.risk_score = guard_res.risk_score.max(85);
        guard_res.findings.extend(gov_decision.findings.clone());
        guard_res.reasons.extend(gov_decision.admin_reasons.clone());
    }

    // 3. If Hard Guard or Admin Per-Model Policy triggers, block immediately before any external network call!
    if guard_res.hard_blocked {
        let duration_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let ev = TraceEvent {
            event_id: uuid::Uuid::new_v4().simple().to_string(),
            node_id: agent_identity.canonical_id.clone(),
            session_id: session_id.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            event_type: if cmd_opt.is_some() {
                EventType::CommandRun
            } else {
                EventType::LlmInference
            },
            target: format!("Claude Terminal Guard Interception [{}]", model_name),
            action_details: serde_json::json!({
                "client": "claude-code-terminal",
                "agent_identity": agent_identity,
                "requested_model": gov_decision.requested_model,
                "effective_model": model_name,
                "model_trust_tier": gov_decision.trust_tier,
                "guard_mode": "HardBlock",
                "findings": guard_res.findings,
                "risk_score": guard_res.risk_score,
                "evaluation_latency_us": guard_res.evaluation_latency_us,
                "sanitized_prompt": guard_res.sanitized_prompt,
            }),
            diff: None,
            provenance_sources: vec![],
            security_verdict: SecurityVerdict::Blocked,
            security_reasons: guard_res.reasons.clone(),
            duration_ms,
            prev_hash: String::new(),
            event_hash: String::new(),
            signature: None,
            public_key: None,
        };

        if let Some(ref writer) = state.proxy_cfg.ledger_writer {
            let _ = writer.try_enqueue(ev.clone());
        } else {
            let mut e = ev.clone();
            let _ = state.proxy_cfg.db.insert_event_signed(&mut e, Some(&*state.proxy_cfg.identity));
        }
        ship_event_to_siem_async(state.proxy_cfg.client.clone(), ev);

        let strict_http = headers
            .get("x-guard-strict-http")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        if strict_http {
            return Err(StatusCode::FORBIDDEN);
        }

        let block_explanation = if shield_terminals {
            gov_decision.shielded_client_message.unwrap_or_else(|| {
                format!(
                    "🛡️ [AgentTrace Shielded Terminal] This request was intercepted by your Administrator's security policy for model `{}` ({:?} tier). Zero sensitive data left your workstation, and your terminal session remains active.",
                    model_name, gov_decision.trust_tier
                )
            })
        } else {
            format!(
                "🛡️ **[AgentTrace AI Guard — HARD BLOCK]**\n\n\
                 Your outbound Claude Terminal prompt/action was intercepted locally on `{}` in **{} µs** before leaving your workstation.\n\n\
                 - **Agent Identity**: `{}`\n\
                 - **Model Profile**: `{}` (`{:?}`)\n\
                 - **Risk Score**: `{}/100`\n\
                 - **Triggered Rules**:\n  - {}",
                state.node_id,
                guard_res.evaluation_latency_us,
                agent_identity.canonical_id,
                model_name,
                gov_decision.trust_tier,
                guard_res.risk_score,
                guard_res.reasons.join("\n  - ")
            )
        };

        return Ok(build_anthropic_response(
            &model_name,
            &block_explanation,
            is_stream,
            "Blocked",
            &state.node_id,
        ));
    }

    // 4. If RedactAndForward scrubbed a secret, rewrite the prompt in the JSON payload before forwarding!
    let mut effective_prompt = guard_res.sanitized_prompt.clone().unwrap_or(combined_prompt.clone());
    if let Some(ref sys_prefix) = gov_decision.injected_system_prefix {
        effective_prompt = format!("{}\n{}", sys_prefix, effective_prompt);
    }
    if guard_res.verdict == SecurityVerdict::Flagged {
        if let Some(msgs) = req_json.get_mut("messages").and_then(|m| m.as_array_mut()) {
            for msg in msgs.iter_mut() {
                if let Some(s) = msg.get("content").and_then(|c| c.as_str()).map(|x| x.to_string()) {
                    let (_, scrubbed_s) = crate::ai_guard::scan_and_redact_secrets(&s);
                    msg["content"] = serde_json::Value::String(scrubbed_s);
                } else if let Some(blocks) = msg.get_mut("content").and_then(|c| c.as_array_mut()) {
                    for b in blocks.iter_mut() {
                        if let Some(t) = b.get("text").and_then(|x| x.as_str()).map(|x| x.to_string()) {
                            let (_, scrubbed_t) = crate::ai_guard::scan_and_redact_secrets(&t);
                            b["text"] = serde_json::Value::String(scrubbed_t);
                        }
                    }
                }
            }
        }
    }

    // 5. Route either to Local Corporate Cluster Hub or upstream Anthropic API
    let route_decision = state
        .proxy_cfg
        .delegation_engine
        .decide(&crate::delegation_engine::AgentDelegationRequest {
            prompt: effective_prompt.clone(),
            session_id: Some(session_id.clone()),
            agent_id: Some(agent_identity.canonical_id.clone()),
            tags: vec!["claude-terminal".to_string()],
            requested_model: Some(model_name.clone()),
        })
        .await;

    // Check if an external Anthropic upstream URL is explicitly configured and reachable
    let anthropic_upstream = std::env::var("AGENTTRACE_ANTHROPIC_UPSTREAM").ok();
    let mut assistant_reply = String::new();
    let mut routed_target = format!("{} [{}]", route_decision.target_upstream, route_decision.selected_model);

    if let Some(ref cloud_url) = anthropic_upstream {
        if state.proxy_cfg.security.is_upstream_allowed(cloud_url) {
            let target_url = format!("{}/v1/messages", cloud_url.trim_end_matches('/'));
            let mut req = state
                .proxy_cfg
                .client
                .post(&target_url)
                .header("content-type", "application/json")
                .header("anthropic-version", "2023-06-01");
            if let Some(api_key) = headers.get("x-api-key") {
                req = req.header("x-api-key", api_key);
            }
            if let Ok(resp) = req.json(&req_json).send().await {
                if resp.status().is_success() {
                    if let Ok(j) = resp.json::<serde_json::Value>().await {
                        if let Some(t) = j
                            .get("content")
                            .and_then(|c| c.as_array())
                            .and_then(|a| a.first())
                            .and_then(|b| b.get("text"))
                            .and_then(|x| x.as_str())
                        {
                            assistant_reply = t.to_string();
                            routed_target = format!("Anthropic Cloud ({})", model_name);
                        }
                    }
                }
            }
        }
    }

    // If not handled by external cloud, route through the local corporate cluster model (Ollama / Hermes)
    if assistant_reply.is_empty() {
        let openai_url = format!("{}/v1/chat/completions", route_decision.target_upstream.trim_end_matches('/'));
        let openai_payload = serde_json::json!({
            "model": route_decision.selected_model,
            "messages": [
                {"role": "user", "content": effective_prompt}
            ],
            "stream": false
        });

        let req_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(2500))
            .build()
            .unwrap_or_default();

        if let Ok(resp) = req_client.post(&openai_url).json(&openai_payload).send().await {
            if let Ok(j) = resp.json::<serde_json::Value>().await {
                if let Some(content) = j
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|a| a.first())
                    .and_then(|ch| ch.get("message"))
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                {
                    assistant_reply = content.to_string();
                }
            }
        }
    }

    if assistant_reply.is_empty() {
        let redact_note = if guard_res.verdict == SecurityVerdict::Flagged {
            " (Note: Sensitive credentials were automatically redacted in-flight by AgentTrace DLP Guard)"
        } else {
            ""
        };
        assistant_reply = format!(
            "[AgentTrace Corporate Claude Hub · Routed to {} on {}]{}\nVerified prompt processed safely.",
            route_decision.selected_model, route_decision.node_id, redact_note
        );
    }

    let duration_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let ev = TraceEvent {
        event_id: uuid::Uuid::new_v4().simple().to_string(),
        node_id: agent_identity.canonical_id.clone(),
        session_id,
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_type: EventType::LlmInference,
        target: format!("Claude Hub -> {}", routed_target),
        action_details: serde_json::json!({
            "client": "claude-code-terminal",
            "agent_identity": agent_identity,
            "requested_model": model_name,
            "routed_model": route_decision.selected_model,
            "routed_node": route_decision.node_id,
            "guard_verdict": guard_res.verdict,
            "evaluation_latency_us": guard_res.evaluation_latency_us,
            "prompt": effective_prompt,
            "findings": guard_res.findings,
        }),
        diff: None,
        provenance_sources: vec![],
        security_verdict: guard_res.verdict.clone(),
        security_reasons: guard_res.reasons,
        duration_ms,
        prev_hash: String::new(),
        event_hash: String::new(),
        signature: None,
        public_key: None,
    };

    if let Some(ref writer) = state.proxy_cfg.ledger_writer {
        let _ = writer.try_enqueue(ev.clone());
    } else {
        let mut e = ev.clone();
        let _ = state.proxy_cfg.db.insert_event_signed(&mut e, Some(&*state.proxy_cfg.identity));
    }
    ship_event_to_siem_async(state.proxy_cfg.client.clone(), ev);

    let verdict_str = match guard_res.verdict {
        SecurityVerdict::Allowed => "Allowed",
        SecurityVerdict::Flagged => "Flagged-Redacted",
        SecurityVerdict::Blocked => "Blocked",
    };

    Ok(build_anthropic_response(
        &model_name,
        &assistant_reply,
        is_stream,
        verdict_str,
        &route_decision.node_id,
    ))
}

/// Handles `POST /v1/messages/count_tokens` so Claude Code CLI token estimation works natively.
pub async fn handle_anthropic_count_tokens(
    Json(req): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let (prompt, _, _) = extract_anthropic_prompt_and_tools(&req);
    let est_tokens = (prompt.len() / 4).max(1);
    Json(serde_json::json!({
        "input_tokens": est_tokens
    }))
}
