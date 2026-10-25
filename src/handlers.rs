use crate::{
    agent_interface::{self, AgentInterfaceState},
    analytics,
    db::Database,
    ledger::{self, NodeIdentity},
    policy::{self, AccessAction, PolicyDecision, PolicyEngine},
    proxy::{handle_chat_completions, handle_models, ProxyConfig},
    registry::ClusterModelRegistry,
    schema::{self, NodeHeartbeat, SecurityVerdict, TraceEvent},
    secrets::ClusterSecurityConfig,
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, Response,
    },
    routing::{delete, get, post, put},
    Json, Router,
};
use futures_util::stream::Stream;
use std::{convert::Infallible, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::CorsLayer;

const DASHBOARD_HTML: &str = include_str!("dashboard.html");
const POLICIES_HTML: &str = include_str!("policies.html");
const WORKGROUPS_HTML: &str = include_str!("workgroups.html");
const CONSOLE_HTML: &str = include_str!("console.html");
const GRAPH_HTML: &str = include_str!("graph.html");
const FLYER_HTML: &str = include_str!("../docs/flyer.html");
const INSTALL_PS1: &str = include_str!("install.ps1");
const PROVISION_PS1: &str = include_str!("provision.ps1");
const INSTALL_SH: &str = include_str!("install.sh");
const PROVISION_SH: &str = include_str!("provision.sh");
const INSTALL_PY: &str = include_str!("../install.py");

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub proxy_cfg: Arc<ProxyConfig>,
    pub tx_events: broadcast::Sender<String>,
    pub security: Arc<ClusterSecurityConfig>,
    pub identity: Arc<NodeIdentity>,
    pub lead_url: Option<String>,
    #[allow(dead_code)]
    pub node_id: String,
    pub registry: Arc<RwLock<ClusterModelRegistry>>,
    pub policy: Arc<RwLock<PolicyEngine>>,
    pub ledger_writer: Option<crate::ledger_writer::LedgerWriter>,
    pub health_watchdog: Option<Arc<crate::cluster_health::ClusterHealthWatchdog>>,
}

