use agent_trace_gw::{
    agent_interface::AgentInterfaceState,
    db::Database,
    delegation_engine::{AgentDelegationEngine, RouteOverrideStore},
    handlers::{create_router, AppState},
    ledger::NodeIdentity,
    policy::PolicyEngine,
    proxy::ProxyConfig,
    registry::ClusterModelRegistry,
    schema::{EventType, NodeHeartbeat, SecurityVerdict, TraceEvent},
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
fn create_test_app(auth_token: Option<String>) -> (axum::Router, Database, tempfile::TempDir) {
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
async fn test_http_dashboard_and_static_routes() {
    let (app, _db, _tmp) = create_test_app(None);

    // 1. GET / (Dashboard)
    let res = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("AgentTrace Gateway"));

    // 2. GET /dashboard
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/dashboard")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2b. GET /console (Terminal Console UI)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/console")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("AgentTrace Terminal Console"));
    assert!(body_str.contains("Unified Diff"));

    // 2c. GET /terminal alias
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/terminal")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2d. GET /graph (Multi-Agent Problem Contribution Graph UI)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/graph")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("PROBLEM CONTRIBUTION GRAPH"));

    // 2e. GET /api/v1/telemetry/contributions (Contribution Graph JSON API)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/telemetry/contributions?limit=100")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. GET /flyer
    let res = app
        .clone()
        .oneshot(Request::builder().uri("/flyer").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. GET /install with custom Host header
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/install")
                .header(header::HOST, "10.0.0.42:8080")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let script = String::from_utf8_lossy(&body);
    assert!(script.contains("10.0.0.42"));

    // 5. GET /install.sh & /provision.sh
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/install.sh")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/provision.sh")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_http_event_ingestion_and_ledger_verification_pipeline() {
    let (app, _db, _tmp) = create_test_app(None);

    let event_payload = json!({
        "event_id": "http_ev_101",
        "node_id": "PC-1",
        "session_id": "sess_unit_test",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "event_type": "FileWrite",
        "target": "src/config.json",
        "action_details": { "bytes": 128 },
        "diff": Some("+ {\"mode\": \"production\"}"),
        "provenance_sources": ["ide_user_prompt"],
        "security_verdict": "Allowed",
        "security_reasons": ["Target is inside project workspace"],
        "duration_ms": 12.5,
        "prev_hash": "",
        "event_hash": "",
        "signature": None::<String>,
        "public_key": None::<String>
    });

    // 1. Ingest event via POST /api/v1/events
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/events")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&event_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED);

    // 2. Query events via GET /api/v1/events
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/events?limit=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let events_list: Vec<Value> = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(events_list.len(), 1);
    assert_eq!(events_list[0]["event_id"], "http_ev_101");
    let event_hash = events_list[0]["event_hash"].as_str().unwrap();
    assert_eq!(event_hash.len(), 64);

    // 3. Query cluster summary via GET /api/v1/summary
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let summary: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(summary["total_events"], 1);
    assert_eq!(summary["total_writes"], 1);
    assert_eq!(summary["ledger_verified"], true);

    // 4. Verify cryptographic ledger via GET /api/v1/ledger/verify
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/ledger/verify")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let ledger_report: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(ledger_report["is_valid"], true);
    assert_eq!(ledger_report["total_blocks"], 1);
    assert_eq!(ledger_report["latest_block_hash"], event_hash);
}

#[tokio::test]
async fn test_http_node_heartbeat_and_summary_metrics() {
    let (app, _db, _tmp) = create_test_app(None);

    let heartbeat = NodeHeartbeat {
        node_id: "NODE-WORKER-02".to_string(),
        hostname: "NODE-02".to_string(),
        ip_address: "192.168.1.102".to_string(),
        role: "Worker Node (CUDA)".to_string(),
        status: "ACTIVE".to_string(),
        last_seen: chrono::Utc::now().to_rfc3339(),
        public_key: Some("aabbccddeeff00112233445566778899".to_string()),
        models: None,
    };

    // 1. Register heartbeat via POST /api/v1/heartbeat
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/heartbeat")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&heartbeat).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    // 2. Validate node registration in cluster summary
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let summary: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(summary["total_nodes"], 1);
}

