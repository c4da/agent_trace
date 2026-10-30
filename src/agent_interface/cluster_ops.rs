//! Cluster Operations, Project Book & Workgroup Tool Executors (`src/agent_interface/cluster_ops.rs`).

use serde_json::{json, Value};
use crate::{
    agent_interface::AgentInterfaceState,
    schema::{EventType, SecurityVerdict, TraceEvent},
    workgroups::{global_workgroups, MarkdownRuleAttachment, ProblemWorkgroup, WorkgroupMember, WorkgroupRole},
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

/// Retrieves active Problem Workgroups and attached markdown rules.
pub async fn exec_get_workgroup_rules(params: &Value) -> (bool, Value, Option<String>) {
    let target_wg = params.get("workgroup_id").and_then(|v| v.as_str());
    let store = global_workgroups();
    let reg = store.read().await;

    if let Some(wg_id) = target_wg {
        if let Some(wg) = reg.groups.get(wg_id) {
            (true, json!({ "workgroup": wg }), None)
        } else {
            (false, json!({}), Some(format!("Workgroup '{}' not found", wg_id)))
        }
    } else {
        (true, json!({ "total_workgroups": reg.groups.len(), "workgroups": reg.groups }), None)
    }
}

/// Creates or updates a Problem Workgroup hierarchy with assigned agents, models, and domain objectives.
pub async fn exec_upsert_workgroup(params: &Value) -> (bool, Value, Option<String>) {
    let name = match params.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.trim().is_empty() => n.to_string(),
        _ => return (false, json!({}), Some("Missing required field: 'name'".to_string())),
    };
    let problem_domain = match params.get("problem_domain").and_then(|v| v.as_str()) {
        Some(d) if !d.trim().is_empty() => d.to_string(),
        _ => return (false, json!({}), Some("Missing required field: 'problem_domain'".to_string())),
    };

    let id = params.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| {
        let slug: String = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        format!("wg-{}", slug.trim_matches('-'))
    });

    let description = params.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let mut members = Vec::new();
    if let Some(arr) = params.get("members").and_then(|v| v.as_array()) {
        for m in arr {
            let agent_id = m.get("agent_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if agent_id.is_empty() {
                continue;
            }
            let role_str = m.get("role").and_then(|v| v.as_str()).unwrap_or("Builder");
            let role = match role_str {
                "Lead" | "LeadArchitect" => WorkgroupRole::LeadArchitect,
                "Critic" | "SanityVerifier" => WorkgroupRole::SanityVerifier,
                "ToolExecutor" | "CLI" => WorkgroupRole::ToolExecutor,
                _ => WorkgroupRole::PrimaryBuilder,
            };
            let node_id = m.get("target_node").and_then(|v| v.as_str()).unwrap_or("PC-1").to_string();
            let display_name = m.get("display_name").and_then(|v| v.as_str()).unwrap_or(&agent_id).to_string();
            members.push(WorkgroupMember {
                agent_id,
                display_name,
                node_id,
                role,
                parent_agent_id: None,
            });
        }
    }

    let trigger_keywords: Vec<String> = params
        .get("trigger_keywords")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let store = global_workgroups();
    let mut guard = store.write().await;

    let existing = guard.groups.get(&id).cloned();
    let (icon, color, execution_strategy, markdown_rules) = match existing {
        Some(ex) => (ex.icon, ex.color, ex.execution_strategy, ex.markdown_rules),
        None => ("folder".to_string(), "#4285F4".to_string(), "autonomous".to_string(), Vec::new()),
    };

    let wg = ProblemWorkgroup {
        id: id.clone(),
        name,
        problem_domain,
        icon,
        color,
        description,
        execution_strategy,
        members,
        markdown_rules,
        trigger_keywords,
        tasks_completed: 0,
    };

    guard.upsert(wg.clone());
    let _ = guard.save_to_disk();

    (
        true,
        json!({
            "status": "upserted",
            "workgroup_id": id,
            "workgroup": wg
        }),
        None,
    )
}

