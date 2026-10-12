//! Cryptographic verification of the SQLite ledger hash chain.

use crate::ledger::{
    compute_event_hash_v1, compute_event_hash_v2_raw, LedgerVerificationReport,
    NodeIdentity, RawEventFields, GENESIS_PREV_HASH,
};
use crate::schema::{EventType, SecurityVerdict, TraceEvent};
use rusqlite::{Connection, Result as SqliteResult};

/// Verifies hash chain continuity, event hashes, and Ed25519 signatures.
pub fn verify_ledger(conn: &Connection) -> SqliteResult<LedgerVerificationReport> {
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

        // 2. Recompute and check event_hash
        let raw_fields = RawEventFields {
            prev_hash: &ev.prev_hash,
            event_id: &ev.event_id,
            node_id: &ev.node_id,
            session_id: &ev.session_id,
            timestamp: &ev.timestamp,
            event_type: &ev.event_type,
            target: &ev.target,
            details_str: &raw_details,
            diff: ev.diff.as_deref(),
            sources_str: &raw_sources,
            security_verdict: &ev.security_verdict,
            reasons_str: &raw_reasons,
            duration_ms: ev.duration_ms,
        };
        let computed_v2 = compute_event_hash_v2_raw(&raw_fields);
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
