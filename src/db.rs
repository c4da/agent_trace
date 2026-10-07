use crate::ledger::{
    compute_event_hash_v1, compute_event_hash_v2, compute_event_hash_v2_raw,
    LedgerVerificationReport, NodeIdentity, GENESIS_PREV_HASH,
};
use crate::schema::{ClusterSummary, NodeHeartbeat, TraceEvent, EventType, SecurityVerdict};
use rusqlite::{params, Connection, Result as SqliteResult};
use std::sync::{Arc, Mutex};
use serde_json::Value;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(db_path: &str) -> SqliteResult<Self> {
        let conn = Connection::open(db_path)?;
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
        )?;

        Ok(Database {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    #[allow(dead_code)]
    pub fn get_latest_event_hash(&self) -> SqliteResult<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT event_hash FROM events ORDER BY rowid DESC LIMIT 1")?;
        let result: SqliteResult<String> = stmt.query_row([], |row| row.get(0));
        match result {
            Ok(hash) => Ok(hash),
            _ => Ok(GENESIS_PREV_HASH.to_string()),
        }
    }

    pub fn insert_event(&self, event: &mut TraceEvent) -> SqliteResult<()> {
        self.insert_event_signed(event, None)
    }

    pub fn insert_event_signed(&self, event: &mut TraceEvent, identity: Option<&NodeIdentity>) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

        // 1. Chaining: attach to latest block hash (ALWAYS server-authoritative)
        let mut stmt = conn.prepare("SELECT event_hash FROM events ORDER BY rowid DESC LIMIT 1")?;
        let latest_hash: String = match stmt.query_row([], |r| r.get(0)) {
            Ok(h) => h,
            Err(_) => GENESIS_PREV_HASH.to_string(),
        };

        event.prev_hash = latest_hash;

        // 2. Compute canonical V2 SHA-256 event hash (covers all security fields)
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

    pub fn upsert_node(&self, node: &NodeHeartbeat) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
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

    pub fn delete_node(&self, node_id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM nodes WHERE node_id = ?1", params![node_id])?;
        Ok(())
    }

    pub fn get_recent_events(&self, limit: usize) -> SqliteResult<Vec<Value>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT event_id, node_id, session_id, timestamp, event_type,
                    target, action_details, diff, provenance_sources,
                    security_verdict, security_reasons, duration_ms,
                    prev_hash, event_hash, signature, public_key
             FROM events ORDER BY rowid DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
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

    pub fn verify_ledger(&self) -> SqliteResult<LedgerVerificationReport> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT event_id, node_id, session_id, timestamp, event_type,
                    target, action_details, diff, provenance_sources,
                    security_verdict, security_reasons, duration_ms,
                    prev_hash, event_hash, signature, public_key
             FROM events ORDER BY rowid ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            let details_str: String = row.get(6)?;
            let sources_str: String = row.get(8)?;
            let reasons_str: String = row.get(10)?;
            let ev_type_str: String = row.get(4)?;
            let sec_verdict_str: String = row.get(9)?;

            let event_type = match ev_type_str.as_str() {
                "FileRead" => EventType::FileRead,
                "FileWrite" => EventType::FileWrite,
                "CommandRun" => EventType::CommandRun,
                "WebRequest" => EventType::WebRequest,
                "DecisionStep" => EventType::DecisionStep,
                _ => EventType::LlmInference,
            };

            let security_verdict = match sec_verdict_str.as_str() {
                "Flagged" => SecurityVerdict::Flagged,
                "Blocked" => SecurityVerdict::Blocked,
                _ => SecurityVerdict::Allowed,
            };

            Ok((
                TraceEvent {
                    event_id: row.get(0)?,
                    node_id: row.get(1)?,
                    session_id: row.get(2)?,
                    timestamp: row.get(3)?,
                    event_type,
                    target: row.get(5)?,
                    action_details: serde_json::from_str(&details_str).unwrap_or_default(),
                    diff: row.get(7)?,
                    provenance_sources: serde_json::from_str(&sources_str).unwrap_or_default(),
                    security_verdict,
                    security_reasons: serde_json::from_str(&reasons_str).unwrap_or_default(),
                    duration_ms: row.get(11)?,
                    prev_hash: row.get(12)?,
                    event_hash: row.get(13)?,
                    signature: row.get(14)?,
                    public_key: row.get(15)?,
                },
                details_str,
                sources_str,
                reasons_str,
            ))
        })?;

        let mut is_valid = true;
        let mut expected_prev_hash = GENESIS_PREV_HASH.to_string();
        let mut total_blocks = 0;
        let mut broken_block_id = None;
        let mut error_reason = None;

        for r in rows {
            let (ev, raw_details, raw_sources, raw_reasons) = r?;
            total_blocks += 1;

            // 1. Check prev_hash continuity
            if ev.prev_hash != expected_prev_hash {
                is_valid = false;
                broken_block_id = Some(ev.event_id.clone());
                error_reason = Some(format!(
                    "Broken hash chain at block {}: expected prev_hash {}, got {}",
                    ev.event_id, expected_prev_hash, ev.prev_hash
                ));
                break;
            }

            // 2. Recompute and check event_hash (supports canonical V2 or legacy V1)
            let computed_v2 = compute_event_hash_v2_raw(
                &ev.prev_hash,
                &ev.event_id,
                &ev.node_id,
                &ev.session_id,
                &ev.timestamp,
                &ev.event_type,
                &ev.target,
                &raw_details,
                ev.diff.as_deref(),
                &raw_sources,
                &ev.security_verdict,
                &raw_reasons,
                ev.duration_ms,
            );
            let matches = (computed_v2 == ev.event_hash) || (compute_event_hash_v1(&ev) == ev.event_hash);
            if !matches {
                is_valid = false;
                broken_block_id = Some(ev.event_id.clone());
                error_reason = Some(format!(
                    "Tampered event payload at block {}: computed hash {}, recorded hash {}",
                    ev.event_id, computed_v2, ev.event_hash
                ));
                break;
            }

            // 3. Verify Ed25519 signature if attached
            if let (Some(ref sig), Some(ref pk)) = (&ev.signature, &ev.public_key) {
                if !NodeIdentity::verify_signature(&ev.event_hash, sig, pk) {
                    is_valid = false;
                    broken_block_id = Some(ev.event_id.clone());
                    error_reason = Some(format!("Invalid Ed25519 signature at block {}", ev.event_id));
                    break;
                }
            }

            expected_prev_hash = ev.event_hash.clone();
        }

        Ok(LedgerVerificationReport {
            is_valid,
            total_blocks,
            genesis_hash: GENESIS_PREV_HASH.to_string(),
            latest_block_hash: expected_prev_hash,
            broken_block_id,
            error_reason,
        })
    }

    pub fn get_cluster_summary(&self) -> SqliteResult<ClusterSummary> {
        let (nodes, total_events, total_reads, total_writes, total_commands, security_alerts, blast_radius_score) = {
            let conn = self.conn.lock().unwrap();

            let mut stmt_nodes = conn.prepare("SELECT node_id, hostname, ip_address, role, status, last_seen, public_key FROM nodes")?;
            let node_rows = stmt_nodes.query_map([], |row| {
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
            for n in node_rows {
                nodes.push(n?);
            }

            let total_events: u64 = conn.query_row("SELECT count(*) FROM events", [], |r| r.get(0)).unwrap_or(0);
            let total_reads: u64 = conn.query_row("SELECT count(*) FROM events WHERE event_type = 'FileRead'", [], |r| r.get(0)).unwrap_or(0);
            let total_writes: u64 = conn.query_row("SELECT count(*) FROM events WHERE event_type = 'FileWrite'", [], |r| r.get(0)).unwrap_or(0);
            let total_commands: u64 = conn.query_row("SELECT count(*) FROM events WHERE event_type = 'CommandRun'", [], |r| r.get(0)).unwrap_or(0);
            let security_alerts: u64 = conn.query_row("SELECT count(*) FROM events WHERE security_verdict != 'Allowed'", [], |r| r.get(0)).unwrap_or(0);

            let blast_radius_score = ((total_writes * 10 + total_commands * 5 + security_alerts * 25).min(100)) as f64;
            (nodes, total_events, total_reads, total_writes, total_commands, security_alerts, blast_radius_score)
        };

        let verification = match self.verify_ledger() {
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

    /// Computes multi-dimensional telemetry analytics across trace events in the ledger.
    pub fn get_telemetry_analytics(&self) -> SqliteResult<crate::analytics::TelemetryAnalyticsReport> {
        let (total_events, top_read_targets, top_write_targets, agent_profiles, time_series, security_anomalies) = {
            let conn = self.conn.lock().unwrap();

            let total_events: u64 = conn
                .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
                .unwrap_or(0);

            // 1. Top Read Targets Hotspots
            let mut top_read_stmt = conn.prepare(
                "SELECT target, count(*) as cnt, max(security_verdict)
                 FROM events WHERE event_type = 'FileRead'
                 GROUP BY target ORDER BY cnt DESC LIMIT 10",
            )?;
            let top_read_targets: Vec<crate::analytics::TargetHotspot> = top_read_stmt
                .query_map([], |row| {
                    let verdict: String = row.get(2)?;
                    Ok(crate::analytics::TargetHotspot {
                        target: row.get(0)?,
                        event_type: EventType::FileRead,
                        count: row.get(1)?,
                        risk_flagged: verdict != "Allowed",
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            // 2. Top Write Targets Hotspots
            let mut top_write_stmt = conn.prepare(
                "SELECT target, count(*) as cnt, max(security_verdict)
                 FROM events WHERE event_type = 'FileWrite'
                 GROUP BY target ORDER BY cnt DESC LIMIT 10",
            )?;
            let top_write_targets: Vec<crate::analytics::TargetHotspot> = top_write_stmt
                .query_map([], |row| {
                    let verdict: String = row.get(2)?;
                    Ok(crate::analytics::TargetHotspot {
                        target: row.get(0)?,
                        event_type: EventType::FileWrite,
                        count: row.get(1)?,
                        risk_flagged: verdict != "Allowed",
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            // 3. Agent Behavioral Profiles & Threat Scores
            let mut agent_stmt = conn.prepare(
                "SELECT node_id, count(*),
                        sum(case when event_type = 'FileRead' then 1 else 0 end),
                        sum(case when event_type = 'FileWrite' then 1 else 0 end),
                        sum(case when event_type = 'CommandRun' then 1 else 0 end),
                        sum(case when event_type = 'LlmInference' then 1 else 0 end),
                        sum(case when security_verdict != 'Allowed' then 1 else 0 end)
                 FROM events GROUP BY node_id ORDER BY count(*) DESC",
            )?;
            let agent_profiles: Vec<crate::analytics::AgentProfile> = agent_stmt
                .query_map([], |row| {
                    let agent_id: String = row.get(0)?;
                    let total: u64 = row.get(1)?;
                    let reads: u64 = row.get(2)?;
                    let writes: u64 = row.get(3)?;
                    let commands: u64 = row.get(4)?;
                    let llm_calls: u64 = row.get(5)?;
                    let violations: u64 = row.get(6)?;

                    let (blast_radius_score, threat_level) =
                        crate::analytics::AgentProfile::compute_threat(reads, writes, commands, violations);
                    let read_to_write_ratio =
                        crate::analytics::AgentProfile::compute_ratio(reads, writes);

                    Ok(crate::analytics::AgentProfile {
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
                })?
                .filter_map(|r| r.ok())
                .collect();

            // 4. Time Series Activity Bucketing
            let mut ts_stmt = conn.prepare(
                "SELECT substr(timestamp, 1, 13) || ':00' as bucket,
                        sum(case when event_type = 'FileRead' then 1 else 0 end),
                        sum(case when event_type = 'FileWrite' then 1 else 0 end),
                        sum(case when event_type = 'CommandRun' then 1 else 0 end),
                        sum(case when event_type = 'LlmInference' then 1 else 0 end),
                        sum(case when security_verdict != 'Allowed' then 1 else 0 end)
                 FROM events GROUP BY bucket ORDER BY bucket DESC LIMIT 24",
            )?;
            let time_series: Vec<crate::analytics::TimeSeriesBucket> = ts_stmt
                .query_map([], |row| {
                    Ok(crate::analytics::TimeSeriesBucket {
                        bucket: row.get(0)?,
                        reads: row.get(1)?,
                        writes: row.get(2)?,
                        commands: row.get(3)?,
                        llm_calls: row.get(4)?,
                        alerts: row.get(5)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            // 5. Security Anomalies Breakdown
            let mut anomaly_stmt = conn.prepare(
                "SELECT security_verdict, security_reasons, count(*), max(timestamp), target
                 FROM events WHERE security_verdict != 'Allowed'
                 GROUP BY security_verdict, security_reasons ORDER BY count(*) DESC LIMIT 10",
            )?;
            let security_anomalies: Vec<crate::analytics::SecurityAnomaly> = anomaly_stmt
                .query_map([], |row| {
                    let sec_str: String = row.get(0)?;
                    let reasons_json: String = row.get(1)?;
                    let verdict = if sec_str == "Blocked" {
                        SecurityVerdict::Blocked
                    } else {
                        SecurityVerdict::Flagged
                    };
                    let parsed_reasons: Vec<String> = serde_json::from_str(&reasons_json).unwrap_or_default();
                    let reason = parsed_reasons.first().cloned().unwrap_or_else(|| "Security Policy Alert".to_string());

                    Ok(crate::analytics::SecurityAnomaly {
                        verdict,
                        reason,
                        count: row.get(2)?,
                        latest_timestamp: row.get(3)?,
                        sample_target: row.get(4)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            (total_events, top_read_targets, top_write_targets, agent_profiles, time_series, security_anomalies)
        };

        let rep = self.verify_ledger().unwrap_or(LedgerVerificationReport {
            is_valid: false,
            total_blocks: 0,
            genesis_hash: GENESIS_PREV_HASH.to_string(),
            latest_block_hash: GENESIS_PREV_HASH.to_string(),
            broken_block_id: None,
            error_reason: None,
        });

        Ok(crate::analytics::TelemetryAnalyticsReport {
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
}