#[tokio::test]
async fn test_http_telemetry_analytics_endpoint() {
    let (app, db, _tmp) = create_test_app(None);

    // Seed events into DB
    let mut ev1 = TraceEvent {
        event_id: "an_1".to_string(),
        node_id: "PC-1".to_string(),
        session_id: "sess_an".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_type: EventType::FileRead,
        target: "src/main.rs".to_string(),
        action_details: json!({"agent": "Agent-Alpha"}),
        diff: None,
        provenance_sources: vec!["tool_read".to_string()],
        security_verdict: SecurityVerdict::Allowed,
        security_reasons: vec![],
        duration_ms: 10.0,
        prev_hash: String::new(),
        event_hash: String::new(),
        signature: None,
        public_key: None,
    };
    db.insert_event(&mut ev1).unwrap();

    let mut ev2 = TraceEvent {
        event_id: "an_2".to_string(),
        node_id: "PC-1".to_string(),
        session_id: "sess_an".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_type: EventType::FileWrite,
        target: "c:/windows/system32/drivers/etc/hosts".to_string(),
        action_details: json!({"agent": "Agent-Alpha"}),
        diff: None,
        provenance_sources: vec!["unauthorized_tool".to_string()],
        security_verdict: SecurityVerdict::Blocked,
        security_reasons: vec!["Attempted modification of protected system file".to_string()],
        duration_ms: 25.0,
        prev_hash: String::new(),
        event_hash: String::new(),
        signature: None,
        public_key: None,
    };
    db.insert_event(&mut ev2).unwrap();

    // GET /api/v1/telemetry/analytics
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/telemetry/analytics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let report: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(report["total_events"], 2);
    assert_eq!(report["security_anomalies"].as_array().unwrap().len(), 1);
    assert_eq!(
        report["security_anomalies"][0]["verdict"],
        "Blocked"
    );
    assert_eq!(
        report["agent_profiles"][0]["agent_id"],
        "PC-1"
    );
    assert_eq!(
        report["agent_profiles"][0]["threat_level"],
        "CRITICAL"
    );
}

