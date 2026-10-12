//! SQLite Telemetry and Cluster Database for AgentTrace Gateway.
//!
//! Provides thread-safe, WAL-enabled persistence for the cryptographic ledger,
//! node registrations, policy telemetry, and analytics queries.

pub mod analytics;
pub mod events;
pub mod nodes;
pub mod schema_init;
pub mod verify;

pub use events::EventFilterCriteria;

use crate::ledger::{LedgerVerificationReport, NodeIdentity};
use crate::schema::{ClusterSummary, NodeHeartbeat, TraceEvent};
use rusqlite::{Connection, Result as SqliteResult};
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Thread-safe handle to the SQLite telemetry database.
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Opens or creates the SQLite database at `db_path` and initializes tables.
    pub fn new(db_path: &str) -> SqliteResult<Self> {
        let conn = Connection::open(db_path)?;
        schema_init::init_tables(&conn)?;

        Ok(Database {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Fetches the latest event hash or genesis hash if empty.
    pub fn get_latest_event_hash(&self) -> SqliteResult<String> {
        let conn = self.conn.lock().unwrap();
        events::get_latest_event_hash(&conn)
    }

    /// Inserts an event without signing.
    pub fn insert_event(&self, event: &mut TraceEvent) -> SqliteResult<()> {
        self.insert_event_signed(event, None)
    }

    /// Inserts an event, chains hash, and optionally attaches an Ed25519 signature.
    pub fn insert_event_signed(
        &self,
        event: &mut TraceEvent,
        identity: Option<&NodeIdentity>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        events::insert_event_signed(&conn, event, identity)
    }

    /// Inserts or updates node registration.
    pub fn upsert_node(&self, node: &NodeHeartbeat) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        nodes::upsert_node(&conn, node)
    }

    /// Deletes a node by its ID.
    pub fn delete_node(&self, node_id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        nodes::delete_node(&conn, node_id)
    }

    /// Returns recently recorded events.
    pub fn get_recent_events(&self, limit: usize) -> SqliteResult<Vec<Value>> {
        let conn = self.conn.lock().unwrap();
        events::get_recent_events(&conn, limit)
    }

    /// Returns recently recorded events matching filter criteria.
    pub fn get_recent_events_filtered(
        &self,
        limit: usize,
        node_id: Option<&str>,
        event_type: Option<&str>,
    ) -> SqliteResult<Vec<Value>> {
        let conn = self.conn.lock().unwrap();
        events::get_recent_events_filtered(
            &conn,
            EventFilterCriteria {
                limit,
                node_id,
                event_type,
            },
        )
    }

    /// Verifies cryptographic hash continuity, payloads, and signatures.
    pub fn verify_ledger(&self) -> SqliteResult<LedgerVerificationReport> {
        let conn = self.conn.lock().unwrap();
        verify::verify_ledger(&conn)
    }

    /// Computes cluster health and event volume summary.
    pub fn get_cluster_summary(&self) -> SqliteResult<ClusterSummary> {
        let conn = self.conn.lock().unwrap();
        analytics::get_cluster_summary(&conn)
    }

    /// Computes multi-dimensional telemetry analytics.
    pub fn get_telemetry_analytics(&self) -> SqliteResult<crate::analytics::TelemetryAnalyticsReport> {
        let conn = self.conn.lock().unwrap();
        analytics::get_telemetry_analytics(&conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{EventType, SecurityVerdict};
    use tempfile::NamedTempFile;

    #[test]
    fn test_node_upsert_and_deletion() {
        let tmp = NamedTempFile::new().unwrap();
        let db = Database::new(tmp.path().to_str().unwrap()).unwrap();

        let node1 = NodeHeartbeat {
            node_id: "PC-1".to_string(),
            hostname: "VECTOR".to_string(),
            ip_address: "192.168.1.101".to_string(),
            role: "Lead Node".to_string(),
            status: "ACTIVE".to_string(),
            last_seen: chrono::Utc::now().to_rfc3339(),
            public_key: Some("abcdef123456".to_string()),
            models: None,
        };
        let node2 = NodeHeartbeat {
            node_id: "PC-2".to_string(),
            hostname: "NODE-02".to_string(),
            ip_address: "192.168.1.102".to_string(),
            role: "Worker Node".to_string(),
            status: "ACTIVE".to_string(),
            last_seen: chrono::Utc::now().to_rfc3339(),
            public_key: None,
            models: None,
        };

        db.upsert_node(&node1).unwrap();
        db.upsert_node(&node2).unwrap();

        let summary = db.get_cluster_summary().unwrap();
        assert_eq!(summary.total_nodes, 2);

        db.delete_node("PC-2").unwrap();
        let summary2 = db.get_cluster_summary().unwrap();
        assert_eq!(summary2.total_nodes, 1);
        assert_eq!(summary2.active_nodes[0].node_id, "PC-1");
    }

    #[test]
    fn test_insert_event_signed_and_verification() {
        let tmp = NamedTempFile::new().unwrap();
        let db = Database::new(tmp.path().to_str().unwrap()).unwrap();

        let key_file = NamedTempFile::new().unwrap();
        let identity = NodeIdentity::load_or_generate(key_file.path()).unwrap();

        let mut ev1 = TraceEvent {
            event_id: "ev_1".to_string(),
            node_id: "PC-1".to_string(),
            session_id: "sess_1".to_string(),
            timestamp: "2026-10-07T10:00:00Z".to_string(),
            event_type: EventType::FileRead,
            target: "src/main.rs".to_string(),
            action_details: serde_json::json!({}),
            diff: None,
            provenance_sources: vec!["agent".to_string()],
            security_verdict: SecurityVerdict::Allowed,
            security_reasons: vec![],
            duration_ms: 5.0,
            prev_hash: String::new(),
            event_hash: String::new(),
            signature: None,
            public_key: None,
        };
        db.insert_event_signed(&mut ev1, Some(&identity)).unwrap();

        let mut ev2 = TraceEvent {
            event_id: "ev_2".to_string(),
            node_id: "PC-1".to_string(),
            session_id: "sess_1".to_string(),
            timestamp: "2026-10-07T10:01:00Z".to_string(),
            event_type: EventType::FileWrite,
            target: "src/main.rs".to_string(),
            action_details: serde_json::json!({}),
            diff: Some("+ refactor".to_string()),
            provenance_sources: vec!["agent".to_string()],
            security_verdict: SecurityVerdict::Allowed,
            security_reasons: vec![],
            duration_ms: 12.0,
            prev_hash: String::new(),
            event_hash: String::new(),
            signature: None,
            public_key: None,
        };
        db.insert_event_signed(&mut ev2, Some(&identity)).unwrap();

        let report = db.verify_ledger().unwrap();
        assert!(report.is_valid);
        assert_eq!(report.total_blocks, 2);
        assert_eq!(ev2.prev_hash, ev1.event_hash);
    }

    #[test]
    fn test_get_recent_events_limit() {
        let tmp = NamedTempFile::new().unwrap();
        let db = Database::new(tmp.path().to_str().unwrap()).unwrap();

        for i in 1..=5 {
            let mut ev = TraceEvent {
                event_id: format!("ev_{}", i),
                node_id: "PC-1".to_string(),
                session_id: "sess_1".to_string(),
                timestamp: format!("2026-10-07T10:0{}:00Z", i),
                event_type: EventType::FileRead,
                target: format!("file_{}.txt", i),
                action_details: serde_json::json!({}),
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
            db.insert_event_signed(&mut ev, None).unwrap();
        }

        let recent = db.get_recent_events(3).unwrap();
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0]["event_id"], "ev_5");
        assert_eq!(recent[1]["event_id"], "ev_4");
        assert_eq!(recent[2]["event_id"], "ev_3");
    }

    #[test]
    fn test_get_telemetry_analytics_aggregation() {
        let tmp = NamedTempFile::new().unwrap();
        let db = Database::new(tmp.path().to_str().unwrap()).unwrap();

        let create_event = |id: &str, etype: EventType, target: &str, verdict: SecurityVerdict, reason: &str| TraceEvent {
            event_id: id.to_string(),
            node_id: "Agent-Alpha".to_string(),
            session_id: "sess_test".to_string(),
            timestamp: "2026-10-07T12:00:00Z".to_string(),
            event_type: etype,
            target: target.to_string(),
            action_details: serde_json::json!({}),
            diff: None,
            provenance_sources: vec![],
            security_verdict: verdict,
            security_reasons: if reason.is_empty() { vec![] } else { vec![reason.to_string()] },
            duration_ms: 2.0,
            prev_hash: String::new(),
            event_hash: String::new(),
            signature: None,
            public_key: None,
        };

        for i in 1..=3 {
            let mut ev = create_event(&format!("rA_{}", i), EventType::FileRead, "fileA.rs", SecurityVerdict::Allowed, "");
            db.insert_event_signed(&mut ev, None).unwrap();
        }
        let mut ev = create_event("rB_1", EventType::FileRead, "fileB.rs", SecurityVerdict::Allowed, "");
        db.insert_event_signed(&mut ev, None).unwrap();

        for i in 1..=2 {
            let mut ev = create_event(&format!("wA_{}", i), EventType::FileWrite, "fileA.rs", SecurityVerdict::Allowed, "");
            db.insert_event_signed(&mut ev, None).unwrap();
        }

        let mut ev_bad = create_event("bad_1", EventType::FileRead, ".env", SecurityVerdict::Blocked, "Blacklisted sensitive file");
        db.insert_event_signed(&mut ev_bad, None).unwrap();

        let report = db.get_telemetry_analytics().unwrap();
        assert_eq!(report.total_events, 7);
        assert!(report.ledger_valid);
        assert_eq!(report.top_read_targets[0].target, "fileA.rs");
        assert_eq!(report.top_read_targets[0].count, 3);
        assert_eq!(report.top_write_targets[0].target, "fileA.rs");
        assert_eq!(report.top_write_targets[0].count, 2);

        let profile = report.agent_profiles.iter().find(|p| p.agent_id == "Agent-Alpha").unwrap();
        assert_eq!(profile.reads, 5);
        assert_eq!(profile.writes, 2);
        assert_eq!(profile.violations, 1);
        assert_eq!(profile.threat_level, "CRITICAL");

        assert_eq!(report.security_anomalies.len(), 1);
        assert_eq!(report.security_anomalies[0].verdict, SecurityVerdict::Blocked);
        assert_eq!(report.security_anomalies[0].reason, "Blacklisted sensitive file");
    }
}