/// Constructs the complete Axum Router for the AgentTrace Gateway.
pub fn create_router(state: AppState, agent_state: AgentInterfaceState) -> Router {
    let claude_state = crate::claude_bridge::ClaudeBridgeState {
        proxy_cfg: state.proxy_cfg.clone(),
        policy: state.policy.clone(),
        node_id: state.node_id.clone(),
    };

    Router::new()
        .route("/", get(serve_dashboard))
        .route("/dashboard", get(serve_dashboard))
        .route("/policies", get(serve_policies))
        .route("/workgroups", get(serve_workgroups))
        .route("/console", get(serve_console))
        .route("/terminal", get(serve_console))
        .route("/client", get(crate::client::serve_client))
        .route("/app", get(crate::client::serve_client))
        .route("/mobile", get(crate::mobile::serve_mobile))
        .route("/graph", get(serve_graph))
        .route("/flyer", get(serve_flyer))
        .route("/marketing", get(serve_flyer))
        .route("/install", get(serve_install_py))
        .route("/install.py", get(serve_install_py))
        .route("/install.ps1", get(serve_install_script))
        .route("/provision.ps1", get(serve_provision_script))
        .route("/install.sh", get(serve_install_sh))
        .route("/provision.sh", get(serve_provision_sh))
        .route("/bootstrap.ps1", get(crate::ota_dist::serve_bootstrap_ps1))
        .route("/winrm.ps1", get(crate::ota_dist::serve_winrm_ps1))
        .route("/winrm.cmd", get(crate::ota_dist::serve_winrm_cmd))
        .route("/worker.cmd", get(crate::ota_dist::serve_worker_cmd))
        .route("/join.cmd", get(crate::ota_dist::serve_worker_cmd))
        .route("/join", get(crate::ota_dist::serve_join_html))
        .route("/dist/agent_trace_gw.exe", get(crate::ota_dist::stream_binary))
        .route("/api/v1/dist/manifest", get(crate::ota_dist::get_manifest))
        .route("/api/v1/dist/agent_trace_gw.exe", get(crate::ota_dist::stream_binary))
        .route("/api/v1/cluster/deploy-worker", post(crate::ota_dist::deploy_worker))
        // OpenAI & Corporate Claude Code Terminal API Guard Proxies
        .route("/v1/chat/completions", post(handle_chat_completions).with_state(state.proxy_cfg.clone()))
        .route("/v1/messages", post(crate::claude_bridge::handle_anthropic_messages).with_state(claude_state))
        .route("/v1/messages/count_tokens", post(crate::claude_bridge::handle_anthropic_count_tokens))
        .route("/v1/models", get(handle_models).with_state(state.proxy_cfg.clone()))
        // Active AI Guard, Pre-Image CAS Guard, SIEM/ELK Export, Vault & Live Benchmarks
        .route("/api/v1/guard/intercept", post(guard_intercept_handler))
        .route("/api/v1/cas/status", get(crate::cas_guard::cas_status_handler))
        .route("/api/v1/cas/protect", post(crate::cas_guard::cas_protect_handler))
        .route("/api/v1/cas/enforce", post(crate::cas_guard::cas_enforce_handler))
        .route("/api/v1/export/elk", get(export_elk_handler))
        .route("/api/v1/benchmarks", get(get_benchmarks_handler))
        .route("/api/v1/vault/encrypt", post(encrypt_secret_handler))
        .route("/api/v1/hub/dispatch", post(crate::hub::handle_hub_dispatch).with_state(state.proxy_cfg.clone()))
        .route("/api/v1/cluster/availability", get(crate::hub::handle_cluster_availability).with_state(state.proxy_cfg.clone()))
        .route("/api/v1/cluster/health", get(get_cluster_health_handler))
        .route("/api/v1/cluster/health/probe", post(probe_cluster_health_handler))
        .route("/api/v1/events", post(ingest_event).get(get_events))
        .route("/api/v1/telemetry", get(get_events))
        .route("/api/v1/heartbeat", post(register_heartbeat))
        .route("/api/v1/summary", get(get_summary))
        .route("/api/v1/ledger/verify", get(verify_ledger_handler))
        .route("/api/v1/stream", get(sse_handler))
        // Telemetry Analytics, Contribution Graph, Alerts & Sandbox Policy Endpoints
        .route("/api/v1/telemetry/analytics", get(get_analytics_handler))
        .route("/api/v1/telemetry/contributions", get(get_contributions_handler))
        .route("/api/v1/alerts", get(get_alerts_handler))
        .route("/api/v1/alerts/ack", post(ack_alert_handler))
        .route("/api/v1/alerts/reset", post(reset_alerts_handler))
        .route("/api/v1/policy", get(list_policies_handler).post(set_policy_handler))
        .route("/api/v1/policy/:agent_id", delete(delete_policy_handler))
        .route("/api/v1/policy/evaluate", post(evaluate_policy_handler))
        .route("/api/v1/zones", get(list_zones_handler).post(upsert_zone_handler))
        .route("/api/v1/zones/:zone_id", delete(delete_zone_handler))
        .route("/api/v1/zones/confine", post(confine_playground_handler))
        // Problem Workgroups & Attached Markdown (.md) Rules Endpoints
        .route("/api/v1/workgroups", get(crate::workgroups::list_workgroups_handler).post(crate::workgroups::upsert_workgroup_handler))
        .route("/api/v1/workgroups/yaml", post(crate::workgroups::apply_workgroup_yaml_handler))
        .route("/api/v1/workgroups/:workgroup_id", delete(crate::workgroups::delete_workgroup_handler))
        .route("/api/v1/workgroups/:workgroup_id/yaml", get(crate::workgroups::get_workgroup_yaml_handler))
        .route("/api/v1/workgroups/:workgroup_id/rules", post(crate::workgroups::attach_markdown_rule_handler))
        .route("/api/v1/workgroups/:workgroup_id/rules/:rule_id", delete(crate::workgroups::remove_markdown_rule_handler))
        .route("/api/v1/workgroups/:workgroup_id/preview", get(crate::workgroups::preview_workgroup_prompt_handler))
        // Live Markdown (.md) Rule Reading & Editing Endpoints
        .route("/api/v1/rules/file", get(crate::workgroups::get_rule_file_handler).post(crate::workgroups::save_rule_file_handler))
        // Projects, Workspaces & Persistent Conversation Contexts
        .route("/api/v1/projects", get(crate::projects::list_projects_handler).post(crate::projects::register_project_handler))
        .route("/api/v1/projects/active", post(crate::projects::set_active_project_handler))
        .route("/api/v1/projects/:project_id", put(crate::projects::update_project_handler))
        .route("/api/v1/projects/:project_id/conversations", get(crate::projects::list_project_conversations_handler))
        .route("/api/v1/conversations", post(crate::projects::create_conversation_handler))
        .route("/api/v1/conversations/:id", get(crate::projects::get_conversation_handler).put(crate::projects::update_conversation_handler).delete(crate::projects::delete_conversation_handler))
        .route("/api/v1/conversations/:id/messages", post(crate::projects::append_message_handler))
        // Artifacts Management Endpoints (Workspace & Antigravity Brain)
        .route("/api/v1/artifacts", get(crate::artifacts::list_artifacts_handler))
        .route("/api/v1/artifacts/:id", get(crate::artifacts::get_artifact_handler))
        .route("/api/v1/artifacts/:id/view", get(crate::artifacts::view_artifact_handler))
        // Background Tasks Observability & Management Endpoints
        .route("/api/v1/tasks", get(crate::tasks::list_tasks_handler))
        .route("/api/v1/tasks/:id", get(crate::tasks::get_task_handler))
        .route("/api/v1/tasks/:id/cancel", post(crate::tasks::cancel_task_handler))
        // Autonomous Keep-Busy Idle Worker Endpoints
        .route("/api/v1/idle_worker/status", get(crate::idle_worker::get_idle_worker_status_handler))
        .route("/api/v1/idle_worker/config", post(crate::idle_worker::update_idle_worker_config_handler))
        .route("/api/v1/idle_worker/trigger", post(crate::idle_worker::trigger_idle_task_handler))
        .route("/api/v1/idle_worker/user_activity", post(crate::idle_worker::record_user_activity_handler))
        .route("/api/v1/idle_worker/proposals/:proposal_id/dismiss", post(crate::idle_worker::dismiss_proposal_handler))
        .route("/api/v1/idle_worker/proposals/:proposal_id/apply", post(crate::idle_worker::apply_proposal_handler))
        // AI Agent Control, MCP & Dynamic Delegation Endpoints
        .route("/api/v1/agent/handoff", get(agent_interface::handle_agent_handoff).with_state(agent_state.clone()))
        .route("/api/v1/agent/tools", get(agent_interface::get_agent_tools))
        .route("/api/v1/agent/action", post(agent_interface::handle_agent_action).with_state(agent_state.clone()))
        .route("/api/v1/mcp", post(agent_interface::handle_mcp_rpc).with_state(agent_state.clone()))
        .route("/api/v1/delegate/decide", post(agent_interface::handle_delegate_decide).with_state(agent_state.clone()))
        .route("/api/v1/delegate/override", post(agent_interface::handle_delegate_override).with_state(agent_state.clone()))
        .route("/api/v1/delegate/overrides", get(agent_interface::handle_delegate_list_overrides).with_state(agent_state.clone()))
        .route("/api/v1/delegate/override/:rule_id", delete(agent_interface::handle_delegate_remove_override).with_state(agent_state.clone()))
        .route("/api/v1/delegate/config", get(agent_interface::handle_delegate_get_config).post(agent_interface::handle_delegate_set_config).with_state(agent_state))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn serve_install_py(headers: HeaderMap) -> Response {
    let host_header = headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("192.168.1.101:8080");
    let lead_ip = host_header.split(':').next().unwrap_or("192.168.1.101");
    let dynamic_script = INSTALL_PY.replace("192.168.1.101", lead_ip);

    Response::builder()
        .header("content-type", "text/plain; charset=utf-8")
        .body(Body::from(dynamic_script))
        .unwrap()
}

pub async fn serve_install_script(headers: HeaderMap) -> Response {
    let host_header = headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("192.168.1.101:8080");
    let lead_ip = host_header.split(':').next().unwrap_or("192.168.1.101");
    let dynamic_script = INSTALL_PS1.replace("192.168.1.101", lead_ip);

    Response::builder()
        .header("content-type", "text/plain; charset=utf-8")
        .body(Body::from(dynamic_script))
        .unwrap()
}

pub async fn serve_provision_script(headers: HeaderMap) -> Response {
    let host_header = headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("192.168.1.101:8080");
    let lead_ip = host_header.split(':').next().unwrap_or("192.168.1.101");
    let dynamic_script = PROVISION_PS1.replace("192.168.1.101", lead_ip);

    Response::builder()
        .header("content-type", "text/plain; charset=utf-8")
        .body(Body::from(dynamic_script))
        .unwrap()
}

pub async fn serve_install_sh(headers: HeaderMap) -> Response {
    let host_header = headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("192.168.1.101:8080");
    let lead_ip = host_header.split(':').next().unwrap_or("192.168.1.101");
    let dynamic_script = INSTALL_SH.replace("192.168.1.101", lead_ip);

    Response::builder()
        .header("content-type", "text/x-shellscript; charset=utf-8")
        .body(Body::from(dynamic_script))
        .unwrap()
}

pub async fn serve_provision_sh(headers: HeaderMap) -> Response {
    let host_header = headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("192.168.1.101:8080");
    let lead_ip = host_header.split(':').next().unwrap_or("192.168.1.101");
    let dynamic_script = PROVISION_SH.replace("192.168.1.101", lead_ip);

    Response::builder()
        .header("content-type", "text/x-shellscript; charset=utf-8")
        .body(Body::from(dynamic_script))
        .unwrap()
}

pub async fn serve_dist_binary() -> Result<Response, StatusCode> {
    let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("target/debug/agent_trace_gw.exe"));
    let bytes = tokio::fs::read(&exe_path).await.map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Response::builder()
        .header("content-type", "application/octet-stream")
        .header("content-disposition", "attachment; filename=\"agent_trace_gw.exe\"")
        .body(Body::from(bytes))
        .unwrap())
}