/// Attaches a Markdown (.md) rule file or inline specification to a Problem Workgroup.
pub async fn exec_attach_workgroup_rule(params: &Value) -> (bool, Value, Option<String>) {
    let workgroup_id = match params.get("workgroup_id").and_then(|v| v.as_str()) {
        Some(w) if !w.trim().is_empty() => w.to_string(),
        _ => return (false, json!({}), Some("Missing required parameter: 'workgroup_id'".to_string())),
    };
    let title = match params.get("title").and_then(|v| v.as_str()) {
        Some(t) if !t.trim().is_empty() => t.to_string(),
        _ => return (false, json!({}), Some("Missing required parameter: 'title'".to_string())),
    };

    let file_path = params.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string());
    let inline_markdown = params.get("inline_markdown").and_then(|v| v.as_str()).map(|s| s.to_string());
    let enforce_strict = params.get("enforce_strict").and_then(|v| v.as_bool()).unwrap_or(true);

    let rule_id = {
        let slug: String = title
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        format!("rule-{}", slug.trim_matches('-'))
    };

    let rule = MarkdownRuleAttachment {
        id: rule_id.clone(),
        title: title.clone(),
        file_path,
        inline_markdown,
        enforce_strict,
    };

    let store = global_workgroups();
    let mut guard = store.write().await;
    match guard.attach_rule(&workgroup_id, rule) {
        Some(updated_wg) => {
            let _ = guard.save_to_disk();
            (
                true,
                json!({
                    "status": "rule_attached",
                    "workgroup_id": workgroup_id,
                    "rule_id": rule_id,
                    "rule_title": title,
                    "total_rules": updated_wg.markdown_rules.len()
                }),
                None,
            )
        }
        None => (false, json!({}), Some(format!("Workgroup '{}' not found", workgroup_id))),
    }
}

/// Probes a specific TCP port with a timeout and measures round-trip latency in milliseconds.
async fn probe_port(ip: &str, port: u16, timeout_ms: u64) -> (bool, f64) {
    let t0 = std::time::Instant::now();
    let addr = format!("{}:{}", ip, port);
    let connect_future = tokio::net::TcpStream::connect(&addr);
    match tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), connect_future).await {
        Ok(Ok(_stream)) => (true, t0.elapsed().as_secs_f64() * 1000.0),
        _ => (false, t0.elapsed().as_secs_f64() * 1000.0),
    }
}

/// Probes the cluster ports of a single node and returns a structured diagnostic report.
async fn check_single_worker_health(target_ip: &str, lead_url: &str) -> Value {
    let (gw_open, gw_lat) = probe_port(target_ip, 8080, 1000).await;
    let (hermes_open, hermes_lat) = probe_port(target_ip, 8642, 1000).await;
    let (ollama_open, ollama_lat) = probe_port(target_ip, 11434, 1000).await;

    let (status, verdict, diagnostics, next_action) = if gw_open {
        (
            "ONLINE",
            "Gateway Active",
            "AgentTrace Edge Gateway is listening and active on :8080.".to_string(),
            "Node is ready for decentralized task execution and cryptographic telemetry signing.".to_string(),
        )
    } else if hermes_open || ollama_open {
        let engine = if hermes_open { "Hermes Agent (:8642)" } else { "Ollama (:11434)" };
        (
            "GATEWAY_MISSING",
            "Inference Engine Online, Gateway Missing",
            format!("{} is reachable, but the AgentTrace Edge Gateway (:8080) is not yet active on this node.", engine),
            format!("Run the bootstrap command on the target machine: irm {}/bootstrap.ps1 | iex", lead_url.trim_end_matches('/')),
        )
    } else {
        (
            "OFFLINE",
            "Unreachable",
            "No cluster ports (:8080, :8642, :11434) responded within 1000ms.".to_string(),
            "Verify the worker node is powered on, connected to LAN, and firewalls permit inbound traffic.".to_string(),
        )
    };

    json!({
        "target_ip": target_ip,
        "status": status,
        "verdict": verdict,
        "diagnostics": diagnostics,
        "recommended_action": next_action,
        "ports": {
            "8080_gateway": { "open": gw_open, "latency_ms": (gw_lat * 10.0).round() / 10.0 },
            "8642_hermes": { "open": hermes_open, "latency_ms": (hermes_lat * 10.0).round() / 10.0 },
            "11434_ollama": { "open": ollama_open, "latency_ms": (ollama_lat * 10.0).round() / 10.0 }
        },
        "bootstrap_command": format!("irm {}/bootstrap.ps1 | iex", lead_url.trim_end_matches('/'))
    })
}

