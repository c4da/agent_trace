use agent_trace_gw::{
    agent_interface::AgentInterfaceState,
    antigravity_watcher,
    copilot_watcher,
    db::Database,
    delegation_engine::{AgentDelegationEngine, RouteOverrideStore},
    handlers::{create_router, AppState},
    ledger::NodeIdentity,
    policy::PolicyEngine,
    proxy::ProxyConfig,
    registry::ClusterModelRegistry,
    schema::NodeHeartbeat,
    secrets::ClusterSecurityConfig,
    services,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::{broadcast, RwLock};

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
            models: None,
        });
        let _ = db.upsert_node(&NodeHeartbeat {
            node_id: "PC-2".to_string(),
            hostname: "NODE-02".to_string(),
            ip_address: "192.168.1.102".to_string(),
            role: "Worker Node (hermes-agent :8642 + CUDA)".to_string(),
            status: "ACTIVE".to_string(),
            last_seen: chrono::Utc::now().to_rfc3339(),
            public_key: None,
            models: None,
        });
        let _ = db.upsert_node(&NodeHeartbeat {
            node_id: "PC-1-Copilot".to_string(),
            hostname: hostname.clone(),
            ip_address: "127.0.0.1".to_string(),
            role: "GitHub Copilot CLI (v1.0.95 · gpt-6-luna)".to_string(),
            status: "ACTIVE".to_string(),
            last_seen: chrono::Utc::now().to_rfc3339(),
            public_key: Some(identity.public_key_hex().to_string()),
            models: None,
        });
        let _ = db.delete_node("PC-3");
    }

    let (tx_events, _) = broadcast::channel(100);

    if !is_worker {
        // Spawn native autonomous Antigravity transcript watcher on Lead
        antigravity_watcher::spawn_watcher(db.clone(), tx_events.clone());
        // Spawn native autonomous GitHub Copilot CLI session watcher on Lead
        copilot_watcher::spawn_watcher(db.clone(), tx_events.clone());
    }

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let registry = Arc::new(RwLock::new(ClusterModelRegistry::new()));
    let override_store = Arc::new(RwLock::new(RouteOverrideStore::new()));
    let delegation_engine = Arc::new(AgentDelegationEngine::new(
        override_store.clone(),
        registry.clone(),
        upstream_url.clone(),
    ));

    let proxy_cfg = Arc::new(ProxyConfig {
        upstream_url: upstream_url.clone(),
        db: db.clone(),
        security: security_cfg.clone(),
        identity: identity.clone(),
        client: client.clone(),
        registry: registry.clone(),
        delegation_engine: delegation_engine.clone(),
    });

    let policy = Arc::new(RwLock::new(PolicyEngine::new_with_defaults()));

    let state = AppState {
        db: db.clone(),
        proxy_cfg: proxy_cfg.clone(),
        tx_events,
        security: security_cfg.clone(),
        identity: identity.clone(),
        lead_url: lead_url.clone(),
        node_id: node_id.clone(),
        registry: registry.clone(),
        policy: policy.clone(),
    };

    let agent_state = AgentInterfaceState {
        db: db.clone(),
        registry: registry.clone(),
        security: security_cfg.clone(),
        identity: identity.clone(),
        upstream_url: upstream_url.clone(),
        override_store: override_store.clone(),
        delegation_engine: delegation_engine.clone(),
        policy: policy.clone(),
    };

    // Spawn managed background services
    if !is_worker {
        services::spawn_model_introspector(registry.clone(), client.clone(), upstream_url, security_cfg.clone());
        services::spawn_discovery_beacon(port, "192.168.1.101");
        services::spawn_host_agent_discoverer(db.clone(), registry.clone(), hostname.clone(), identity.clone());
    } else if let Some(target_lead) = lead_url {
        services::spawn_worker_forwarder(target_lead, node_id, hostname, identity, client, security_cfg);
    }

    // Build modular HTTP app
    let app = create_router(state, agent_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