pub async fn serve_dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

pub async fn serve_policies() -> Html<&'static str> {
    Html(POLICIES_HTML)
}

pub async fn serve_workgroups() -> Html<&'static str> {
    Html(WORKGROUPS_HTML)
}

pub async fn serve_console() -> Html<&'static str> {
    Html(CONSOLE_HTML)
}

pub async fn serve_graph() -> Html<&'static str> {
    Html(GRAPH_HTML)
}

pub async fn serve_flyer() -> Html<&'static str> {
    Html(FLYER_HTML)
}

pub async fn ingest_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut event): Json<TraceEvent>,
) -> Result<StatusCode, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // 1. Evaluate Agent Sandbox / Whitelist Policy & Hard AI Guard
    if event.node_id.is_empty() || event.node_id == "antigravity" || event.node_id == "local-agent" {
        let ident = crate::agent_identity::AgentIdentity::from_headers_and_session(
            &headers,
            &state.node_id,
            &event.session_id,
            None,
        );
        event.node_id = ident.canonical_id;
    }

    let action = AccessAction::from_event_type(&event.event_type);
    let agent_id = if !event.node_id.is_empty() {
        &event.node_id
    } else {
        "*"
    };

    let decision = {
        let p_guard = state.policy.read().await;
        p_guard.evaluate(agent_id, action, &event.target)
    };

    if decision.verdict != SecurityVerdict::Allowed {
        event.security_verdict = decision.verdict.clone();
        for r in decision.reasons {
            if !event.security_reasons.contains(&r) {
                event.security_reasons.push(r);
            }
        }
    }

    // 1b. Hard AI Guard check for destructive shell commands
    if event.event_type == schema::EventType::CommandRun {
        let cmd_findings = crate::ai_guard::inspect_shell_command_hard(&event.target);
        if !cmd_findings.is_empty() {
            event.security_verdict = SecurityVerdict::Blocked;
            for f in cmd_findings {
                let reason = format!("[{}] {}", f.rule_id, f.description);
                if !event.security_reasons.contains(&reason) {
                    event.security_reasons.push(reason);
                }
            }
        }
    }

    // Cryptographically record event into the tamper-proof ledger (including blocked attempts for audit trail)
    if let Some(ref writer) = state.ledger_writer {
        event = writer.commit_acknowledged(event).await.map_err(|e| {
            eprintln!("[-] Error in ledger writer: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    } else {
        state.db.insert_event_signed(&mut event, Some(&*state.identity)).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let msg = serde_json::to_string(&event).unwrap_or_default();
        let _ = state.tx_events.send(msg);
    }

    // Ship to corporate SIEM / ELK (ECS 8.11) if configured
    crate::siem_exporter::ship_event_to_siem_async(state.proxy_cfg.client.clone(), event.clone());

    // If running on worker and lead_url is set, forward event to lead gateway with mTLS / HMAC signature
    if let Some(ref target_lead) = state.lead_url {
        let forward_url = format!("{}/api/v1/events", target_lead.trim_end_matches('/'));
        let client = state.proxy_cfg.client.clone();
        let ev_payload = event.clone();
        let tok = state.security.auth_token.clone();
        let worker_nid = state.node_id.clone();
        tokio::spawn(async move {
            let body_bytes = serde_json::to_vec(&ev_payload).unwrap_or_default();
            let now_epoch = chrono::Utc::now().timestamp();
            let mtls_cfg = crate::security_hardening::InterGatewayMtlsConfig::from_env();
            let mut req = client
                .post(&forward_url)
                .header("content-type", "application/json")
                .header("x-gateway-node-id", &worker_nid)
                .header("x-gateway-timestamp", now_epoch.to_string())
                .body(body_bytes.clone());
            if let Some(hmac_sig) = mtls_cfg.sign_inter_gateway_request(&worker_nid, now_epoch, &body_bytes) {
                req = req.header("x-gateway-hmac", hmac_sig);
            }
            if let Some(t) = tok {
                req = req.header("x-agenttrace-token", t);
            }
            let _ = req.send().await;
        });
    }

    if event.security_verdict == SecurityVerdict::Blocked {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(StatusCode::CREATED)
}

pub async fn get_analytics_handler(
    State(state): State<AppState>,
) -> Result<Json<analytics::TelemetryAnalyticsReport>, StatusCode> {
    state.db.get_telemetry_analytics().map(Json).map_err(|e| {
        eprintln!("[-] Error computing telemetry analytics: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

#[derive(serde::Deserialize, Default)]
pub struct ContributionsQuery {
    pub limit: Option<usize>,
}

pub async fn get_contributions_handler(
    State(state): State<AppState>,
    Query(q): Query<ContributionsQuery>,
) -> Result<Json<crate::contributions::ContributionGraphReport>, StatusCode> {
    let limit = q.limit.unwrap_or(1000).clamp(1, 5000);
    let raw_events = state
        .db
        .get_recent_events(limit)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let typed_events: Vec<TraceEvent> = raw_events
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    let report = crate::contributions::build_contribution_report(&typed_events);
    Ok(Json(report))
}

pub async fn list_policies_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<policy::AgentAccessPolicy>>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let p = state.policy.read().await;
    Ok(Json(p.list_policies()))
}

pub async fn set_policy_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(p): Json<policy::AgentAccessPolicy>,
) -> Result<Json<policy::AgentAccessPolicy>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut guard = state.policy.write().await;
    guard.set_policy(p.clone());
    Ok(Json(p))
}

pub async fn delete_policy_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(agent_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut guard = state.policy.write().await;
    let removed = guard.remove_policy(&agent_id);
    Ok(Json(serde_json::json!({ "agent_id": agent_id, "removed": removed })))
}

#[derive(serde::Deserialize)]
pub struct EvaluatePolicyRequest {
    pub agent_id: Option<String>,
    pub action: Option<String>,
    pub target: String,
}

pub async fn evaluate_policy_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<EvaluatePolicyRequest>,
) -> Result<Json<PolicyDecision>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let agent_id = req.agent_id.as_deref().unwrap_or("*");
    let act = match req.action.as_deref().unwrap_or("Read").to_lowercase().as_str() {
        "write" => AccessAction::Write,
        "execute" => AccessAction::Execute,
        "networkpull" | "network" => AccessAction::NetworkPull,
        _ => AccessAction::Read,
    };
    let guard = state.policy.read().await;
    let decision = guard.evaluate(agent_id, act, &req.target);
    Ok(Json(decision))
}

pub async fn list_zones_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<crate::zones::ContainmentZone>>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let guard = state.policy.read().await;
    Ok(Json(guard.list_zones()))
}

pub async fn upsert_zone_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(zone): Json<crate::zones::ContainmentZone>,
) -> Result<Json<crate::zones::ContainmentZone>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut guard = state.policy.write().await;
    let saved = guard.upsert_zone(zone);
    Ok(Json(saved))
}

