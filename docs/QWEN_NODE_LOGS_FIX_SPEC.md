# AgentTrace Gateway — Node Filtering & Logs Display Fix

## Root Cause Summary

The `/api/v1/events` endpoint returns only the latest N events globally. Because `PC-1-Antigravity` emits ~7,700 events, the top-100 window is 100% that node. Client-side filtering then finds zero rows for `PC-1` or `PC-2`. The fix is **server-side filtering** via parameterized SQL.

---

## 1. Rust Backend

### `src/handlers.rs`

```rust
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::state::AppState;

// ─── Query Parameters ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct EventQuery {
    /// Exact node identifier, e.g. "PC-1", "PC-2", "PC-1-Antigravity".
    /// When `None`, returns the global recent mix.
    pub node_id: Option<String>,

    /// Optional event-type filter, e.g. "inference", "tool_call", "error".
    pub event_type: Option<String>,

    /// Page size (clamped 1..=500). Default 50.
    pub limit: Option<usize>,
}

// ─── Handler ────────────────────────────────────────────────────────────────────

/// GET /api/v1/events?node_id=PC-1&event_type=inference&limit=100
pub async fn get_events(
    State(state): State<AppState>,
    Query(q): Query<EventQuery>,
) -> Result<Json<Vec<Value>>, StatusCode> {
    let limit = q.limit.unwrap_or(50).clamp(1, 500);

    let node_id: Option<&str> = q.node_id.as_deref();
    let event_type: Option<&str> = q.event_type.as_deref();

    let events = state
        .db
        .get_recent_events_filtered(limit, node_id, event_type)
        .map_err(|e| {
            tracing::error!("DB query failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(events))
}

// ─── Node listing (for the filter pills) ───────────────────────────────────────

/// GET /api/v1/nodes  →  [{ "node_id": "PC-1", "event_count": 70 }, …]
pub async fn get_nodes(
    State(state): State<AppState>,
) -> Result<Json<Vec<Value>>, StatusCode> {
    let nodes = state
        .db
        .get_node_summary()
        .map_err(|e| {
            tracing::error!("Node summary query failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(nodes))
}
```

### `src/db.rs`

```rust
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // ──────────────────────────────────────────────────────────────────────────
    //  Core filtered query (replaces the old get_recent_events)
    // ──────────────────────────────────────────────────────────────────────────

    /// Returns up to `limit` most-recent events, optionally scoped to a single
    /// `node_id` and/or `event_type`.  All parameters are bound via `?`
    /// placeholders — **no string interpolation** — to prevent SQL injection.
    pub fn get_recent_events_filtered(
        &self,
        limit: usize,
        node_id: Option<&str>,
        event_type: Option<&str>,
    ) -> Result<Vec<Value>, rusqlite::Error> {
        let conn = self.conn.lock().expect("db mutex poisoned");

        // Build the WHERE clause dynamically but safely.
        let mut conditions: Vec<String> = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(nid) = node_id {
            conditions.push("node_id = ?".to_string());
            param_values.push(Box::new(nid.to_string()));
        }
        if let Some(et) = event_type {
            conditions.push("event_type = ?".to_string());
            param_values.push(Box::new(et.to_string()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id, node_id, event_type, payload, created_at
             FROM cluster_telemetry
             {}
             ORDER BY created_at DESC, id DESC
             LIMIT ?",
            where_clause
        );

        // Append LIMIT as the final bound parameter.
        param_values.push(Box::new(limit as i64));

        let mut stmt = conn.prepare(&sql)?;

        // rusqlite's `query` accepts a slice of `&dyn ToSql`.
        let param_refs: Vec<&dyn rusqlite::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let id: i64 = row.get(0)?;
            let node_id: String = row.get(1)?;
            let event_type: String = row.get(2)?;
            let payload_raw: String = row.get(3)?;
            let created_at: String = row.get(4)?;

            // Parse the JSON payload column into a Value (fall back to string).
            let payload: Value =
                serde_json::from_str(&payload_raw).unwrap_or(Value::String(payload_raw));

            Ok(Value::Object(
                serde_json::Map::from_iter([
                    ("id".into(), Value::from(id)),
                    ("node_id".into(), Value::from(node_id)),
                    ("event_type".into(), Value::from(event_type)),
                    ("payload".into(), payload),
                    ("created_at".