#[tokio::test]
async fn test_http_policy_management_and_evaluation() {
    let (app, _db, _tmp) = create_test_app(None);

    // 1. GET /api/v1/policy (list default policy)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/policy")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let policies: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(!policies.as_array().unwrap().is_empty());

    // 2. POST /api/v1/policy (set custom policy for agent)
    let policy_payload = json!({
        "agent_id": "Worker-Agent-Beta",
        "read_whitelist": ["c:/safe_workspace/*"],
        "write_whitelist": ["c:/safe_workspace/build/*"],
        "network_whitelist": ["localhost", "127.0.0.1"],
        "blocked_patterns": [".key", ".env"],
        "enforcing": true
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&policy_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. POST /api/v1/policy/evaluate (evaluate allowed path)
    let eval_allowed = json!({
        "agent_id": "Worker-Agent-Beta",
        "action": "Write",
        "target": "c:/safe_workspace/build/output.log"
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/evaluate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&eval_allowed).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let eval_res: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(eval_res["verdict"], "Allowed");

    // 4. POST /api/v1/policy/evaluate (evaluate blocked pattern)
    let eval_blocked = json!({
        "agent_id": "Worker-Agent-Beta",
        "action": "Write",
        "target": "c:/safe_workspace/build/secrets.key"
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/policy/evaluate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&eval_blocked).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let eval_res: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(eval_res["verdict"], "Blocked");

    // 5. DELETE /api/v1/policy/:agent_id
    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/policy/Worker-Agent-Beta")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_http_agent_tools_and_delegation_overrides() {
    let (app, _db, _tmp) = create_test_app(None);

    // 1. GET /api/v1/agent/tools
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/agent/tools")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let tools_doc: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(tools_doc["protocol"], "AgentTrace-Agent-Interface");
    assert!(tools_doc["tools"].as_array().unwrap().len() >= 7);

    // 2. POST /api/v1/delegate/override (create rule)
    let override_payload = json!({
        "rule_id": "test_override_1",
        "criteria": { "type": "tag", "value": "gpu_workload" },
        "target_model": "deepseek-r1:1.5b",
        "target_node": "PC-2",
        "reason": "Route all GPU tags to worker node",
        "created_at": chrono::Utc::now().to_rfc3339()
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/delegate/override")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&override_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. GET /api/v1/delegate/overrides
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/delegate/overrides")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let overrides_list: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(overrides_list["count"], 1);

    // 4. POST /api/v1/delegate/decide (evaluates override rule)
    let decide_req = json!({
        "prompt": "Run mathematical optimization on CUDA tensor",
        "session_id": "sess_rule_test",
        "agent_id": null,
        "tags": ["gpu_workload"],
        "requested_model": "auto"
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/delegate/decide")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&decide_req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let decision: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(decision["selected_model"], "deepseek-r1:1.5b");
    assert_eq!(decision["node_id"], "PC-2");
    assert_eq!(decision["decision_source"], "OverrideRule");

    // 5. DELETE /api/v1/delegate/override/:rule_id
    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/delegate/override/test_override_1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_http_sse_stream_header() {
    let (app, _db, _tmp) = create_test_app(None);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/stream")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(content_type.starts_with("text/event-stream"));
}

#[tokio::test]
async fn test_http_auth_token_enforcement_and_bypass() {
    let (app, _db, _tmp) = create_test_app(Some("super_secret_token_456".to_string()));

    let event_payload = json!({
        "event_id": "auth_ev_1",
        "node_id": "PC-1",
        "session_id": "sess_auth",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "event_type": "CommandRun",
        "target": "src/build.rs",
        "action_details": {},
        "diff": None::<String>,
        "provenance_sources": [],
        "security_verdict": "Allowed",
        "security_reasons": [],
        "duration_ms": 1.0,
        "prev_hash": "",
        "event_hash": "",
        "signature": None::<String>,
        "public_key": None::<String>
    });

    // 1. Request without auth -> 401 Unauthorized
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/events")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&event_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Request with wrong token -> 401 Unauthorized
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/events")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-agenttrace-token", "wrong_token")
                .body(Body::from(serde_json::to_vec(&event_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 3. Request with custom header token -> 201 Created
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/events")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-agenttrace-token", "super_secret_token_456")
                .body(Body::from(serde_json::to_vec(&event_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 4. Request with Bearer authorization header -> 201 Created
    let mut event_payload_2 = event_payload.clone();
    event_payload_2["event_id"] = json!("auth_ev_2");

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/events")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, "Bearer super_secret_token_456")
                .body(Body::from(serde_json::to_vec(&event_payload_2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_http_workgroups_and_markdown_rules_lifecycle() {
    let (app, _db, _tmp) = create_test_app(None);

    // 1. GET /api/v1/workgroups -> verify default groups (wg-coding, wg-image-gen) and available agents
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/workgroups")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let list_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(list_json["workgroups"].as_array().unwrap().len() >= 4);
    assert!(list_json["available_agents"].as_array().unwrap().len() >= 6);

    // 2. POST /api/v1/workgroups -> create a custom Problem Workgroup
    let create_payload = json!({
        "id": "wg-test-image-copilot",
        "name": "Custom Image Generation Squad",
        "problem_domain": "image_generation",
        "icon": "🎨",
        "color": "#bc8cff",
        "description": "DeepSeek + Copilot CLI image synthesis group",
        "execution_strategy": "pipeline",
        "members": [
            {
                "agent_id": "deepseek-r1:1.5b",
                "display_name": "DeepSeek-R1 (1.5B CUDA)",
                "node_id": "PC-2",
                "role": "LeadArchitect"
            },
            {
                "agent_id": "copilot-cli",
                "display_name": "GitHub Copilot CLI",
                "node_id": "PC-1-Copilot",
                "role": "PrimaryBuilder"
            }
        ],
        "trigger_keywords": ["banner", "poster"]
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workgroups")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&create_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. POST /api/v1/workgroups/wg-test-image-copilot/rules -> attach a Markdown (.md) rule
    let rule_payload = json!({
        "title": "Strict SVG & Palette Rules",
        "file_path": "README.md",
        "inline_markdown": "- Always use dark #0d1117 background.\n- No laptop/phone frames.",
        "enforce_strict": true
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workgroups/wg-test-image-copilot/rules")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&rule_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. GET /api/v1/workgroups/wg-test-image-copilot/preview -> verify compiled prompt contains attached .md rule
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/workgroups/wg-test-image-copilot/preview")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let preview_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    let compiled = preview_json["compiled_system_prompt"].as_str().unwrap();
    assert!(compiled.contains("Strict SVG & Palette Rules"));
    assert!(compiled.contains("Always use dark #0d1117 background"));
    assert!(compiled.contains("GitHub Copilot CLI"));

    // 5. DELETE /api/v1/workgroups/wg-test-image-copilot -> cleanup
    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/workgroups/wg-test-image-copilot")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