pub async fn delete_zone_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(zone_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut guard = state.policy.write().await;
    let removed = guard.remove_zone(&zone_id);
    Ok(Json(serde_json::json!({ "zone_id": zone_id, "removed": removed })))
}

pub async fn confine_playground_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<crate::zones::PlaygroundConfinementRequest>,
) -> Result<Json<policy::AgentAccessPolicy>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut guard = state.policy.write().await;
    let updated = guard.set_playground_confinement(&req.agent_id, req.confined);
    Ok(Json(updated))
}

#[derive(serde::Deserialize)]
pub struct EventQuery {
    pub limit: Option<usize>,
    pub node_id: Option<String>,
    pub event_type: Option<String>,
}

pub async fn get_events(
    State(state): State<AppState>,
    Query(q): Query<EventQuery>,
) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    let limit = q.limit.unwrap_or(50).clamp(1, 500);

    // If running as a worker connected to a Lead Node, proxy events from the Lead Node so the UI shows full cluster events
    if let Some(ref target_lead) = state.lead_url {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(1500))
            .build()
            .unwrap_or_default();
        let mut lead_req_url = format!("{}/api/v1/events?limit={}", target_lead.trim_end_matches('/'), limit);
        if let Some(ref n) = q.node_id {
            lead_req_url.push_str(&format!("&node_id={}", n));
        }
        if let Some(ref et) = q.event_type {
            lead_req_url.push_str(&format!("&event_type={}", et));
        }
        if let Ok(resp) = client.get(&lead_req_url).send().await {
            if let Ok(lead_events) = resp.json::<Vec<serde_json::Value>>().await {
                return Ok(Json(lead_events));
            }
        }
    }

    let events = state
        .db
        .get_recent_events_filtered(limit, q.node_id.as_deref(), q.event_type.as_deref())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(events))
}

