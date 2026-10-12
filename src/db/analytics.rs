//! Telemetry analytics and cluster summary queries in SQLite.

use crate::analytics::{
    AgentProfile, SecurityAnomaly, TargetHotspot, TelemetryAnalyticsReport, TimeSeriesBucket,
};
use crate::ledger::{LedgerVerificationReport, GENESIS_PREV_HASH};
use crate::schema::{ClusterSummary, EventType, SecurityVerdict};
use rusqlite::{Connection, Result as SqliteResult};

/// Builds a cluster health summary from node heartbeats and ledger counts.
pub fn get_cluster_summary(conn: &Connection) -> SqliteResult<ClusterSummary> {
    let nodes = super::nodes::get_all_nodes(conn)?;

    let total_events: u64 = conn.query_row("SELECT count(*) FROM events", [], |r| r.get(0)).unwrap_or(0);
    let total_reads: u64 = conn.query_row("SELECT count(*) FROM events WHERE event_type = 'FileRead'", [], |r| r.get(0)).unwrap_or(0);
    let total_writes: u64 = conn.query_row("SELECT count(*) FROM events WHERE event_type = 'FileWrite'", [], |r| r.get(0)).unwrap_or(0);
    let total_commands: u64 = conn.query_row("SELECT count(*) FROM events WHERE event_type = 'CommandRun'", [], |r| r.get(0)).unwrap_or(0);
    let security_alerts: u64 = super::alerts::count_active_alerts(conn);

    let blast_radius_score = ((total_writes * 10 + total_commands * 5 + security_alerts * 25).min(100)) as f64;

    let verification = match super::verify::verify_ledger(conn) {
        Ok(rep) => rep,
        Err(e) => LedgerVerificationReport {
            is_valid: false,
            total_blocks: 0,
            genesis_hash: GENESIS_PREV_HASH.to_string(),
            latest_block_hash: GENESIS_PREV_HASH.to_string(),
            broken_block_id: None,
            error_reason: Some(format!("Ledger verification query error: {}", e)),
        },
    };

    Ok(ClusterSummary {
        total_nodes: nodes.len(),
        active_nodes: nodes,
        total_events,
        total_reads,
        total_writes,
        total_commands,
        security_alerts,
        blast_radius_score,
        latest_ledger_hash: verification.latest_block_hash,
        ledger_verified: verification.is_valid,
    })
}

/// Computes multi-dimensional telemetry analytics across trace events.
pub fn get_telemetry_analytics(conn: &Connection) -> SqliteResult<TelemetryAnalyticsReport> {
    let total_events: u64 = conn.query_row("SELECT count(*) FROM events", [], |r| r.get(0)).unwrap_or(0);
    let top_read_targets = query_top_hotspots(conn, "FileRead")?;
    let top_write_targets = query_top_hotspots(conn, "FileWrite")?;
    let agent_profiles = query_agent_profiles(conn)?;
    let time_series = query_time_series(conn)?;
    let security_anomalies = query_security_anomalies(conn)?;

    let rep = super::verify::verify_ledger(conn).unwrap_or(LedgerVerificationReport {
        is_valid: false,
        total_blocks: 0,
        genesis_hash: GENESIS_PREV_HASH.to_string(),
        latest_block_hash: GENESIS_PREV_HASH.to_string(),
        broken_block_id: None,
        error_reason: None,
    });

    Ok(TelemetryAnalyticsReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        total_events,
        top_read_targets,
        top_write_targets,
        agent_profiles,
        time_series,
        security_anomalies,
        ledger_valid: rep.is_valid,
        ledger_head: rep.latest_block_hash,
    })
}

/// Queries top target hotspots for a specific event type.
fn query_top_hotspots(conn: &Connection, event_type: &str) -> SqliteResult<Vec<TargetHotspot>> {
    let mut stmt = conn.prepare(
        "SELECT target, count(*) as cnt, max(security_verdict)
         FROM events WHERE event_type = ?1
         GROUP BY target ORDER BY cnt DESC LIMIT 10",
    )?;
    let mapped = stmt.query_map([event_type], |row| {
        let verdict: String = row.get(2)?;
        let ev_type = if event_type == "FileRead" { EventType::FileRead } else { EventType::FileWrite };
        Ok(TargetHotspot {
            target: row.get(0)?,
            event_type: ev_type,
            count: row.get(1)?,
            risk_flagged: verdict != "Allowed",
        })
    })?;
    Ok(mapped.filter_map(|r| r.ok()).collect())
}