/// Checks network reachability, engine health, and gateway presence across worker nodes.
pub async fn exec_check_worker_connection(
    state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let lead_ip = crate::services::discover_local_lan_ip().await;
    let lead_url = format!("http://{}:8080", lead_ip);

    let target_ip = params.get("target_ip").and_then(|v| v.as_str()).unwrap_or("all");

    if target_ip.is_empty() || target_ip.eq_ignore_ascii_case("all") {
        let mut ips: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        if let Ok(nodes) = state.db.get_all_nodes() {
            for n in nodes {
                if !n.ip_address.is_empty() && n.ip_address != "127.0.0.1" && n.ip_address != lead_ip {
                    ips.insert(n.ip_address);
                }
            }
        }
        ips.insert("192.168.1.102".to_string());

        let mut reports = Vec::new();
        for ip in &ips {
            reports.push(check_single_worker_health(ip.as_str(), &lead_url).await);
        }

        (
            true,
            json!({
                "lead_ip": lead_ip,
                "lead_url": lead_url,
                "checked_nodes_count": reports.len(),
                "reports": reports
            }),
            None,
        )
    } else {
        let rep = check_single_worker_health(target_ip, &lead_url).await;
        (true, rep, None)
    }
}

/// Performs an autonomous health check across all cluster nodes and reports sleep mode, offline status, and latency.
pub async fn exec_check_cluster_health(
    state: &AgentInterfaceState,
    _params: &Value,
) -> (bool, Value, Option<String>) {
    let wd = crate::cluster_health::ClusterHealthWatchdog::new();
    let snap = wd.probe_all(&state.db, &state.registry, None).await;
    (true, serde_json::to_value(snap).unwrap_or(serde_json::json!({})), None)
}

/// Generates the exact 1-liner network bootstrap command dynamically computed for the current Lead IP.
pub async fn exec_get_cluster_bootstrap_command(
    _state: &AgentInterfaceState,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let lead_ip = crate::services::discover_local_lan_ip().await;
    let lead_url = format!("http://{}:8080", lead_ip);
    let platform = params.get("platform").and_then(|v| v.as_str()).unwrap_or("all");

    let ps1_cmd = format!("irm {}/bootstrap.ps1 | iex", lead_url);
    let bash_cmd = format!("curl -sSL {}/install | python3 - --worker", lead_url);

    let res = match platform.to_lowercase().as_str() {
        "powershell" | "windows" => json!({
            "lead_ip": lead_ip,
            "lead_url": lead_url,
            "powershell_command": ps1_cmd,
            "instructions": "Run this command in an Administrator PowerShell window on any Windows machine on the same LAN."
        }),
        "bash" | "linux" | "macos" => json!({
            "lead_ip": lead_ip,
            "lead_url": lead_url,
            "bash_command": bash_cmd,
            "instructions": "Run this command in a terminal on any Linux or macOS machine on the same LAN."
        }),
        _ => json!({
            "lead_ip": lead_ip,
            "lead_url": lead_url,
            "powershell_command": ps1_cmd,
            "bash_command": bash_cmd,
            "instructions": "Run the PowerShell command on Windows or the bash command on Linux/macOS to spread the network."
        }),
    };

    (true, res, None)
}

