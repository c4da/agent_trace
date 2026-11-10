//! HTTP router, security middleware and core telemetry/ledger handlers.

use crate::{
    agent_interface::{self, AgentInterfaceState},
    db::Database,
    ledger::{self, NodeIdentity},
    proxy::{handle_chat_completions, handle_models, ProxyConfig},
    schema::{self, TraceEvent},
    secrets::{ClusterSecurityConfig, AUTH_COOKIE_NAME},
};
use axum::{
    extract::{Query, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse, Redirect, Response,
    },
    routing::{delete, get, post},
    Json, Router,
};
use futures_util::stream::Stream;
use std::{convert::Infallible, sync::Arc};
use tokio::sync::broadcast;

const DASHBOARD_HTML: &str = include_str!("dashboard.html");
const CONSOLE_HTML: &str = include_str!("console.html");

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub proxy_cfg: Arc<ProxyConfig>,
    pub tx_events: broadcast::Sender<String>,
    pub security: Arc<ClusterSecurityConfig>,
    pub identity: Arc<NodeIdentity>,
    pub ledger_writer: Option<crate::ledger_writer::LedgerWriter>,
    /// True when bound to a loopback address; enables the DNS-rebinding Host check.
    pub loopback_only: bool,
}

/// Paths reachable without a token (static pages carry no data).
fn is_public_path(path: &str) -> bool {
    matches!(path, "/" | "/dashboard" | "/console" | "/login")
}

fn is_loopback_host(host: &str) -> bool {
    let name = if let Some(rest) = host.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host)
    };
    matches!(name, "localhost" | "127.0.0.1" | "::1")
}

/// Rejects DNS-rebinding (non-loopback Host while bound to loopback) and
/// unauthenticated requests to anything but the static pages.
async fn security_middleware(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if state.loopback_only {
        let host = req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("");
        if !is_loopback_host(host) {
            return (StatusCode::MISDIRECTED_REQUEST, "host not allowed").into_response();
        }
    }
    if !is_public_path(req.uri().path()) && !state.security.is_authorized(req.headers()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(req).await
}

/// Constructs the Axum router. No CORS layer: the UI is same-origin, and
/// cross-origin browser access is intentionally denied.
pub fn create_router(state: AppState, agent_state: AgentInterfaceState) -> Router {
    Router::new()
        .route("/", get(serve_dashboard))
        .route("/dashboard", get(serve_dashboard))
        .route("/console", get(serve_console))
        .route("/login", get(login_handler))
        // OpenAI-compatible proxy
        .route("/v1/chat/completions", post(handle_chat_completions).with_state(state.proxy_cfg.clone()))
        .route("/v1/models", get(handle_models).with_state(state.proxy_cfg.clone()))
        // DLP
        .route("/api/v1/dlp/scan", post(dlp_scan_handler))
        // Telemetry & ledger
        .route("/api/v1/events", post(ingest_event).get(get_events))
        .route("/api/v1/summary", get(get_summary))
        .route("/api/v1/savings", get(get_savings))
        .route("/api/v1/ledger/verify", get(verify_ledger_handler))
        .route("/api/v1/stream", get(sse_handler))
        .route("/api/v1/alerts", get(get_alerts_handler))
        .route("/api/v1/alerts/ack", post(ack_alert_handler))
        .route("/api/v1/alerts/reset", post(reset_alerts_handler))
        // Agent interface (MCP + delegation)
        .route("/api/v1/agent/handoff", get(agent_interface::handle_agent_handoff).with_state(agent_state.clone()))
        .route("/api/v1/agent/tools", get(agent_interface::get_agent_tools))
        .route("/api/v1/agent/action", post(agent_interface::handle_agent_action).with_state(agent_state.clone()))
        .route("/api/v1/mcp", post(agent_interface::handle_mcp_rpc).with_state(agent_state.clone()))
        .route("/api/v1/delegate/decide", post(agent_interface::handle_delegate_decide).with_state(agent_state.clone()))
        .route("/api/v1/delegate/override", post(agent_interface::handle_delegate_override).with_state(agent_state.clone()))
        .route("/api/v1/delegate/overrides", get(agent_interface::handle_delegate_list_overrides).with_state(agent_state.clone()))
        .route("/api/v1/delegate/override/:rule_id", delete(agent_interface::handle_delegate_remove_override).with_state(agent_state.clone()))
        .route("/api/v1/delegate/config", get(agent_interface::handle_delegate_get_config).post(agent_interface::handle_delegate_set_config).with_state(agent_state))
        .layer(middleware::from_fn_with_state(state.clone(), security_middleware))
        .with_state(state)
}

pub async fn serve_dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

pub async fn serve_console() -> Html<&'static str> {
    Html(CONSOLE_HTML)
}

