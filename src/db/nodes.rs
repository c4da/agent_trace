//! Cluster node tracking and heartbeat management in SQLite.

use crate::schema::NodeHeartbeat;
use rusqlite::{params, Connection, Result as SqliteResult};

/// Inserts or updates a node heartbeat.
pub fn upsert_node(conn: &Connection, node: &NodeHeartbeat) -> SqliteResult<()> {
    conn.execute(
        "INSERT INTO nodes (node_id, hostname, ip_address, role, status, last_seen, public_key)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(node_id) DO UPDATE SET
             hostname = excluded.hostname,
             ip_address = excluded.ip_address,
             role = excluded.role,
             status = excluded.status,
             last_seen = excluded.last_seen,
             public_key = excluded.public_key",
        params![
            node.node_id,
            node.hostname,
            node.ip_address,
            node.role,
            node.status,
            node.last_seen,
            node.public_key,
        ],
    )?;
    Ok(())
}

/// Deletes a node by its ID.
pub fn delete_node(conn: &Connection, node_id: &str) -> SqliteResult<()> {
    conn.execute("DELETE FROM nodes WHERE node_id = ?1", params![node_id])?;
    Ok(())
}

/// Retrieves all recorded cluster nodes.
pub fn get_all_nodes(conn: &Connection) -> SqliteResult<Vec<NodeHeartbeat>> {
    let mut stmt = conn.prepare(
        "SELECT node_id, hostname, ip_address, role, status, last_seen, public_key FROM nodes",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(NodeHeartbeat {
            node_id: row.get(0)?,
            hostname: row.get(1)?,
            ip_address: row.get(2)?,
            role: row.get(3)?,
            status: row.get(4)?,
            last_seen: row.get(5)?,
            public_key: row.get(6)?,
            models: None,
        })
    })?;

    let mut nodes = Vec::new();
    for n in rows {
        nodes.push(n?);
    }
    Ok(nodes)
}

/// Updates a node's operational status and last_seen timestamp.
pub fn update_node_status(conn: &Connection, node_id: &str, status: &str, last_seen: &str) -> SqliteResult<bool> {
    let rows_affected = conn.execute(
        "UPDATE nodes SET status = ?1, last_seen = ?2 WHERE node_id = ?3",
        params![status, last_seen, node_id],
    )?;
    Ok(rows_affected > 0)
}