pub async fn register_heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(node): Json<NodeHeartbeat>,
) -> Result<StatusCode, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    state.db.upsert_node(&node).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(ref models) = node.models {
        let mut reg = state.registry.write().await;
        reg.register_models(models.clone());
    } else {
        let endpoint = format!("http://{}:8642", node.ip_address);
        let mut reg = state.registry.write().await;
        reg.register_single_model(&node.node_id, &endpoint, "hermes-agent", None);
    }

    Ok(StatusCode::OK)
}

pub async fn get_summary(
    State(state): State<AppState>,
) -> Result<Json<schema::ClusterSummary>, StatusCode> {
    // If running as a worker connected to a Lead Node, proxy summary from the Lead Node so dashboard displays the whole cluster
    if let Some(ref target_lead) = state.lead_url {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(1500))
            .build()
            .unwrap_or_default();
        if let Ok(resp) = client.get(format!("{}/api/v1/summary", target_lead.trim_end_matches('/'))).send().await {
            if let Ok(lead_summary) = resp.json::<schema::ClusterSummary>().await {
                return Ok(Json(lead_summary));
            }
        }
    }

    let summary = state.db.get_cluster_summary().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(summary))
}

pub async fn get_cluster_health_handler(
    State(state): State<AppState>,
) -> Result<Json<crate::cluster_health::ClusterHealthSnapshot>, StatusCode> {
    if let Some(ref watchdog) = state.health_watchdog {
        let snap = watchdog.get_cached_snapshot().await;
        Ok(Json(snap))
    } else {
        let wd = crate::cluster_health::ClusterHealthWatchdog::new();
        let snap = wd.probe_all(&state.db, &state.registry, Some(&state.tx_events)).await;
        Ok(Json(snap))
    }
}