#[derive(serde::Deserialize)]
pub struct LoginQuery {
    pub token: String,
}

/// `GET /login?token=...` — sets an HttpOnly, SameSite=Strict cookie for the browser UI.
pub async fn login_handler(State(state): State<AppState>, Query(q): Query<LoginQuery>) -> Response {
    let mut probe = HeaderMap::new();
    if let Ok(v) = format!("Bearer {}", q.token).parse() {
        probe.insert(header::AUTHORIZATION, v);
    }
    if !state.security.is_authorized(&probe) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let cookie = format!("{}={}; HttpOnly; SameSite=Strict; Path=/", AUTH_COOKIE_NAME, q.token);
    ([(header::SET_COOKIE, cookie)], Redirect::to("/console")).into_response()
}

#[derive(serde::Deserialize)]
pub struct DlpScanRequest {
    pub text: String,
}

/// `POST /api/v1/dlp/scan` — returns findings and the redacted text.
pub async fn dlp_scan_handler(Json(req): Json<DlpScanRequest>) -> Json<serde_json::Value> {
    let (mut findings, redacted) = crate::dlp::scan_and_redact_secrets(&req.text);
    findings.extend(crate::dlp::scan_prompt_injection(&req.text));
    Json(serde_json::json!({ "findings": findings, "redacted": redacted }))
}

/// `POST /api/v1/events` — records an externally reported event (e.g. from an agent hook).
pub async fn ingest_event(
    State(state): State<AppState>,
    Json(mut event): Json<TraceEvent>,
) -> Result<StatusCode, StatusCode> {
    // Never persist secrets reported by agents.
    event.target = crate::dlp::scan_and_redact_secrets(&event.target).1;
    if let Ok(details) = serde_json::to_string(&event.action_details) {
        let (_, clean) = crate::dlp::scan_and_redact_secrets(&details);
        event.action_details = serde_json::from_str(&clean).unwrap_or(event.action_details);
    }

    if let Some(ref writer) = state.ledger_writer {
        writer.commit_acknowledged(event).await.map_err(|e| {
            tracing::error!("ledger writer: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    } else {
        let db = state.db.clone();
        let identity = state.identity.clone();
        let committed = tokio::task::spawn_blocking(move || {
            db.insert_event_signed(&mut event, Some(&*identity)).map(|_| event)
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let _ = state.tx_events.send(serde_json::to_string(&committed).unwrap_or_default());
    }
    Ok(StatusCode::CREATED)
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
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || db.get_recent_events_filtered(limit, q.node_id.as_deref(), q.event_type.as_deref()))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_summary(State(state): State<AppState>) -> Result<Json<schema::ClusterSummary>, StatusCode> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || db.get_cluster_summary())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// `GET /api/v1/savings` — tokens served by local/cluster models instead of a paid API.
pub async fn get_savings(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || db.local_token_usage())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn verify_ledger_handler(
    State(state): State<AppState>,
) -> Result<Json<ledger::LedgerVerificationReport>, StatusCode> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || db.verify_ledger())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn sse_handler(State(state): State<AppState>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.tx_events.subscribe();
    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(msg) => yield Ok(Event::default().data(msg)),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
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
    state
        .db
        .list_security_alerts(q.include_acked.unwrap_or(false))
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(serde::Deserialize)]
pub struct AckAlertRequest {
    pub event_id: String,
}

pub async fn ack_alert_handler(
    State(state): State<AppState>,
    Json(req): Json<AckAlertRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let updated = state.db.acknowledge_alert(&req.event_id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "status": "acknowledged", "event_id": req.event_id, "updated": updated })))
}

#[derive(serde::Deserialize, Default)]
pub struct ResetAlertsRequest {
    pub restore: Option<bool>,
}

pub async fn reset_alerts_handler(
    State(state): State<AppState>,
    Json(req): Json<ResetAlertsRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if req.restore.unwrap_or(false) {
        let count = state.db.restore_all_alerts().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(Json(serde_json::json!({ "status": "restored", "restored_count": count })))
    } else {
        let count = state.db.acknowledge_all_alerts().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(Json(serde_json::json!({ "status": "reset", "acknowledged_count": count })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_host_check() {
        for ok in ["localhost:8080", "127.0.0.1:8080", "[::1]:8080", "localhost"] {
            assert!(is_loopback_host(ok), "{ok}");
        }
        for bad in ["evil.example:8080", "127.0.0.1.evil.example", "192.168.1.5:8080", ""] {
            assert!(!is_loopback_host(bad), "{bad}");
        }
    }
}