/// Triggers zero-walk Over-the-Air (OTA) binary distribution and remote push provisioning.
pub async fn exec_deploy_cluster_worker(
    state: &AgentInterfaceState,
    agent_id: &str,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let target_ip = match params.get("target_ip").and_then(|v| v.as_str()) {
        Some(ip) if !ip.trim().is_empty() => ip.trim().to_string(),
        _ => return (false, json!({}), Some("Missing required parameter: 'target_ip'".to_string())),
    };
    let port = params.get("port").and_then(|v| v.as_u64()).unwrap_or(8642) as u16;
    let lead_ip = crate::services::discover_local_lan_ip().await;
    let lead_url = format!("http://{}:8080", lead_ip);

    let (gw_open, _) = probe_port(&target_ip, 8080, 600).await;
    if gw_open {
        return (
            true,
            json!({
                "status": "already_running",
                "target_ip": target_ip,
                "port": 8080,
                "details": format!("AgentTrace Edge Gateway is already running and active on {}:8080.", target_ip),
                "bootstrap_command": format!("irm {}/bootstrap.ps1 | iex", lead_url)
            }),
            None,
        );
    }

    let trigger_url = format!("http://{}:{}/api/v1/system/bootstrap", target_ip, port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let remote_res = client
        .post(&trigger_url)
        .json(&json!({
            "action": "ota_bootstrap",
            "bootstrap_url": format!("{}/bootstrap.ps1", lead_url),
            "lead_url": lead_url
        }))
        .send()
        .await;

    let (delivered, details) = match remote_res {
        Ok(res) if res.status().is_success() => (true, format!("Triggered remote worker bootstrap via {}", trigger_url)),
        Ok(res) => (false, format!("Target returned HTTP status {}. Use 1-line bootstrap command to spread the network.", res.status())),
        Err(e) => (false, format!("Direct trigger to {} failed/timed out: {}. Run bootstrap command on worker.", trigger_url, e)),
    };

    let mut deploy_event = TraceEvent {
        event_id: uuid::Uuid::new_v4().to_string(),
        node_id: "LEAD".to_string(),
        session_id: "cluster_provisioning".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_type: EventType::DecisionStep,
        target: format!("cluster_worker_deploy:{}", target_ip),
        action_details: json!({
            "target_ip": target_ip,
            "port": port,
            "delivered": delivered,
            "initiated_by_agent": agent_id,
            "details": details
        }),
        diff: None,
        provenance_sources: vec![agent_id.to_string()],
        security_verdict: SecurityVerdict::Allowed,
        security_reasons: vec!["OTA Worker Provisioning".to_string()],
        duration_ms: 0.0,
        prev_hash: "0".repeat(64),
        event_hash: "0".repeat(64),
        signature: None,
        public_key: None,
    };
    let _ = state.db.insert_event_signed(&mut deploy_event, Some(&*state.identity));

    (
        true,
        json!({
            "status": if delivered { "triggered" } else { "pending_manual_trigger" },
            "target_ip": target_ip,
            "port": port,
            "delivered_automatically": delivered,
            "details": details,
            "bootstrap_command": format!("irm {}/bootstrap.ps1 | iex", lead_url)
        }),
        None,
    )
}

/// Dispatches prompt to the cluster via Central Prompt Hub at zero external token cost.
pub async fn exec_dispatch_cluster_prompt(
    state: &AgentInterfaceState,
    agent_id: &str,
    params: &Value,
) -> (bool, Value, Option<String>) {
    let prompt = match params.get("prompt").and_then(|v| v.as_str()) {
        Some(p) if !p.trim().is_empty() => p,
        _ => return (false, json!({}), Some("Missing required parameter: 'prompt'".to_string())),
    };

    let workgroup_id = params.get("workgroup_id").and_then(|v| v.as_str()).unwrap_or("auto");
    let mode = params.get("mode").and_then(|v| v.as_str()).unwrap_or("auto");
    let requested_model = params.get("requested_model").and_then(|v| v.as_str()).map(|s| s.to_string());

    let payload = json!({
        "prompt": prompt,
        "mode": mode,
        "workgroup_id": workgroup_id,
        "requested_model": requested_model,
        "agent_id": agent_id,
        "unload_heavy_after": true
    });

    let client = reqwest::Client::new();
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let url = format!("http://127.0.0.1:{}/api/v1/hub/dispatch", port);

    let mut req_builder = client.post(&url).json(&payload);
    if let Some(ref tok) = state.security.auth_token {
        req_builder = req_builder.header("X-AgentTrace-Token", tok);
    }

    match req_builder.send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<Value>().await {
                    Ok(body) => (true, body, None),
                    Err(e) => (false, json!({}), Some(format!("Invalid response JSON from hub: {}", e))),
                }
            } else {
                let status = resp.status();
                let txt = resp.text().await.unwrap_or_default();
                (false, json!({}), Some(format!("Hub returned HTTP {}: {}", status, txt)))
            }
        }
        Err(e) => (false, json!({}), Some(format!("Failed to reach prompt hub at {}: {}", url, e))),
    }
}