pub async fn probe_cluster_health_handler(
    State(state): State<AppState>,
) -> Result<Json<crate::cluster_health::ClusterHealthSnapshot>, StatusCode> {
    if let Some(ref watchdog) = state.health_watchdog {
        let snap = watchdog.probe_all(&state.db, &state.registry, Some(&state.tx_events)).await;
        Ok(Json(snap))
    } else {
        let wd = crate::cluster_health::ClusterHealthWatchdog::new();
        let snap = wd.probe_all(&state.db, &state.registry, Some(&state.tx_events)).await;
        Ok(Json(snap))
    }
}

pub async fn verify_ledger_handler(
    State(state): State<AppState>,
) -> Result<Json<ledger::LedgerVerificationReport>, StatusCode> {
    if let Some(ref target_lead) = state.lead_url {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(1500))
            .build()
            .unwrap_or_default();
        if let Ok(resp) = client.get(format!("{}/api/v1/ledger/verify", target_lead.trim_end_matches('/'))).send().await {
            if let Ok(lead_report) = resp.json::<ledger::LedgerVerificationReport>().await {
                return Ok(Json(lead_report));
            }
        }
    }

    let report = state.db.verify_ledger().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(report))
}

pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.tx_events.subscribe();
    let stream = async_stream::stream! {
        while let Ok(msg) = rx.recv().await {
            yield Ok(Event::default().data(msg));
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(serde::Deserialize, Default)]
pub struct AlertsQuery {
    pub include_acked: Option<bool>,
}

pub async fn get_alerts_handler(
    State(state): State<AppState>,
    Query(q): Query<AlertsQuery>,
) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    let include_acked = q.include_acked.unwrap_or(false);
    let alerts = state
        .db
        .list_security_alerts(include_acked)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(alerts))
}

