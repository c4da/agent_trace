use agent_trace_gw::schema::{ClusterSummary, EventType, NodeHeartbeat, SecurityVerdict, TraceEvent};
use serde_json::json;

#[test]
fn test_trace_event_serialization_roundtrip() {
    let event = TraceEvent {
        event_id: "evt-test-101".to_string(),
        node_id: "PC-1".to_string(),
        session_id: "session-xyz".to_string(),
        timestamp: "2026-10-09T10:00:00Z".to_string(),
        event_type: EventType::FileWrite,
        target: "C:/cada/projects/test.rs".to_string(),
        action_details: json!({"lines_added": 15, "author": "antigravity"}),
        diff: Some("+pub fn test() {}".to_string()),
        provenance_sources: vec!["ide_edit".to_string(), "tool_call".to_string()],
        security_verdict: SecurityVerdict::Allowed,
        security_reasons: vec!["Within project directory".to_string()],
        duration_ms: 45.2,
        prev_hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        event_hash: "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90".to_string(),
        signature: Some("deadbeef".repeat(16)),
        public_key: Some("1c8751c56f22fba5316a8aadf0c231d3b776144ffc95f786552695a12f1b984e".to_string()),
    };

    let serialized = serde_json::to_string(&event).expect("Failed to serialize TraceEvent");
    let deserialized: TraceEvent = serde_json::from_str(&serialized).expect("Failed to deserialize TraceEvent");

    assert_eq!(deserialized.event_id, event.event_id);
    assert_eq!(deserialized.node_id, event.node_id);
    assert_eq!(deserialized.session_id, event.session_id);
    assert_eq!(deserialized.event_type, EventType::FileWrite);
    assert_eq!(deserialized.security_verdict, SecurityVerdict::Allowed);
    assert_eq!(deserialized.diff, Some("+pub fn test() {}".to_string()));
    assert_eq!(deserialized.duration_ms, 45.2);
}

#[test]
fn test_security_verdict_variants_and_equality() {
    let allowed = SecurityVerdict::Allowed;
    let flagged = SecurityVerdict::Flagged;
    let blocked = SecurityVerdict::Blocked;

    assert_ne!(allowed, flagged);
    assert_ne!(flagged, blocked);
    assert_ne!(allowed, blocked);

    let allowed_json = serde_json::to_string(&allowed).unwrap();
    assert_eq!(allowed_json, "\"Allowed\"");

    let blocked_json = serde_json::to_string(&blocked).unwrap();
    assert_eq!(blocked_json, "\"Blocked\"");

    let parsed: SecurityVerdict = serde_json::from_str(&blocked_json).unwrap();
    assert_eq!(parsed, SecurityVerdict::Blocked);
}

#[test]
fn test_event_type_variants_and_serde() {
    let types = vec![
        (EventType::FileRead, "\"FileRead\""),
        (EventType::FileWrite, "\"FileWrite\""),
        (EventType::CommandRun, "\"CommandRun\""),
        (EventType::WebRequest, "\"WebRequest\""),
        (EventType::DecisionStep, "\"DecisionStep\""),
        (EventType::LlmInference, "\"LlmInference\""),
    ];

    for (ev, expected_json) in types {
        let serialized = serde_json::to_string(&ev).unwrap();
        assert_eq!(serialized, expected_json);
        let deserialized: EventType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, ev);
    }
}

#[test]
fn test_node_heartbeat_optional_fields() {
    let hb_minimal = json!({
        "node_id": "PC-2",
        "hostname": "NODE-02",
        "ip_address": "192.168.1.102",
        "role": "Worker Node",
        "status": "ACTIVE",
        "last_seen": "2026-10-09T10:00:00Z",
        "public_key": null,
        "models": null
    });

    let node: NodeHeartbeat = serde_json::from_value(hb_minimal).expect("Deserialization failed");
    assert_eq!(node.node_id, "PC-2");
    assert!(node.public_key.is_none());
    assert!(node.models.is_none());
}

#[test]
fn test_cluster_summary_deserialization() {
    let summary = ClusterSummary {
        total_nodes: 2,
        active_nodes: vec![],
        total_events: 7500,
        total_reads: 2000,
        total_writes: 800,
        total_commands: 2500,
        security_alerts: 4,
        blast_radius_score: 98.5,
        latest_ledger_hash: "abcdef123456".to_string(),
        ledger_verified: true,
    };

    let serialized = serde_json::to_string(&summary).unwrap();
    let parsed: ClusterSummary = serde_json::from_str(&serialized).unwrap();

    assert_eq!(parsed.total_nodes, 2);
    assert_eq!(parsed.total_events, 7500);
    assert!(parsed.ledger_verified);
    assert_eq!(parsed.blast_radius_score, 98.5);
}
