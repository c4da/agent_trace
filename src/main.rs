use agent_trace_gw::{
    agent_interface::AgentInterfaceState,
    db::Database,
    delegation_engine::{AgentDelegationEngine, RouteOverrideStore},
    dlp::DlpMode,
    handlers::{create_router, AppState},
    ledger::NodeIdentity,
    proxy::ProxyConfig,
    registry::ClusterModelRegistry,
    secrets::ClusterSecurityConfig,
    services,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::{broadcast, RwLock};

fn env_flag(name: &str) -> bool {
    std::env::var(name).map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    let allow_lan = args.iter().any(|a| a == "--insecure-lan" || a == "--lan") || env_flag("AGENTTRACE_ALLOW_INSECURE_LAN");

    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "cluster_telemetry.db".to_string());
    let upstream_url = std::env::var("UPSTREAM_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let bind_host = std::env::var("BIND_HOST").unwrap_or_else(|_| if allow_lan { "0.0.0.0" } else { "127.0.0.1" }.to_string());
    let addr: SocketAddr = format!("{}:{}", bind_host, port).parse()?;
    let loopback_only = addr.ip().is_loopback();
    let workers = services::parse_worker_upstreams(&agent_trace_gw::secrets::setting("WORKER_UPSTREAMS").unwrap_or_default());

    let security = Arc::new(ClusterSecurityConfig::load());
    if !loopback_only && security.auth_token.is_none() {
        eprintln!("Refusing to bind {} without AGENTTRACE_TOKEN: set a token before exposing the gateway on the network.", addr);
        std::process::exit(2);
    }

    let identity = Arc::new(NodeIdentity::load_or_generate(&security.key_path)?);
    let dlp_mode = DlpMode::from_env();

    println!("AgentTrace Gateway");
    println!("  Listening:  http://{}", addr);
    println!("  Auth:       {}", if security.auth_token.is_some() { "token required" } else { "off (loopback only)" });
    println!("  DLP mode:   {:?}", dlp_mode);
    println!("  Upstream:   {}", upstream_url);
    for (node, url) in &workers {
        println!("  Worker:     {} -> {}", node, url);
    }
    println!("  Database:   {}", db_path);
    println!("  Public key: {}...", &identity.public_key_hex()[..16]);
    println!("  Console:    http://localhost:{}/console", port);

    let db = Database::new(&db_path)?;
    let (tx_events, _) = broadcast::channel(256);

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(300))
        .build()?;

    let registry = Arc::new(RwLock::new(ClusterModelRegistry::new()));
    let override_store = Arc::new(RwLock::new(RouteOverrideStore::new()));
    let delegation_engine = Arc::new(AgentDelegationEngine::new(override_store.clone(), registry.clone(), upstream_url.clone()));

    let (ledger_writer, _writer_handle) = agent_trace_gw::ledger_writer::LedgerWriter::spawn(
        db.clone(),
        Some(identity.clone()),
        Some(tx_events.clone()),
        1024,
    );

    let proxy_cfg = Arc::new(ProxyConfig {
        upstream_url: upstream_url.clone(),
        db: db.clone(),
        security: security.clone(),
        identity: identity.clone(),
        client: client.clone(),
        registry: registry.clone(),
        delegation_engine: delegation_engine.clone(),
        ledger_writer: Some(ledger_writer.clone()),
        dlp_mode,
    });

    let state = AppState {
        db: db.clone(),
        proxy_cfg,
        tx_events,
        security: security.clone(),
        identity: identity.clone(),
        ledger_writer: Some(ledger_writer),
        loopback_only,
    };

    let agent_state = AgentInterfaceState {
        db,
        registry: registry.clone(),
        security: security.clone(),
        identity,
        upstream_url: upstream_url.clone(),
        override_store,
        delegation_engine,
    };

    services::spawn_model_introspector(registry, client, upstream_url, workers, security);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, create_router(state, agent_state)).await?;
    Ok(())
}
