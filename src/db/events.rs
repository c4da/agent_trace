//! Ingestion and query logic for trace events in SQLite.

use crate::ledger::{compute_event_hash_v2, NodeIdentity, GENESIS_PREV_HASH};
use crate::schema::TraceEvent;
use rusqlite::{params, Connection, Result as SqliteResult};
use serde_json::Value;

/// Filter criteria for querying events without passing many arguments.
#[derive(Clone, Copy, Debug, Default)]
pub struct EventFilterCriteria<'a> {
    pub limit: usize,
    pub node_id: Option<&'a str>,
    pub event_type: Option<&'a str>,
}

/// Reads the latest event hash from SQLite or returns genesis.
pub fn get_latest_event_hash(conn: &Connection) -> SqliteResult<String> {
    let mut stmt = conn.prepare("SELECT event_hash FROM events ORDER BY rowid DESC LIMIT 1")?;
    let result: SqliteResult<String> = stmt.query_row([], |row| row.get(0));
    match result {
        Ok(hash) => Ok(hash),
        _ => Ok(GENESIS_PREV_HASH.to_string()),
    }
}

/// Inserts an event, updates hash chain, and optionally signs with Ed25519.
pub fn insert_event_signed(
    conn: &Connection,
    event: &mut TraceEvent,
    identity: Option<&NodeIdentity>,
) -> SqliteResult<()> {
    // 1. Chaining: attach to latest block hash (server-authoritative)
    event.prev_hash = get_latest_event_hash(conn)?;

    // 2. Compute canonical V2 SHA-256 event hash
    event.event_hash = compute_event_hash_v2(event);

    // 3. Attach cryptographic Ed25519 node signature if identity is provided
    if let Some(id) = identity {
        event.signature = Some(id.sign_event(&event.event_hash));
        event.public_key = Some(id.public_key_hex().to_string());
    }

    let details_json = serde_json::to_string(&event.action_details).unwrap_or_default();
    let sources_json = serde_json::to_string(&event.provenance_sources).unwrap_or_default();
    let reasons_json = serde_json::to_string(&event.security_reasons).unwrap_or_default();
    let event_type_str = format!("{:?}", event.event_type);
    let verdict_str = format!("{:?}", event.security_verdict);

    conn.execute(
        "INSERT INTO events (
            event_id, node_id, session_id, timestamp, event_type,
            target, action_details, diff, provenance_sources,
            security_verdict, security_reasons, duration_ms,
            prev_hash, event_hash, signature, public_key
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            event.event_id,
            event.node_id,
            event.session_id,
            event.timestamp,
            event_type_str,
            event.target,
            details_json,
            event.diff,
            sources_json,
            verdict_str,
            reasons_json,
            event.duration_ms,
            event.prev_hash,
            event.event_hash,
            event.signature,
            event.public_key,
        ],
    )?;

    Ok(())
}

/// Returns recently recorded events as JSON objects.
pub fn get_recent_events(conn: &Connection, limit: usize) -> SqliteResult<Vec<Value>> {
    get_recent_events_filtered(conn, EventFilterCriteria { limit, ..Default::default() })
}

/// Returns filtered events matching criteria.
pub fn get_recent_events_filtered(
    conn: &Connection,
    criteria: EventFilterCriteria<'_>,
) -> SqliteResult<Vec<Value>> {
    let mut conditions = Vec::new();
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(nid) = criteria.node_id {
        let trimmed = nid.trim();
        if !trimmed.is_empty() && trimmed != "ALL" {
            if trimmed == "PC-1" {
                conditions.push("(node_id = ? OR node_id = 'DEFAULT')");
                params_vec.push(Box::new(trimmed.to_string()));
            } else if trimmed == "PC-1-Antigravity" {
                conditions.push("node_id = ?");
                params_vec.push(Box::new(trimmed.to_string()));
            } else {
                conditions.push("(node_id = ? OR node_id LIKE ?)");
                params_vec.push(Box::new(trimmed.to_string()));
                params_vec.push(Box::new(format!("{}%", trimmed)));
            }
        }
    }

    if let Some(et) = criteria.event_type {
        let trimmed_et = et.trim();
        if !trimmed_et.is_empty() && trimmed_et != "ALL" {
            conditions.push("event_type = ?");
            params_vec.push(Box::new(trimmed_et.to_string()));
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT event_id, node_id, session_id, timestamp, event_type,
                target, action_details, diff, provenance_sources,
                security_verdict, security_reasons, duration_ms,
                prev_hash, event_hash, signature, public_key
         FROM events {} ORDER BY rowid DESC LIMIT ?",
        where_clause
    );

    params_vec.push(Box::new(criteria.limit as i64));

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        let details: String = row.get(6)?;
        let sources: String = row.get(8)?;
        let reasons: String = row.get(10)?;

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
        }))
    })?;

    let mut events = Vec::new();
    for r in rows {
        events.push(r?);
    }
    Ok(events)
}
