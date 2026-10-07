mod antigravity_watcher;
mod db;
mod ledger;
mod proxy;
mod schema;
mod secrets;

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
use ledger::NodeIdentity;
use proxy::{handle_chat_completions, ProxyConfig};
use schema::{NodeHeartbeat, TraceEvent};
use secrets::ClusterSecurityConfig;
use std::{convert::Infallible, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
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
    security: Arc<ClusterSecurityConfig>,
    identity: Arc<NodeIdentity>,
    lead_url: Option<String>,
    #[allow(dead_code)]
    node_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    let is_worker_arg = args.iter().any(|a| a == "--worker");
    let mut cli_lead_url = None;
    for i in 0..args.len() {
        if args[i] == "--lead" && i + 1 < args.len() {
            cli_lead_url = Some(args[i + 1].clone());
        }
    }

    let role = std::env::var("ROLE")
        .unwrap_or_else(|_| if is_worker_arg { "worker".to_string() } else { "lead".to_string() });
    let is_worker = role.to_lowercase() == "worker";

    let lead_url = cli_lead_url.or_else(|| std::env::var("LEAD_URL").ok());

    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| {
        if is_worker { "worker_telemetry.db".to_string() } else { "cluster_telemetry.db".to_string() }
    });

    let default_upstream = if is_worker { "http://127.0.0.1:8642" } else { "http://127.0.0.1:11434" };
    let upstream_url = std::env::var("UPSTREAM_URL").unwrap_or_else(|_| default_upstream.to_string());
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);

    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "VECTOR".to_string());
    let default_node_id = if is_worker { format!("Worker-{}", hostname) } else { "PC-1".to_string() };
    let node_id = std::env::var("NODE_ID").unwrap_or(default_node_id);

    // 1. Load security configuration & secrets
    let security_cfg = Arc::new(ClusterSecurityConfig::load());

    // 2. Load or generate persistent Ed25519 cryptographic identity
    let identity = Arc::new(NodeIdentity::load_or_generate(&security_cfg.key_path)?);

    println!("============================================================");
    println!(" AGENT TRACE GATEWAY // CRYPTOGRAPHIC TELEMETRY LEDGER");
    println!("============================================================");
    println!(" Role:        {}", if is_worker { "Worker / Edge Node" } else { "Lead Node" });
    println!(" Node ID:     {}", node_id);
    println!(" Public Key:  {}...", &identity.public_key_hex()[..16]);
    println!(" Database:    {}", db_path);
    println!(" Upstream:    {}", upstream_url);
    if let Some(ref l) = lead_url {
        println!(" Lead Target: {}", l);
    }
    println!(" Listening:   http://0.0.0.0:{}", port);
    if !is_worker {
        println!(" Dashboard:   http://localhost:{}/dashboard", port);
    }
    println!(" Ledger API:  http://localhost:{}/api/v1/ledger/verify", port);
    println!("============================================================");

    let db = Database::new(&db_path)?;

    if !is_worker {
        // Register verified initial cluster topology in lead DB
        let _ = db.upsert_node(&NodeHeartbeat {
            node_id: "PC-1".to_string(),
            hostname: "VECTOR".to_string(),
            ip_address: "192.168.1.101".to_string(),
            role: "Lead Node (Antigravity + Hermes + Gemma 4)".to_string(),
            status: "ACTIVE".to_string(),
            last_seen: chrono::Utc::now().to_rfc3339(),
            public_key: Some(identity.public_key_hex().to_string()),
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
        let _ = db.delete_node("PC-3");
    }

    let (tx_events, _) = broadcast::channel(100);

    if !is_worker {
        // Spawn native autonomous Antigravity transcript watcher on Lead
        antigravity_watcher::spawn_watcher(db.clone(), tx_events.clone());
    }

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let proxy_cfg = Arc::new(ProxyConfig {
        upstream_url,
        db: db.clone(),
        security: security_cfg.clone(),
        identity: identity.clone(),
        client: client.clone(),
    });

    let state = AppState {
        db,
        proxy_cfg,
        tx_events,
        security: security_cfg.clone(),
        identity: identity.clone(),
        lead_url: lead_url.clone(),
        node_id: node_id.clone(),
    };

    // If running in Worker Mode, start background Heartbeat & Telemetry Forwarder to Lead
    if is_worker {
        if let Some(target_lead) = lead_url.clone() {
            let worker_nid = node_id.clone();
            let worker_host = hostname.clone();
            let worker_pubkey = identity.public_key_hex().to_string();
            let lead_client = client.clone();
            let lead_endpoint = target_lead.trim_end_matches('/').to_string();
            let sec_token = security_cfg.auth_token.clone();

            tokio::spawn(async move {
                let hb_url = format!("{}/api/v1/heartbeat", lead_endpoint);
                let heartbeat_payload = serde_json::json!({
                    "node_id": worker_nid,
                    "hostname": worker_host,
                    "ip_address": "127.0.0.1",
                    "role": "Worker Edge Gateway (Active)",
                    "status": "ACTIVE",
                    "last_seen": chrono::Utc::now().to_rfc3339(),
                    "public_key": worker_pubkey
                });

                loop {
                    let mut req = lead_client.post(&hb_url)
                        .header("content-type", "application/json")
                        .json(&heartbeat_payload);
                    if let Some(ref tok) = sec_token {
                        req = req.header("x-agenttrace-token", tok);
                    }
                    let _ = req.send().await;
                    tokio::time::sleep(Duration::from_secs(15)).await;
                }
            });
        }
    }

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

    if !is_worker {
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
                    tokio::time::sleep(Duration::from_secs(4)).await;
                }
            }
        });
    }

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
    headers: HeaderMap,
    Json(mut event): Json<TraceEvent>,
) -> Result<StatusCode, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    state.db.insert_event_signed(&mut event, Some(&*state.identity)).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let msg = serde_json::to_string(&event).unwrap_or_default();
    let _ = state.tx_events.send(msg);

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
    // Bound limit between 1 and 500 to prevent denial of service
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let events = state.db.get_recent_events(limit).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(events))
}

async fn register_heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(node): Json<NodeHeartbeat>,
) -> Result<StatusCode, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
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
