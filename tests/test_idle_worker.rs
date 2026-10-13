use agent_trace_gw::{
    agent_interface::AgentInterfaceState,
    db::Database,
    delegation_engine::{AgentDelegationEngine, RouteOverrideStore},
    handlers::{create_router, AppState},
    ledger::NodeIdentity,
    policy::PolicyEngine,
    proxy::ProxyConfig,
    registry::ClusterModelRegistry,
    secrets::ClusterSecurityConfig,
};
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{collections::HashSet, sync::Arc};
use tokio::sync::{broadcast, RwLock};
use tower::ServiceExt;

fn create_test_app() -> (axum::Router, Database, tempfile::TempDir) {
    let tmp_dir = tempfile::tempdir().expect("failed to create tempdir");
    let db_path = tmp_dir.path().join("test_idle_telemetry.db");
    let db = Database::new(db_path.to_str().expect("valid path")).expect("failed to init db");

    let key_path = tmp_dir.path().join("test_node_key.pk8");
    let identity = Arc::new(
        NodeIdentity::load_or_generate(key_path.to_str().expect("valid key path"))
            .expect("failed to generate identity"),
    );

    let mut allowlist = HashSet::new();
    allowlist.insert("http://127.0.0.1:11434".to_string());

    let security_cfg = Arc::new(ClusterSecurityConfig {
        auth_token: None,
        upstream_allowlist: allowlist,
        worker_hermes_key: None,
        key_path,
    });

    let (tx_events, _) = broadcast::channel(100);

    let client = reqwest::Client::builder().build().unwrap();
    let registry = Arc::new(RwLock::new(ClusterModelRegistry::new()));
    let override_store = Arc::new(RwLock::new(RouteOverrideStore::new()));
    let upstream_url = "http://127.0.0.1:11434".to_string();

    let delegation_engine = Arc::new(AgentDelegationEngine::new(
        override_store.clone(),
        registry.clone(),
        upstream_url.clone(),
    ));

    let (ledger_writer, _) = agent_trace_gw::ledger_writer::LedgerWriter::spawn(
        db.clone(),
        Some(identity.clone()),
        Some(tx_events.clone()),
        1024,
    );

    let proxy_cfg = Arc::new(ProxyConfig {
        upstream_url: upstream_url.clone(),
        db: db.clone(),
        security: security_cfg.clone(),
        identity: identity.clone(),
        client,
        registry: registry.clone(),
        delegation_engine: delegation_engine.clone(),
        ledger_writer: Some(ledger_writer.clone()),
    });

    let policy = Arc::new(RwLock::new(PolicyEngine::new_with_defaults()));

    let state = AppState {
        db: db.clone(),
        proxy_cfg: proxy_cfg.clone(),
        tx_events,
        security: security_cfg.clone(),
        identity: identity.clone(),
        lead_url: None,
        node_id: "TEST-LEAD".to_string(),
        registry: registry.clone(),
        policy: policy.clone(),
        ledger_writer: Some(ledger_writer),
    };

    let agent_state = AgentInterfaceState {
        db: db.clone(),
        registry,
        security: security_cfg,
        identity,
        upstream_url,
        override_store,
        delegation_engine,
        policy,
    };

    let router = create_router(state, agent_state);
    (router, db, tmp_dir)
}

#[tokio::test]
async fn test_idle_worker_status_and_config_update() {
    let (app, _db, _dir) = create_test_app();

    // 1. GET /api/v1/idle_worker/status
    let req = Request::builder()
        .uri("/api/v1/idle_worker/status")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();

    assert!(body["enabled"].as_bool().unwrap());
    assert!(body["config"]["tasks"].as_array().unwrap().len() >= 3);

    // 2. POST /api/v1/idle_worker/config
    let new_cfg = json!({
        "enabled": true,
        "idle_threshold_secs": 15,
        "tasks": ["CodeQualityAudit", "InvariantAudit", "LedgerAudit"],
        "custom_objective": "Test objective",
        "target_model": "deepseek-r1:1.5b",
        "auto_stage_proposals": true
    });

    let req = Request::builder()
        .uri("/api/v1/idle_worker/config")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&new_cfg).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "config_updated");
    assert_eq!(body["config"]["idle_threshold_secs"], 15);
    assert_eq!(body["config"]["target_model"], "deepseek-r1:1.5b");
}

#[tokio::test]
async fn test_idle_worker_trigger_and_user_preemption() {
    let (app, _db, _dir) = create_test_app();

    // 1. Trigger manual idle task
    let req = Request::builder()
        .uri("/api/v1/idle_worker/trigger")
        .method("POST")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "triggered_task");
    assert!(!body["task_result"]["findings"].as_array().unwrap().is_empty());

    // 2. Trigger user activity (preempts engine)
    let req = Request::builder()
        .uri("/api/v1/idle_worker/user_activity")
        .method("POST")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "user_activity_recorded");
    assert!(body["preempted"].as_bool().unwrap());
}
