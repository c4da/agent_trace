//! Bounded Asynchronous Channel Ledger Writer for AgentTrace Gateway.
//!
//! Replaces synchronous `Mutex<Connection>` async locking with an actor-style
//! dedicated background writer task. Enforces strict chronological hash chain sequencing,
//! eliminates SQLite lock contention on Tokio request worker threads, and provides
//! bounded backpressure to prevent out-of-memory errors during multi-agent telemetry spikes.

use crate::db::Database;
use crate::ledger::NodeIdentity;
use crate::schema::TraceEvent;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{debug, error, info};

/// Command payload submitted to the dedicated ledger writer actor.
pub enum LedgerWriteCommand {
    /// Fire-and-forget ingestion: Enqueues event for non-blocking disk commit.
    FireAndForget(TraceEvent),
    /// Acknowledged ingestion: Waits until the event is signed, hash-chained, and committed.
    Acknowledged {
        event: TraceEvent,
        reply: oneshot::Sender<Result<TraceEvent, String>>,
    },
}

/// Actor handle for the asynchronous ledger writer.
/// Implements `Clone` and can be shared freely across Axum handlers and background watchers.
#[derive(Clone)]
pub struct LedgerWriter {
    tx: mpsc::Sender<LedgerWriteCommand>,
}

impl LedgerWriter {
    /// Spawns the dedicated background ledger writer task and returns the client handle.
    ///
    /// # Arguments
    /// * `db` - SQLite database handle with WAL mode enabled.
    /// * `identity` - Optional Ed25519 node identity for cryptographic signing.
    /// * `tx_events` - Optional broadcast channel to stream committed events to live SSE subscribers.
    /// * `capacity` - Maximum bounded buffer size (e.g. 1024).
    pub fn spawn(
        db: Database,
        identity: Option<Arc<NodeIdentity>>,
        tx_events: Option<broadcast::Sender<String>>,
        capacity: usize,
    ) -> (Self, tokio::task::JoinHandle<()>) {
        let (tx, mut rx) = mpsc::channel::<LedgerWriteCommand>(capacity);

        let handle = tokio::spawn(async move {
            debug!("[LedgerWriter] Background writer task spawned (capacity: {})", capacity);
            let mut batch = Vec::with_capacity(32);

            while let Some(cmd) = rx.recv().await {
                batch.push(cmd);

                // Drain any additional immediately available commands up to batch capacity (32)
                while batch.len() < 32 {
                    match rx.try_recv() {
                        Ok(next) => batch.push(next),
                        Err(_) => break,
                    }
                }

                // Process batch sequentially under authoritative lock
                for item in batch.drain(..) {
                    match item {
                        LedgerWriteCommand::FireAndForget(mut ev) => {
                            if let Err(e) = db.insert_event_signed(&mut ev, identity.as_deref()) {
                                error!("[-] [LedgerWriter] Failed to commit fire-and-forget event {}: {}", ev.event_id, e);
                            } else if let Some(ref sse_tx) = tx_events {
                                if let Ok(json_str) = serde_json::to_string(&ev) {
                                    let _ = sse_tx.send(json_str);
                                }
                            }
                        }
                        LedgerWriteCommand::Acknowledged { mut event, reply } => {
                            match db.insert_event_signed(&mut event, identity.as_deref()) {
                                Ok(_) => {
                                    if let Some(ref sse_tx) = tx_events {
                                        if let Ok(json_str) = serde_json::to_string(&event) {
                                            let _ = sse_tx.send(json_str);
                                        }
                                    }
                                    let _ = reply.send(Ok(event));
                                }
                                Err(e) => {
                                    error!("[-] [LedgerWriter] Failed to commit acknowledged event {}: {}", event.event_id, e);
                                    let _ = reply.send(Err(e.to_string()));
                                }
                            }
                        }
                    }
                }
            }

            info!("[LedgerWriter] Ingestion channel closed. Writer task terminated cleanly.");
        });

        (Self { tx }, handle)
    }

    /// Non-blocking fire-and-forget enqueue. Returns `Err` if the bounded buffer is full.
    pub fn try_enqueue(&self, event: TraceEvent) -> Result<(), String> {
        self.tx
            .try_send(LedgerWriteCommand::FireAndForget(event))
            .map_err(|e| e.to_string())
    }

    /// Asynchronous fire-and-forget enqueue with backpressure. Waits if buffer is currently full.
    pub async fn enqueue(&self, event: TraceEvent) -> Result<(), String> {
        self.tx
            .send(LedgerWriteCommand::FireAndForget(event))
            .await
            .map_err(|e| e.to_string())
    }

    /// Asynchronously commits an event and waits for SQLite insertion, signature, and hash chaining.
    pub async fn commit_acknowledged(&self, event: TraceEvent) -> Result<TraceEvent, String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let cmd = LedgerWriteCommand::Acknowledged {
            event,
            reply: reply_tx,
        };

        self.tx.send(cmd).await.map_err(|e| e.to_string())?;

        reply_rx
            .await
            .map_err(|_| "Ledger writer dropped acknowledgment responder".to_string())?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{EventType, SecurityVerdict};
    use tempfile::NamedTempFile;

    fn sample_event(id: &str) -> TraceEvent {
        TraceEvent {
            event_id: id.to_string(),
            node_id: "TEST-NODE".to_string(),
            session_id: "test-session".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            event_type: EventType::CommandRun,
            target: "cargo test".to_string(),
            action_details: serde_json::json!({"cmd": "cargo test"}),
            diff: None,
            provenance_sources: vec!["unit_test".to_string()],
            security_verdict: SecurityVerdict::Allowed,
            security_reasons: vec![],
            duration_ms: 12.5,
            prev_hash: String::new(),
            event_hash: String::new(),
            signature: None,
            public_key: None,
        }
    }

    #[tokio::test]
    async fn test_ledger_writer_acknowledged_commit() {
        let tmp = NamedTempFile::new().unwrap();
        let db = Database::new(tmp.path().to_str().unwrap()).unwrap();

        let key_tmp = NamedTempFile::new().unwrap();
        let identity = Arc::new(NodeIdentity::load_or_generate(key_tmp.path().to_str().unwrap()).unwrap());

        let (writer, _handle) = LedgerWriter::spawn(db.clone(), Some(identity.clone()), None, 64);

        let ev1 = sample_event("ev_1");
        let committed_1 = writer.commit_acknowledged(ev1).await.expect("commit 1 ok");
        assert!(!committed_1.event_hash.is_empty());
        assert!(committed_1.signature.is_some());

        let ev2 = sample_event("ev_2");
        let committed_2 = writer.commit_acknowledged(ev2).await.expect("commit 2 ok");
        assert_eq!(committed_2.prev_hash, committed_1.event_hash);

        // Verify ledger integrity
        let report = db.verify_ledger().expect("ledger verify ok");
        assert!(report.is_valid);
        assert_eq!(report.total_blocks, 2);
    }

    #[tokio::test]
    async fn test_ledger_writer_fire_and_forget_batching() {
        let tmp = NamedTempFile::new().unwrap();
        let db = Database::new(tmp.path().to_str().unwrap()).unwrap();

        let (writer, _handle) = LedgerWriter::spawn(db.clone(), None, None, 128);

        for i in 0..10 {
            writer.enqueue(sample_event(&format!("batch_{}", i))).await.unwrap();
        }

        // Give worker a moment to drain batch
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let report = db.verify_ledger().expect("ledger verify ok");
        assert!(report.is_valid);
        assert_eq!(report.total_blocks, 10);
    }
}