/// Queries behavioral profile metrics grouped by agent/node ID.
fn query_agent_profiles(conn: &Connection) -> SqliteResult<Vec<AgentProfile>> {
    let mut stmt = conn.prepare(
        "SELECT node_id, count(*),
                sum(case when event_type = 'FileRead' then 1 else 0 end),
                sum(case when event_type = 'FileWrite' then 1 else 0 end),
                sum(case when event_type = 'CommandRun' then 1 else 0 end),
                sum(case when event_type = 'LlmInference' then 1 else 0 end),
                sum(case when security_verdict != 'Allowed' then 1 else 0 end)
         FROM events GROUP BY node_id ORDER BY count(*) DESC",
    )?;
    let mapped = stmt.query_map([], |row| {
        let agent_id: String = row.get(0)?;
        let total: u64 = row.get(1)?;
        let reads: u64 = row.get(2)?;
        let writes: u64 = row.get(3)?;
        let commands: u64 = row.get(4)?;
        let llm_calls: u64 = row.get(5)?;
        let violations: u64 = row.get(6)?;

        let (blast_radius_score, threat_level) = AgentProfile::compute_threat(reads, writes, commands, violations);
        let read_to_write_ratio = AgentProfile::compute_ratio(reads, writes);

        Ok(AgentProfile {
            agent_id,
            total_events: total,
            reads,
            writes,
            commands,
            llm_inferences: llm_calls,
            violations,
            read_to_write_ratio,
            blast_radius_score,
            threat_level,
        })
    })?;
    Ok(mapped.filter_map(|r| r.ok()).collect())
}

/// Groups recent events into hourly time buckets.
fn query_time_series(conn: &Connection) -> SqliteResult<Vec<TimeSeriesBucket>> {
    let mut stmt = conn.prepare(
        "SELECT substr(timestamp, 1, 13) || ':00' as bucket,
                sum(case when event_type = 'FileRead' then 1 else 0 end),
                sum(case when event_type = 'FileWrite' then 1 else 0 end),
                sum(case when event_type = 'CommandRun' then 1 else 0 end),
                sum(case when event_type = 'LlmInference' then 1 else 0 end),
                sum(case when security_verdict != 'Allowed' then 1 else 0 end)
         FROM events GROUP BY bucket ORDER BY bucket DESC LIMIT 24",
    )?;
    let mapped = stmt.query_map([], |row| {
        Ok(TimeSeriesBucket {
            bucket: row.get(0)?,
            reads: row.get(1)?,
            writes: row.get(2)?,
            commands: row.get(3)?,
            llm_calls: row.get(4)?,
            alerts: row.get(5)?,
        })
    })?;
    Ok(mapped.filter_map(|r| r.ok()).collect())
}

/// Identifies top recorded security policy anomalies.
fn query_security_anomalies(conn: &Connection) -> SqliteResult<Vec<SecurityAnomaly>> {
    let mut stmt = conn.prepare(
        "SELECT security_verdict, security_reasons, count(*), max(timestamp), target
         FROM events WHERE security_verdict != 'Allowed'
         GROUP BY security_verdict, security_reasons ORDER BY count(*) DESC LIMIT 10",
    )?;
    let mapped = stmt.query_map([], |row| {
        let sec_str: String = row.get(0)?;
        let reasons_json: String = row.get(1)?;
        let verdict = if sec_str == "Blocked" {
            SecurityVerdict::Blocked
        } else {
            SecurityVerdict::Flagged
        };
        let parsed_reasons: Vec<String> = serde_json::from_str(&reasons_json).unwrap_or_default();
        let reason = parsed_reasons.first().cloned().unwrap_or_else(|| "Security Policy Alert".to_string());

        Ok(SecurityAnomaly {
            verdict,
            reason,
            count: row.get(2)?,
            latest_timestamp: row.get(3)?,
            sample_target: row.get(4)?,
        })
    })?;
    Ok(mapped.filter_map(|r| r.ok()).collect())
}
