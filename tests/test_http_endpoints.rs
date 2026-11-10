use agent_trace_gw::{
    agent_interface::AgentInterfaceState,
    db::Database,
    delegation_engine::{AgentDelegationEngine, RouteOverrideStore},
    handlers::{create_router, AppState},
    ledger::NodeIdentity,
    dlp::DlpMode,
    proxy::ProxyConfig,
    registry::ClusterModelRegistry,
    schema::{EventType, SecurityVerdict, TraceEvent},
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

/// Creates a fully configured in-memory/temp test instance of the AgentTrace Gateway Axum router.
fn create_test_app(auth_token: Option<String>, loopback_only: bool) -> (axum::Router, Database, tempfile::TempDir) {
    let tmp_dir = tempfile::tempdir().expect("failed to create tempdir");
    let db_path = tmp_dir.path().join("test_http_telemetry.db");
    let db = Database::new(db_path.to_str().expect("valid path")).expect("failed to init db");

    let key_path = tmp_dir.path().join("test_node_key.pk8");
    let identity = Arc::new(
        NodeIdentity::load_or_generate(key_path.to_str().expect("valid key path"))
            .expect("failed to generate identity"),
    );

    let mut allowlist = HashSet::new();
    allowlist.insert("http://127.0.0.1:11434".to_string());
    allowlist.insert("http://localhost:11434".to_string());
    allowlist.insert("http://192.168.1.102:8642".to_string());

    let security_cfg = Arc::new(ClusterSecurityConfig {
        require_auth: false,
        auth_token,
        upstream_allowlist: allowlist,
        worker_hermes_key: None,
        key_path,
    });

    let (tx_events, _) = broadcast::channel(100);
    let client = reqwest::Client::new();
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
        dlp_mode: DlpMode::Redact,
    });

    let state = AppState {
        db: db.clone(),
        proxy_cfg: proxy_cfg.clone(),
        tx_events,
        security: security_cfg.clone(),
        identity: identity.clone(),
        ledger_writer: Some(ledger_writer),
        loopback_only,
    };

    let agent_state = AgentInterfaceState {
        db: db.clone(),
        registry,
        security: security_cfg,
        identity,
        upstream_url,
        override_store,
        delegation_engine,
    };

    let router = create_router(state, agent_state);
    (router, db, tmp_dir)
}

fn req(uri: &str, host: &str) -> Request<Body> {
    Request::builder().uri(uri).header(header::HOST, host).body(Body::empty()).unwrap()
}

#[tokio::test]
async fn static_pages_are_public_even_with_token() {
    let (app, _db, _tmp) = create_test_app(Some("s3cret".into()), true);
    for uri in ["/", "/dashboard", "/console"] {
        let res = app.clone().oneshot(req(uri, "localhost:8080")).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{uri}");
    }
}

#[tokio::test]
async fn token_required_for_api_routes() {
    let (app, _db, _tmp) = create_test_app(Some("s3cret".into()), true);
    let res = app.clone().oneshot(req("/api/v1/events", "localhost")).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let ok = Request::builder()
        .uri("/api/v1/events")
        .header(header::HOST, "localhost")
        .header(header::AUTHORIZATION, "Bearer s3cret")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(ok).await.unwrap().status(), StatusCode::OK);

    let bad = Request::builder()
        .uri("/api/v1/events")
        .header(header::HOST, "localhost")
        .header(header::AUTHORIZATION, "Bearer wrong")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(bad).await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn foreign_host_header_rejected_when_loopback_bound() {
    let (app, _db, _tmp) = create_test_app(None, true);
    let res = app.clone().oneshot(req("/api/v1/summary", "evil.example")).await.unwrap();
    assert_eq!(res.status(), StatusCode::MISDIRECTED_REQUEST);

    for host in ["localhost:8080", "127.0.0.1:8080", "[::1]:8080"] {
        let res = app.clone().oneshot(req("/api/v1/summary", host)).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{host}");
    }

    // Not loopback-bound: Host check disabled.
    let (app, _db, _tmp) = create_test_app(None, false);
    let res = app.oneshot(req("/api/v1/summary", "evil.example")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn no_cors_headers_for_cross_origin_requests() {
    let (app, _db, _tmp) = create_test_app(None, true);
    let r = Request::builder()
        .uri("/api/v1/summary")
        .header(header::HOST, "localhost")
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(r).await.unwrap();
    assert!(res.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
}

#[tokio::test]
async fn dlp_scan_redacts_aws_key() {
    let (app, _db, _tmp) = create_test_app(None, true);
    let r = Request::builder()
        .method("POST")
        .uri("/api/v1/dlp/scan")
        .header(header::HOST, "localhost")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"text": "key=AKIAIOSFODNN7EXAMPLE ok"}).to_string()))
        .unwrap();
    let res = app.oneshot(r).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&body).unwrap();
    let redacted = v["redacted"].as_str().unwrap();
    assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"), "{redacted}");
    assert!(!v["findings"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn ingest_event_redacts_and_ledger_verifies() {
    let (app, db, _tmp) = create_test_app(None, true);
    let ev = TraceEvent {
        event_id: "ev_http_1".into(),
        node_id: "PC-1".into(),
        session_id: "s1".into(),
        timestamp: "2026-10-07T12:00:00Z".into(),
        event_type: EventType::CommandRun,
        target: "echo AKIAIOSFODNN7EXAMPLE".into(),
        action_details: json!({}),
        diff: None,
        provenance_sources: vec![],
        security_verdict: SecurityVerdict::Allowed,
        security_reasons: vec![],
        duration_ms: 1.0,
        prev_hash: String::new(),
        event_hash: String::new(),
        signature: None,
        public_key: None,
    };
    let r = Request::builder()
        .method("POST")
        .uri("/api/v1/events")
        .header(header::HOST, "localhost")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&ev).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(r).await.unwrap();
    assert!(res.status().is_success(), "{}", res.status());

    // The ledger writer is asynchronous; poll briefly.
    let mut found = false;
    for _ in 0..60 {
        let evs = db.get_recent_events_filtered(10, None, None).unwrap();
        if let Some(e) = evs.first() {
            let s = e.to_string();
            assert!(!s.contains("AKIAIOSFODNN7EXAMPLE"), "secret stored raw: {s}");
            found = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(found, "event never persisted");

    let res = app.oneshot(req("/api/v1/ledger/verify", "localhost")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn agent_tools_endpoint_lists_all_tools() {
    let (app, _db, _tmp) = create_test_app(None, true);
    let res = app.oneshot(req("/api/v1/agent/tools", "localhost")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["tools"].as_array().unwrap().len(), 17);
}