#[derive(serde::Deserialize)]
pub struct AckAlertRequest {
    pub event_id: String,
}

pub async fn ack_alert_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AckAlertRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let updated = state
        .db
        .acknowledge_alert(&req.event_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({
        "status": "acknowledged",
        "event_id": req.event_id,
        "updated": updated
    })))
}

#[derive(serde::Deserialize, Default)]
pub struct ResetAlertsRequest {
    pub restore: Option<bool>,
}

pub async fn reset_alerts_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ResetAlertsRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if req.restore.unwrap_or(false) {
        let count = state.db.restore_all_alerts().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(Json(serde_json::json!({ "status": "restored", "restored_count": count })))
    } else {
        let count = state.db.acknowledge_all_alerts().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(Json(serde_json::json!({ "status": "reset", "acknowledged_count": count })))
    }
}

/// Synchronous Pre-Execution Hard AI Guard gate (`POST /api/v1/guard/intercept`).
pub async fn guard_intercept_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<crate::ai_guard::GuardInterceptRequest>,
) -> Result<(StatusCode, Json<crate::ai_guard::GuardInterceptionResult>), StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let result = {
        let p_guard = state.policy.read().await;
        crate::ai_guard::evaluate_hard_guard(&req, &p_guard, crate::ai_guard::GuardEnforcementMode::HardBlock)
    };

    // Record every guard interception into the cryptographic SHA-256 ledger
    let mut ev = TraceEvent {
        event_id: uuid::Uuid::new_v4().simple().to_string(),
        node_id: req.agent_id.clone(),
        session_id: "ai-guard-gate".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_type: if req.command.is_some() {
            schema::EventType::CommandRun
        } else if req.target_path.is_some() {
            schema::EventType::FileWrite
        } else {
            schema::EventType::LlmInference
        },
        target: req
            .command
            .clone()
            .or_else(|| req.target_path.clone())
            .unwrap_or_else(|| "AI Guard Pre-Execution Inspection".to_string()),
        action_details: serde_json::json!({
            "guard_mode": result.enforcement_mode,
            "hard_blocked": result.hard_blocked,
            "risk_score": result.risk_score,
            "findings": result.findings,
            "evaluation_latency_us": result.evaluation_latency_us,
        }),
        diff: None,
        provenance_sources: vec!["ai_guard".to_string()],
        security_verdict: result.verdict.clone(),
        security_reasons: result.reasons.clone(),
        duration_ms: (result.evaluation_latency_us as f64) / 1000.0,
        prev_hash: String::new(),
        event_hash: String::new(),
        signature: None,
        public_key: None,
    };

    if let Some(ref writer) = state.ledger_writer {
        let _ = writer.try_enqueue(ev.clone());
    } else {
        let _ = state.db.insert_event_signed(&mut ev, Some(&*state.identity));
    }
    crate::siem_exporter::ship_event_to_siem_async(state.proxy_cfg.client.clone(), ev);

    let status = if result.hard_blocked {
        StatusCode::FORBIDDEN
    } else {
        StatusCode::OK
    };
    Ok((status, Json(result)))
}

