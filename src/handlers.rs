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
    routing::{delete, get, post},
    Json, Router,
};
use futures_util::stream::Stream;
use std::{convert::Infallible, path::PathBuf, sync::Arc};
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::CorsLayer;

const DASHBOARD_HTML: &str = include_str!("dashboard.html");
const CONSOLE_HTML: &str = include_str!("console.html");
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
}

/// Constructs the complete Axum Router for the AgentTrace Gateway.
pub fn create_router(state: AppState, agent_state: AgentInterfaceState) -> Router {
    Router::new()
        .route("/", get(serve_dashboard))
        .route("/dashboard", get(serve_dashboard))
        .route("/console", get(serve_console))
        .route("/terminal", get(serve_console))
        .route("/flyer", get(serve_flyer))
        .route("/marketing", get(serve_flyer))
        .route("/install", get(serve_install_py))
        .route("/install.py", get(serve_install_py))
        .route("/install.ps1", get(serve_install_script))
        .route("/provision.ps1", get(serve_provision_script))
        .route("/install.sh", get(serve_install_sh))
        .route("/provision.sh", get(serve_provision_sh))
        .route("/dist/agent_trace_gw.exe", get(serve_dist_binary))
        .route("/v1/chat/completions", post(handle_chat_completions).with_state(state.proxy_cfg.clone()))
        .route("/v1/models", get(handle_models).with_state(state.proxy_cfg.clone()))
        .route("/api/v1/events", post(ingest_event).get(get_events))
        .route("/api/v1/heartbeat", post(register_heartbeat))
        .route("/api/v1/summary", get(get_summary))
        .route("/api/v1/ledger/verify", get(verify_ledger_handler))
        .route("/api/v1/stream", get(sse_handler))
        // Telemetry Analytics, Alerts & Sandbox Policy Endpoints
        .route("/api/v1/telemetry/analytics", get(get_analytics_handler))
        .route("/api/v1/alerts", get(get_alerts_handler))
        .route("/api/v1/alerts/ack", post(ack_alert_handler))
        .route("/api/v1/alerts/reset", post(reset_alerts_handler))
        .route("/api/v1/policy", get(list_policies_handler).post(set_policy_handler))
        .route("/api/v1/policy/:agent_id", delete(delete_policy_handler))
        .route("/api/v1/policy/evaluate", post(evaluate_policy_handler))
        .route("/api/v1/zones", get(list_zones_handler).post(upsert_zone_handler))
        .route("/api/v1/zones/:zone_id", delete(delete_zone_handler))
        .route("/api/v1/zones/confine", post(confine_playground_handler))
        // AI Agent Control, MCP & Dynamic Delegation Endpoints
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

pub async fn serve_console() -> Html<&'static str> {
    Html(CONSOLE_HTML)
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

    // 1. Evaluate Agent Sandbox / Whitelist Policy
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

    // If running on worker and lead_url is set, forward event to lead gateway asynchronously
    if let Some(ref target_lead) = state.lead_url {
        let forward_url = format!("{}/api/v1/events", target_lead.trim_end_matches('/'));
        let client = state.proxy_cfg.client.clone();
        let ev_payload = event.clone();
        let tok = state.security.auth_token.clone();
        tokio::spawn(async move {
            let mut req = client.post(&forward_url).json(&ev_payload);
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
    let summary = state.db.get_cluster_summary().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(summary))
}

pub async fn verify_ledger_handler(
    State(state): State<AppState>,
) -> Result<Json<ledger::LedgerVerificationReport>, StatusCode> {
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

