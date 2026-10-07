use agent_trace_gw::{
    db::Database,
    ledger::{compute_event_hash_v1, compute_event_hash_v2, NodeIdentity, GENESIS_PREV_HASH},
    schema::{EventType, SecurityVerdict, TraceEvent},
};
use tempfile::NamedTempFile;

#[test]
fn test_v2_tamper_detection_security_verdict() {
    let tmp = NamedTempFile::new().unwrap();
    let db_path = tmp.path().to_str().unwrap();
    let db = Database::new(db_path).unwrap();

    let key_file = NamedTempFile::new().unwrap();
    let identity = NodeIdentity::load_or_generate(key_file.path()).unwrap();

    // 1. Insert an event that was Flagged by security
    let mut ev1 = TraceEvent {
        event_id: "ev_1".to_string(),
        node_id: "PC-2".to_string(),
        session_id: "sess_1".to_string(),
        timestamp: "2026-10-07T12:00:00Z".to_string(),
        event_type: EventType::CommandRun,
        target: "rm -rf /".to_string(),
        action_details: serde_json::json!({"blocked": true}),
        diff: None,
        provenance_sources: vec!["rule_engine".to_string()],
        security_verdict: SecurityVerdict::Flagged,
        security_reasons: vec!["Dangerous destructive command".to_string()],
        duration_ms: 12.5,
        prev_hash: String::new(),
        event_hash: String::new(),
        signature: None,
        public_key: None,
    };

    db.insert_event_signed(&mut ev1, Some(&identity)).unwrap();

    // Verification must pass initially
    let report = db.verify_ledger().unwrap();
    assert!(report.is_valid);
    assert_eq!(report.total_blocks, 1);

    // 2. An attacker directly tampers with SQLite table, flipping verdict from Flagged to Allowed
    {
        let conn = rusqlite::Connection::open(db_path).unwrap();
        conn.execute("UPDATE events SET security_verdict = 'Allowed' WHERE event_id = 'ev_1'", []).unwrap();
    }

    // 3. Verification MUST catch this tampering!
    let tampered_report = db.verify_ledger().unwrap();
    assert!(!tampered_report.is_valid, "Tampering with security_verdict must be detected by V2 canonical hash!");
    assert_eq!(tampered_report.broken_block_id, Some("ev_1".to_string()));
}

#[test]
fn test_ed25519_signature_verification() {
    let key_file = NamedTempFile::new().unwrap();
    let identity = NodeIdentity::load_or_generate(key_file.path()).unwrap();

    let digest = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
    let sig = identity.sign_event(digest);

    assert!(NodeIdentity::verify_signature(digest, &sig, identity.public_key_hex()));
    // Mismatched digest must fail
    assert!(!NodeIdentity::verify_signature("different_digest", &sig, identity.public_key_hex()));
}

#[test]
fn test_v1_legacy_compatibility() {
    let ev = TraceEvent {
        event_id: "legacy_1".to_string(),
        node_id: "PC-1".to_string(),
        session_id: "s1".to_string(),
        timestamp: "2026-10-05T00:00:00Z".to_string(),
        event_type: EventType::FileRead,
        target: "main.rs".to_string(),
        action_details: serde_json::json!({}),
        diff: None,
        provenance_sources: vec![],
        security_verdict: SecurityVerdict::Allowed,
        security_reasons: vec![],
        duration_ms: 10.0,
        prev_hash: GENESIS_PREV_HASH.to_string(),
        event_hash: String::new(),
        signature: None,
        public_key: None,
    };

    let hash_v1 = compute_event_hash_v1(&ev);
    let hash_v2 = compute_event_hash_v2(&ev);
    assert_ne!(hash_v1, hash_v2, "V2 and V1 hashes should use distinct canonical formats");
}

#[test]
fn test_verify_live_db() {
    if std::path::Path::new("cluster_telemetry.db").exists() {
        let db = Database::new("cluster_telemetry.db").unwrap();
        let report = db.verify_ledger().unwrap();
        assert!(report.is_valid, "Live database ledger must verify cleanly: {:?}", report.error_reason);
    }
}