#[derive(serde::Deserialize, Default)]
pub struct ElkExportQuery {
    pub limit: Option<usize>,
    pub format: Option<String>,
    pub index: Option<String>,
}

/// Exports telemetry and guard events in Elastic Common Schema (ECS 8.11) `/_bulk` NDJSON or stream format (`GET /api/v1/export/elk`).
pub async fn export_elk_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ElkExportQuery>,
) -> Result<Response, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let limit = q.limit.unwrap_or(100).clamp(1, 2000);
    let raw_events = state
        .db
        .get_recent_events(limit)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let typed_events: Vec<TraceEvent> = raw_events
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    let fmt = q.format.as_deref().unwrap_or("bulk");
    let index = q.index.as_deref().unwrap_or("agenttrace-security-events");

    let ndjson = if fmt == "stream" {
        crate::siem_exporter::to_ecs_ndjson_stream(&typed_events)
    } else {
        crate::siem_exporter::to_elasticsearch_bulk_ndjson(&typed_events, index)
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/x-ndjson; charset=utf-8")
        .body(Body::from(ndjson))
        .unwrap())
}

/// Runs and returns live empirical latency & memory footprint benchmarks (`GET /api/v1/benchmarks`).
pub async fn get_benchmarks_handler() -> Json<crate::benchmarks::BenchmarkSuiteReport> {
    let report = tokio::task::spawn_blocking(crate::benchmarks::run_live_benchmarks)
        .await
        .unwrap_or_else(|_| crate::benchmarks::run_live_benchmarks());
    Json(report)
}

#[derive(serde::Deserialize)]
pub struct VaultEncryptRequest {
    pub plaintext: String,
}

/// Encrypts a secret value using the Gateway's ChaCha20-Poly1305 AEAD Vault (`POST /api/v1/vault/encrypt`).
pub async fn encrypt_secret_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<VaultEncryptRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let vault = crate::security_hardening::EncryptedSecretVault::load_default();
    let encrypted = vault
        .encrypt(&req.plaintext)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({
        "cipher": "ChaCha20-Poly1305 (AEAD)",
        "encrypted_value": encrypted
    })))
}

