mod antigravity_watcher;
mod db;
mod ledger;
mod proxy;
mod schema;

use axum::{
    body::Body,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, Response,
    },
    routing::{get, post},
    Json, Router,
};
use db::Database;
use futures_util::stream::Stream;
use proxy::{handle_chat_completions, ProxyConfig};
use schema::{NodeHeartbeat, TraceEvent};
use std::{convert::Infallible, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

const DASHBOARD_HTML: &str = include_str!("dashboard.html");
const INSTALL_PS1: &str = include_str!("install.ps1");
const PROVISION_PS1: &str = include_str!("provision.ps1");
const INSTALL_SH: &str = include_str!("install.sh");
const PROVISION_SH: &str = include_str!("provision.sh");
const INSTALL_PY: &str = include_str!("../install.py");

#[derive(Clone)]
struct AppState {
    db: Database,
    proxy_cfg: Arc<ProxyConfig>,
    tx_events: broadcast::Sender<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "cluster_telemetry.db".to_string());
    let upstream_url = std::env::var("UPSTREAM_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);

    println!("============================================================");
    println!(" AGENT TRACE GATEWAY // CRYPTOGRAPHIC TELEMETRY LEDGER");
    println!("============================================================");
    println!(" Database:    {}", db_path);
    println!(" Upstream:    {}", upstream_url);
    println!(" Listening:   http://0.0.0.0:{}", port);
    println!(" Dashboard:   http://localhost:{}/dashboard", port);
    println!(" Ledger API:  http://localhost:{}/api/v1/ledger/verify", port);
    println!("============================================================");

    let db = Database::new(&db_path)?;

    // Register verified 2-node cluster topology
    let _ = db.upsert_node(&NodeHeartbeat {
        node_id: "PC-1".to_string(),
        hostname: "VECTOR".to_string(),
        ip_address: "192.168.1.101".to_string(),
        role: "Lead Node (Antigravity + Hermes + Gemma 4)".to_string(),
        status: "ACTIVE".to_string(),
        last_seen: chrono::Utc::now().to_rfc3339(),
        public_key: None,
    });
    let _ = db.upsert_node(&NodeHeartbeat {
        node_id: "PC-2".to_string(),
        hostname: "NODE-02".to_string(),
        ip_address: "192.168.1.102".to_string(),
        role: "Worker Node (hermes-agent :8642 + CUDA)".to_string(),
        status: "ACTIVE".to_string(),
        last_seen: chrono::Utc::now().to_rfc3339(),
        public_key: None,
    });
    // Clean up obsolete PC-3 placeholder if existing in DB
    let _ = db.delete_node("PC-3");

    let (tx_events, _) = broadcast::channel(100);

    // Spawn native autonomous Antigravity transcript watcher (zero manual Python scripts needed)
    antigravity_watcher::spawn_watcher(db.clone(), tx_events.clone());

    let proxy_cfg = Arc::new(ProxyConfig {
        upstream_url,
        db: db.clone(),
    });

    let state = AppState {
        db,
        proxy_cfg,
        tx_events,
    };

    let app = Router::new()
        .route("/", get(serve_dashboard))
        .route("/dashboard", get(serve_dashboard))
        .route("/install", get(serve_install_py))
        .route("/install.py", get(serve_install_py))
        .route("/install.ps1", get(serve_install_script))
        .route("/provision.ps1", get(serve_provision_script))
        .route("/install.sh", get(serve_install_sh))
        .route("/provision.sh", get(serve_provision_sh))
        .route("/dist/agent_trace_gw.exe", get(serve_dist_binary))
        .route("/v1/chat/completions", post(handle_chat_completions).with_state(state.proxy_cfg.clone()))
        .route("/v1/models", get(proxy::handle_models).with_state(state.proxy_cfg.clone()))
        .route("/api/v1/events", post(ingest_event).get(get_events))
        .route("/api/v1/heartbeat", post(register_heartbeat))
        .route("/api/v1/summary", get(get_summary))
        .route("/api/v1/ledger/verify", get(verify_ledger_handler))
        .route("/api/v1/stream", get(sse_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Spawn UDP Network Discovery Beacon (UDP :8089 announces Lead Gateway to LAN every 4s)
    let lead_port = port;
    tokio::spawn(async move {
        if let Ok(socket) = tokio::net::UdpSocket::bind("0.0.0.0:0").await {
            let _ = socket.set_broadcast(true);
            let beacon = serde_json::json!({
                "service": "agent_trace_gw",
                "cluster": "vector-mesh",
                "role": "lead",
                "lead_ip": "192.168.1.101",
                "lead_port": lead_port
            }).to_string();
            let target = "255.255.255.255:8089";
            loop {
                let _ = socket.send_to(beacon.as_bytes(), target).await;
                tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
            }
        }
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn serve_install_py(headers: HeaderMap) -> Response {
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

async fn serve_install_script(headers: HeaderMap) -> Response {
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

async fn serve_provision_script(headers: HeaderMap) -> Response {
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

async fn serve_install_sh(headers: HeaderMap) -> Response {
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

async fn serve_provision_sh(headers: HeaderMap) -> Response {
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

async fn serve_dist_binary() -> Result<Response, StatusCode> {
    let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("target/debug/agent_trace_gw.exe"));
    let bytes = tokio::fs::read(&exe_path).await.map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Response::builder()
        .header("content-type", "application/octet-stream")
        .header("content-disposition", "attachment; filename=\"agent_trace_gw.exe\"")
        .body(Body::from(bytes))
        .unwrap())
}

async fn serve_dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

async fn ingest_event(
    State(state): State<AppState>,
    Json(mut event): Json<TraceEvent>,
) -> Result<StatusCode, StatusCode> {
    state.db.insert_event(&mut event).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let msg = serde_json::to_string(&event).unwrap_or_default();
    let _ = state.tx_events.send(msg);
    Ok(StatusCode::CREATED)
}

#[derive(serde::Deserialize)]
struct LimitQuery {
    limit: Option<usize>,
}

async fn get_events(
    State(state): State<AppState>,
    Query(q): Query<LimitQuery>,
) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    let limit = q.limit.unwrap_or(50);
    let events = state.db.get_recent_events(limit).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(events))
}

async fn register_heartbeat(
    State(state): State<AppState>,
    Json(node): Json<NodeHeartbeat>,
) -> Result<StatusCode, StatusCode> {
    state.db.upsert_node(&node).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::OK)
}

async fn get_summary(
    State(state): State<AppState>,
) -> Result<Json<schema::ClusterSummary>, StatusCode> {
    let summary = state.db.get_cluster_summary().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(summary))
}

async fn verify_ledger_handler(
    State(state): State<AppState>,
) -> Result<Json<ledger::LedgerVerificationReport>, StatusCode> {
    let report = state.db.verify_ledger().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(report))
}

async fn sse_handler(
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
