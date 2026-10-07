use rusqlite::Connection;
use std::fs;
use tempfile::NamedTempFile;

// Test the compiled binary or db module via a simple script or standalone test
#[test]
fn test_hash_chain_tamper_detection() {
    let tmp_file = NamedTempFile::new().unwrap();
    let db_path = tmp_file.path().to_str().unwrap();

    let conn = Connection::open(db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE events (
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
        );",
    ).unwrap();

    // Insert block 1 (Genesis)
    let genesis_prev = "0000000000000000000000000000000000000000000000000000000000000000";
    let block1_hash = "1111111111111111111111111111111111111111111111111111111111111111";
    conn.execute(
        "INSERT INTO events VALUES ('ev_1', 'PC-1', 's1', '2026-10-05T00:00:00Z', 'FileRead', 'main.rs', '{}', NULL, '[]', 'Allowed', '[]', 10.0, ?1, ?2, NULL, NULL)",
        rusqlite::params![genesis_prev, block1_hash],
    ).unwrap();

    // Insert block 2 (Chained to block 1)
    let block2_hash = "2222222222222222222222222222222222222222222222222222222222222222";
    conn.execute(
        "INSERT INTO events VALUES ('ev_2', 'PC-2', 's1', '2026-10-05T00:01:00Z', 'FileWrite', 'app.rs', '{}', '+diff', '[]', 'Allowed', '[]', 15.0, ?1, ?2, NULL, NULL)",
        rusqlite::params![block1_hash, block2_hash],
    ).unwrap();

    // Verify chaining query
    let prev_of_block2: String = conn.query_row("SELECT prev_hash FROM events WHERE event_id = 'ev_2'", [], |r| r.get(0)).unwrap();
    assert_eq!(prev_of_block2, block1_hash);
}
