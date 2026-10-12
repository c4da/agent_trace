//! Database table initialization and PRAGMA settings.

use rusqlite::{Connection, Result as SqliteResult};

/// Creates telemetry and node tables with WAL mode.
pub fn init_tables(conn: &Connection) -> SqliteResult<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         CREATE TABLE IF NOT EXISTS events (
             event_id TEXT PRIMARY KEY,
             node_id TEXT NOT NULL,
             session_id TEXT NOT NULL,
             timestamp TEXT NOT NULL,
             event_type TEXT NOT NULL,
             target TEXT NOT NULL,
             action_details TEXT NOT NULL,
             diff TEXT,
             provenance_sources TEXT NOT NULL,
             security_verdict TEXT NOT NULL,
             security_reasons TEXT NOT NULL,
             duration_ms REAL NOT NULL,
             prev_hash TEXT NOT NULL,
             event_hash TEXT NOT NULL,
             signature TEXT,
             public_key TEXT
         );
         CREATE INDEX IF NOT EXISTS idx_events_hash ON events(event_hash);
         CREATE INDEX IF NOT EXISTS idx_events_node ON events(node_id);

         CREATE TABLE IF NOT EXISTS nodes (
             node_id TEXT PRIMARY KEY,
             hostname TEXT NOT NULL,
             ip_address TEXT NOT NULL,
             role TEXT NOT NULL,
             status TEXT NOT NULL,
             last_seen TEXT NOT NULL,
             public_key TEXT
         );",
    )
}
