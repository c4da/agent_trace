//! Security alert querying, acknowledgment, and reset operations.
//!
//! Keeps alert triage state in `alert_acks` so the `events` hash chain
//! and Ed25519 cryptographic signatures remain 100% immutable.

use rusqlite::{params, Connection, Result as SqliteResult};
use serde_json::Value;

/// Counts active (unacknowledged) security alerts across the ledger.
pub fn count_active_alerts(conn: &Connection) -> u64 {
    conn.query_row(
        "SELECT count(*) FROM events e
         LEFT JOIN alert_acks a ON e.event_id = a.event_id
         WHERE e.security_verdict != 'Allowed' AND a.event_id IS NULL",
        [],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

/// Lists security alert events (`security_verdict != 'Allowed'`).
/// When `include_acked` is false, returns only active (unacknowledged) alerts.
pub fn list_security_alerts(conn: &Connection, include_acked: bool) -> SqliteResult<Vec<Value>> {
    let filter_clause = if include_acked {
        "WHERE e.security_verdict != 'Allowed'"
    } else {
        "WHERE e.security_verdict != 'Allowed' AND a.event_id IS NULL"
    };

    let sql = format!(
        "SELECT e.event_id, e.node_id, e.session_id, e.timestamp, e.event_type,
                e.target, e.action_details, e.diff, e.provenance_sources,
                e.security_verdict, e.security_reasons, e.duration_ms,
                e.prev_hash, e.event_hash, e.signature, e.public_key,
                a.acked_at
         FROM events e
         LEFT JOIN alert_acks a ON e.event_id = a.event_id
         {}
         ORDER BY e.rowid DESC LIMIT 250",
        filter_clause
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_alert_row)?;

    let mut results = Vec::new();
    for r in rows.flatten() {
        results.push(r);
    }
    Ok(results)
}

/// Maps a single SQLite alert row into a JSON Value.
fn map_alert_row(row: &rusqlite::Row<'_>) -> SqliteResult<Value> {
    let details: String = row.get(6)?;
    let sources: String = row.get(8)?;
    let reasons: String = row.get(10)?;
    let acked_at: Option<String> = row.get(16)?;

    Ok(serde_json::json!({
        "event_id": row.get::<_, String>(0)?,
        "node_id": row.get::<_, String>(1)?,
        "session_id": row.get::<_, String>(2)?,
        "timestamp": row.get::<_, String>(3)?,
        "event_type": row.get::<_, String>(4)?,
        "target": row.get::<_, String>(5)?,
        "action_details": serde_json::from_str::<Value>(&details).unwrap_or_default(),
        "diff": row.get::<_, Option<String>>(7)?,
        "provenance_sources": serde_json::from_str::<Value>(&sources).unwrap_or_default(),
        "security_verdict": row.get::<_, String>(9)?,
        "security_reasons": serde_json::from_str::<Value>(&reasons).unwrap_or_default(),
        "duration_ms": row.get::<_, f64>(11)?,
        "prev_hash": row.get::<_, String>(12)?,
        "event_hash": row.get::<_, String>(13)?,
        "signature": row.get::<_, Option<String>>(14)?,
        "public_key": row.get::<_, Option<String>>(15)?,
        "acknowledged": acked_at.is_some(),
        "acked_at": acked_at,
    }))
}

/// Marks a single alert event as acknowledged/dismissed.
pub fn acknowledge_alert(conn: &Connection, event_id: &str) -> SqliteResult<usize> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO alert_acks (event_id, acked_at) VALUES (?1, ?2)",
        params![event_id, now],
    )
}

/// Marks all currently active security alerts as acknowledged/reset.
pub fn acknowledge_all_alerts(conn: &Connection) -> SqliteResult<usize> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO alert_acks (event_id, acked_at)
         SELECT event_id, ?1 FROM events WHERE security_verdict != 'Allowed'",
        params![now],
    )
}

/// Clears acknowledgments to restore historical alerts to active view.
pub fn restore_all_alerts(conn: &Connection) -> SqliteResult<usize> {
    conn.execute("DELETE FROM alert_acks", [])
}
