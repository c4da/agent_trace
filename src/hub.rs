//! Autonomous Multi-Node Prompt Hub (`POST /api/v1/hub/dispatch`).
//!
//! Eliminates manual model/node selection: accepts any raw prompt and automatically:
//! - Classifies intent (`ComplexReasoning`, `FastCodeGeneration`, `SanityVerification`, `GeneralExecution`)
//! - Arbitrates the optimal cluster node & model (`auto` mode)
//! - Or executes a 2-stage **Draft -> Verify Pipeline** (`pipeline` mode: Qwen builds, DeepSeek-R1 audits)
//! - Or executes a **Parallel Multi-Model Swarm** (`swarm` mode: Qwen + DeepSeek-R1 in parallel)
//! - Transparently fails over to local Ollama (`PC-1`) if a remote worker node (`PC-2`) is unreachable
//! - Automatically unloads heavy 27B models (`keep_alive: 0`) after inference to protect host VRAM
//! - Signs and records every hop in the cryptographic SHA-256 + Ed25519 ledger.

use crate::{
    delegation_engine::AgentDelegationRequest,
    proxy::ProxyConfig,
    router::{classify_intent, RoutingIntent},
    schema::{EventType, SecurityVerdict, TraceEvent},
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

#[derive(Debug, Clone, Deserialize)]
pub struct HubDispatchRequest {
    pub prompt: String,
    /// `"auto"` (default single-shot smart route), `"pipeline"` (Qwen Draft -> DeepSeek Verify), `"swarm"` (Parallel Qwen + DeepSeek), or `"workgroup"`.
    pub mode: Option<String>,
    /// Optional Problem Workgroup ID (e.g. `"wg-coding"`, `"wg-image-gen"`). If `"auto"` or omitted, matches by prompt keywords.
    pub workgroup_id: Option<String>,
    /// Optional explicit model override (defaults to `"auto"`).
    pub requested_model: Option<String>,
    /// Optional caller agent ID for ledger attribution.
    pub agent_id: Option<String>,
    /// Optional system prompt prepended to the user prompt.
    pub system_prompt: Option<String>,
    /// Automatically issue `keep_alive: 0` after running heavy 27B+ models (defaults to `true`).
    pub unload_heavy_after: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubStageResult {
    pub stage_name: String,
    pub intent: RoutingIntent,
    pub node_id: String,
    pub selected_model: String,
    pub target_upstream: String,
    pub fallback_used: bool,
    pub vram_unloaded: bool,
    pub duration_ms: f64,
    pub reasoning: String,
    pub response: String,
    pub event_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubDispatchResponse {
    pub mode: String,
    pub prompt: String,
    pub classified_intent: RoutingIntent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_workgroup: Option<String>,
    #[serde(default)]
    pub injected_rules_count: usize,
    pub primary_response: String,
    pub verification_notes: Option<String>,
    pub stages: Vec<HubStageResult>,
    pub total_duration_ms: f64,
}

/// Strips `<think>...</think>` blocks if the model emitted raw chain-of-thought tags,
/// preserving the clean actionable answer (or falling back to full text if only think tags exist).
fn clean_model_output(raw: &str) -> String {
    if let Some(end_idx) = raw.find("</think>") {
        let after = raw[end_idx + 8..].trim();
        if !after.is_empty() {
            return after.to_string();
        }
    }
    raw.trim().to_string()
}

/// Executes a single chat completion against an upstream endpoint, with automatic failover
/// to `default_upstream` (PC-1) if a remote worker node is offline or times out.
async fn execute_stage_call(
    cfg: &Arc<ProxyConfig>,
    stage_name: &str,
    prompt: &str,
    system_prompt: Option<&str>,
    intent: RoutingIntent,
    preferred_node: &str,
    preferred_model: &str,
    preferred_upstream: &str,
    reasoning: &str,
    agent_id: &str,
    unload_heavy: bool,
) -> HubStageResult {
    let t0 = std::time::Instant::now();

    // Append /no_think for Qwen3 models unless /think is explicitly requested so multi-stage Hub pipelines return in seconds
    let effective_prompt = if preferred_model.to_lowercase().starts_with("qwen3")
        && !prompt.contains("/think")
        && !prompt.contains("/no_think")
    {
        format!("{}\n/no_think", prompt)
    } else {
        prompt.to_string()
    };

    let mut messages = Vec::new();
    if let Some(sys) = system_prompt {
        if !sys.trim().is_empty() {
            messages.push(serde_json::json!({ "role": "system", "content": sys }));
        }
    }
    messages.push(serde_json::json!({ "role": "user", "content": effective_prompt }));

    let req_body = serde_json::json!({
        "model": preferred_model,
        "messages": messages,
        "stream": false,
        "think": false
    });

    let primary_url = format!("{}/v1/chat/completions", preferred_upstream.trim_end_matches('/'));
    let mut req = cfg
        .client
        .post(&primary_url)
        .timeout(Duration::from_secs(90))
        .header("content-type", "application/json")
        .json(&req_body);

    if let Some(ref worker_key) = cfg.security.worker_hermes_key {
        if preferred_upstream.contains("192.168.1.102") || preferred_upstream.contains(":8642") {
            req = req.header("authorization", format!("Bearer {}", worker_key));
        }
    }

    let mut final_node = preferred_node.to_string();
    let mut final_model = preferred_model.to_string();
    let mut final_upstream = preferred_upstream.to_string();
    let mut fallback_used = false;

    let response_text = match req.send().await {
        Ok(res) if res.status().is_success() => {
            let json: serde_json::Value = res.json().await.unwrap_or_default();
            json.get("choices")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string()
        }
        _ => {
            // Automatic failover to PC-1 local Ollama if remote node failed or model wasn't loaded
            fallback_used = true;
            final_node = "PC-1".to_string();
            final_upstream = cfg.upstream_url.trim_end_matches('/').to_string();
            // Pick a fast guaranteed local model on PC-1
            final_model = if preferred_model.contains("deepseek") {
                "deepseek-r1:1.5b".to_string()
            } else {
                "qwen3.5:9b".to_string()
            };

            let fb_url = format!("{}/v1/chat/completions", final_upstream);
            let fb_body = serde_json::json!({
                "model": final_model,
                "messages": messages,
                "stream": false,
                "think": false
            });
            match cfg
                .client
                .post(&fb_url)
                .timeout(Duration::from_secs(60))
                .json(&fb_body)
                .send()
                .await
            {
                Ok(res) if res.status().is_success() => {
                    let json: serde_json::Value = res.json().await.unwrap_or_default();
                    json.get("choices")
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|c| c.get("message"))
                        .and_then(|m| m.get("content"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string()
                }
                Ok(res) => format!(
                    "[Hub Upstream Status {} from {} ({})]",
                    res.status(),
                    final_model,
                    final_upstream
                ),
                Err(e) => format!(
                    "[Hub Offline Preview — Routed to {} ({}) on {}: {}]",
                    final_model, stage_name, final_node, e
                ),
            }
        }
    };

    let cleaned_response = clean_model_output(&response_text);

    // Local VRAM Hygiene (Rule 5): Automatically unload 27B+ models when requested
    let mut vram_unloaded = false;
    if unload_heavy && (final_model.contains("27b") || final_model.contains("32b")) {
        let unload_url = format!("{}/api/generate", final_upstream.trim_end_matches('/'));
        let _ = cfg
            .client
            .post(&unload_url)
            .timeout(Duration::from_secs(5))
            .json(&serde_json::json!({
                "model": final_model,
                "keep_alive": 0
            }))
            .send()
            .await;
        vram_unloaded = true;
    }

    let duration_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let event_id = format!("hub-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);

    let ev = TraceEvent {
        event_id: event_id.clone(),
        node_id: final_node.clone(),
        session_id: format!("prompt-hub:{}", agent_id),
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_type: EventType::LlmInference,
        target: format!("{}/v1/chat/completions [{}]", final_upstream, final_model),
        action_details: serde_json::json!({
            "hub_stage": stage_name,
            "intent": intent,
            "model": final_model,
            "node_id": final_node,
            "fallback_used": fallback_used,
            "vram_unloaded": vram_unloaded,
            "reasoning": reasoning,
            "prompt": prompt,
            "completion": cleaned_response,
            "duration_ms": duration_ms,
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

    if let Some(ref writer) = cfg.ledger_writer {
        let _ = writer.try_enqueue(ev);
    } else {
        let mut signed_ev = ev;
        let _ = cfg.db.insert_event_signed(&mut signed_ev, Some(&*cfg.identity));
    }

    HubStageResult {
        stage_name: stage_name.to_string(),
        intent,
        node_id: final_node,
        selected_model: final_model,
        target_upstream: final_upstream,
        fallback_used,
        vram_unloaded,
        duration_ms,
        reasoning: reasoning.to_string(),
        response: cleaned_response,
        event_id,
    }
}

/// Handles `POST /api/v1/hub/dispatch` — the unified Prompt Hub endpoint.
pub async fn handle_hub_dispatch(
    State(cfg): State<Arc<ProxyConfig>>,
    headers: HeaderMap,
    Json(req): Json<HubDispatchRequest>,
) -> Result<Json<HubDispatchResponse>, StatusCode> {
    if !cfg.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let t0 = std::time::Instant::now();
    let explicit_wg_id = req
        .workgroup_id
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != "none");

    // Resolve Problem Workgroup (explicit ID or automatic keyword match)
    let resolved_wg = {
        let wg_reg = crate::workgroups::global_workgroups();
        let mut guard = wg_reg.write().await;
        let found = if let Some(id) = explicit_wg_id {
            if id == "auto" {
                guard.match_workgroup_by_prompt(&req.prompt)
            } else {
                guard.get(id)
            }
        } else {
            guard.match_workgroup_by_prompt(&req.prompt)
        };
        if let Some(ref wg) = found {
            guard.increment_tasks(&wg.id);
        }
        found
    };

    let matched_workgroup = resolved_wg.as_ref().map(|w| w.id.clone());
    let injected_rules_count = resolved_wg
        .as_ref()
        .map(|w| w.markdown_rules.len())
        .unwrap_or(0);

    // Merge attached .md rule files with any caller-supplied system prompt
    let merged_system_prompt = match (&resolved_wg, &req.system_prompt) {
        (Some(wg), Some(custom_sys)) if !custom_sys.trim().is_empty() => {
            Some(format!("{}\n\n{}", wg.compile_system_prompt(), custom_sys))
        }
        (Some(wg), _) => Some(wg.compile_system_prompt()),
        (None, Some(custom_sys)) => Some(custom_sys.clone()),
        (None, None) => None,
    };

    let mode = req
        .mode
        .as_deref()
        .unwrap_or_else(|| {
            if explicit_wg_id.is_some() {
                resolved_wg
                    .as_ref()
                    .map(|w| w.execution_strategy.as_str())
                    .unwrap_or("auto")
            } else {
                "auto"
            }
        })
        .trim()
        .to_lowercase();

    let agent_id = req.agent_id.as_deref().unwrap_or("Lead-Hub-Client");
    let unload_heavy = req.unload_heavy_after.unwrap_or(true);
    let classified_intent = classify_intent(&req.prompt);

    // If an explicit Problem Workgroup with assigned members was selected, run its assigned roster!
    if let Some(ref wg) = resolved_wg {
        if explicit_wg_id.is_some() && explicit_wg_id != Some("auto") && !wg.members.is_empty() {
            let mut stages: Vec<HubStageResult> = Vec::new();
            let active_members: Vec<_> = wg.members.iter().take(2).collect();

            for (idx, member) in active_members.iter().enumerate() {
                let (target_node, target_model, target_upstream) =
                    crate::workgroups::ProblemWorkgroup::resolve_executable_target(
                        member,
                        &cfg.upstream_url,
                    );
                let stage_label = format!(
                    "{}. {} [{} · {}]",
                    idx + 1,
                    member.display_name,
                    member.role.label(),
                    wg.name
                );
                let stage_prompt = if idx == 0 || stages.is_empty() {
                    req.prompt.clone()
                } else {
                    let prev_resp: & str = &stages[0].response;
                    format!(
                        "Workgroup '{}' Stage {} ({} - {}):\nReview, refine, or build upon Stage 1's output while strictly enforcing all attached .md rules.\n\nOriginal Task: {}\n\nStage 1 Output:\n{}",
                        wg.name,
                        idx + 1,
                        member.display_name,
                        member.role.label(),
                        req.prompt,
                        prev_resp
                    )
                };

                let stage_res = execute_stage_call(
                    &cfg,
                    &stage_label,
                    &stage_prompt,
                    merged_system_prompt.as_deref(),
                    classified_intent.clone(),
                    &target_node,
                    &target_model,
                    &target_upstream,
                    &format!(
                        "Workgroup '{}' ({}) assigned member {} ({}) with {} attached .md rule(s)",
                        wg.name,
                        wg.id,
                        member.display_name,
                        member.role.label(),
                        injected_rules_count
                    ),
                    &member.agent_id,
                    unload_heavy,
                )
                .await;
                stages.push(stage_res);
            }

            let primary = stages
                .first()
                .map(|s| s.response.clone())
                .unwrap_or_default();
            let verif = if stages.len() > 1 {
                Some(stages[1].response.clone())
            } else {
                None
            };

            return Ok(Json(HubDispatchResponse {
                mode: format!("workgroup:{}", wg.id),
                prompt: req.prompt,
                classified_intent,
                matched_workgroup,
                injected_rules_count,
                primary_response: primary,
                verification_notes: verif,
                stages,
                total_duration_ms: t0.elapsed().as_secs_f64() * 1000.0,
            }));
        }
    }

    if mode == "pipeline" {
        // Stage 1: Draft / Code / Architecture on the best primary model
        let draft_intent = if classified_intent == RoutingIntent::SanityVerification {
            RoutingIntent::FastCodeGeneration
        } else {
            classified_intent.clone()
        };
        let draft_dec = cfg
            .delegation_engine
            .decide(&AgentDelegationRequest {
                prompt: req.prompt.clone(),
                session_id: Some("hub-pipeline".to_string()),
                agent_id: Some(agent_id.to_string()),
                tags: vec!["pipeline-draft".to_string()],
                requested_model: req.requested_model.clone().or_else(|| Some("auto".to_string())),
            })
            .await;

        let stage1 = execute_stage_call(
            &cfg,
            "1. Primary Architect / Code Builder",
            &req.prompt,
            merged_system_prompt.as_deref(),
            draft_intent,
            &draft_dec.node_id,
            &draft_dec.selected_model,
            &draft_dec.target_upstream,
            &draft_dec.reasoning,
            agent_id,
            unload_heavy,
        )
        .await;

        // Stage 2: Automatic Sanity Verification on DeepSeek-R1 (PC-2 or local fallback)
        let verify_prompt = format!(
            "Verify and critique the following solution for correctness, edge cases, and security. Be concise (max 4 bullets):\n\nTask: {}\n\nProposed Solution:\n{}",
            req.prompt, stage1.response
        );
        let verify_dec = cfg
            .delegation_engine
            .decide(&AgentDelegationRequest {
                prompt: verify_prompt.clone(),
                session_id: Some("hub-pipeline".to_string()),
                agent_id: Some(agent_id.to_string()),
                tags: vec!["pipeline-verify".to_string()],
                requested_model: Some("deepseek-r1:1.5b".to_string()),
            })
            .await;

        let stage2 = execute_stage_call(
            &cfg,
            "2. Sanity & Security Verifier (DeepSeek-R1)",
            &verify_prompt,
            merged_system_prompt
                .as_deref()
                .or(Some("You are a fast security and logic verifier. Output concise bullet points only.")),
            RoutingIntent::SanityVerification,
            &verify_dec.node_id,
            &verify_dec.selected_model,
            &verify_dec.target_upstream,
            &verify_dec.reasoning,
            agent_id,
            false,
        )
        .await;

        let primary = stage1.response.clone();
        let verif = Some(stage2.response.clone());
        return Ok(Json(HubDispatchResponse {
            mode: "pipeline".to_string(),
            prompt: req.prompt,
            classified_intent,
            matched_workgroup,
            injected_rules_count,
            primary_response: primary,
            verification_notes: verif,
            stages: vec![stage1, stage2],
            total_duration_ms: t0.elapsed().as_secs_f64() * 1000.0,
        }));
    } else if mode == "swarm" {
        // Parallel execution across Qwen (PC-1) and DeepSeek-R1 (PC-2 / fallback)
        let qwen_model = if classified_intent == RoutingIntent::ComplexReasoning {
            "qwen3.8:27b"
        } else {
            "qwen3.5:9b"
        };
        let dec_qwen = cfg
            .delegation_engine
            .decide(&AgentDelegationRequest {
                prompt: req.prompt.clone(),
                session_id: Some("hub-swarm".to_string()),
                agent_id: Some(agent_id.to_string()),
                tags: vec!["swarm-qwen".to_string()],
                requested_model: Some(qwen_model.to_string()),
            })
            .await;

        let dec_ds = cfg
            .delegation_engine
            .decide(&AgentDelegationRequest {
                prompt: req.prompt.clone(),
                session_id: Some("hub-swarm".to_string()),
                agent_id: Some(agent_id.to_string()),
                tags: vec!["swarm-deepseek".to_string()],
                requested_model: Some("deepseek-r1:1.5b".to_string()),
            })
            .await;

        let (stage_qwen, stage_ds) = tokio::join!(
            execute_stage_call(
                &cfg,
                "Swarm Node A (Qwen Builder/Architect)",
                &req.prompt,
                merged_system_prompt.as_deref(),
                classified_intent.clone(),
                &dec_qwen.node_id,
                &dec_qwen.selected_model,
                &dec_qwen.target_upstream,
                &dec_qwen.reasoning,
                agent_id,
                unload_heavy,
            ),
            execute_stage_call(
                &cfg,
                "Swarm Node B (DeepSeek-R1 Critic/Verifier)",
                &req.prompt,
                merged_system_prompt
                    .as_deref()
                    .or(Some("Provide a concise second opinion, edge-case check, or alternative angle in 4 bullet points.")),
                RoutingIntent::SanityVerification,
                &dec_ds.node_id,
                &dec_ds.selected_model,
                &dec_ds.target_upstream,
                &dec_ds.reasoning,
                agent_id,
                false,
            )
        );

        let primary = stage_qwen.response.clone();
        let verif = Some(stage_ds.response.clone());
        return Ok(Json(HubDispatchResponse {
            mode: "swarm".to_string(),
            prompt: req.prompt,
            classified_intent,
            matched_workgroup,
            injected_rules_count,
            primary_response: primary,
            verification_notes: verif,
            stages: vec![stage_qwen, stage_ds],
            total_duration_ms: t0.elapsed().as_secs_f64() * 1000.0,
        }));
    }

    // Default Mode: "auto" (Single-Shot Smart Intent Arbitration)
    let decision = cfg
        .delegation_engine
        .decide(&AgentDelegationRequest {
            prompt: req.prompt.clone(),
            session_id: Some("hub-auto".to_string()),
            agent_id: Some(agent_id.to_string()),
            tags: vec!["hub-auto".to_string()],
            requested_model: req.requested_model.clone().or_else(|| Some("auto".to_string())),
        })
        .await;

    let stage = execute_stage_call(
        &cfg,
        "Auto-Routed Execution",
        &req.prompt,
        merged_system_prompt.as_deref(),
        decision.intent.clone(),
        &decision.node_id,
        &decision.selected_model,
        &decision.target_upstream,
        &decision.reasoning,
        agent_id,
        unload_heavy,
    )
    .await;

    Ok(Json(HubDispatchResponse {
        mode: "auto".to_string(),
        prompt: req.prompt,
        classified_intent: decision.intent,
        matched_workgroup,
        injected_rules_count,
        primary_response: stage.response.clone(),
        verification_notes: None,
        stages: vec![stage],
        total_duration_ms: t0.elapsed().as_secs_f64() * 1000.0,
    }))
}
