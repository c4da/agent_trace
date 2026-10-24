//! Enterprise SIEM & ELK Stack (Elasticsearch / Logstash / Kibana) ECS 8.11 Exporter (`src/siem_exporter.rs`).
//!
//! Converts AgentTrace cryptographic ledger and AI Guard events into standard
//! Elastic Common Schema (ECS 8.11) documents and Elasticsearch `/_bulk` NDJSON streams
//! for corporate SOC ingestion (Elastic, Splunk HEC, Datadog, Vector, Filebeat).

use crate::schema::{EventType, SecurityVerdict, TraceEvent};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;

/// Configuration for 3rd-party SIEM / ELK shipping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiemExportConfig {
    /// Whether live SIEM / ELK shipping is active.
    pub enabled: bool,
    /// Optional Elasticsearch or Logstash HTTP endpoint (e.g. `http://localhost:9200` or `https://elk.corp.internal:9200`).
    pub elk_endpoint: Option<String>,
    /// Target Elasticsearch index prefix (default: `agenttrace-security-events`).
    pub index_name: String,
    /// Optional API Key or Bearer token for Elasticsearch / Splunk HEC (`AGENTTRACE_ELK_API_KEY`).
    #[serde(skip_serializing)]
    pub api_key: Option<String>,
    /// Local NDJSON spool path for Filebeat / FluentBit / Vector tailing (`logs/agenttrace-ecs.ndjson`).
    pub local_ndjson_path: Option<String>,
}

impl Default for SiemExportConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl SiemExportConfig {
    pub fn from_env() -> Self {
        let elk_endpoint = std::env::var("AGENTTRACE_ELK_URL")
            .or_else(|_| std::env::var("ELASTICSEARCH_URL"))
            .ok()
            .filter(|s| !s.trim().is_empty());

        let index_name = std::env::var("AGENTTRACE_ELK_INDEX")
            .unwrap_or_else(|_| "agenttrace-security-events".to_string());

        let api_key = std::env::var("AGENTTRACE_ELK_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let local_ndjson_path = std::env::var("AGENTTRACE_ELK_SPOOL")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let enabled = elk_endpoint.is_some() || local_ndjson_path.is_some();

        Self {
            enabled,
            elk_endpoint,
            index_name,
            api_key,
            local_ndjson_path,
        }
    }
}

/// Converts a `TraceEvent` into an Elastic Common Schema (ECS 8.11) JSON object.
pub fn to_ecs_document(ev: &TraceEvent) -> serde_json::Value {
    let outcome = match ev.security_verdict {
        SecurityVerdict::Allowed => "success",
        SecurityVerdict::Blocked => "failure",
        SecurityVerdict::Flagged => "unknown",
    };

    let event_kind = match ev.security_verdict {
        SecurityVerdict::Allowed => "event",
        SecurityVerdict::Blocked | SecurityVerdict::Flagged => "alert",
    };

    let (category, ev_type) = match ev.event_type {
        EventType::FileRead => ("file", "access"),
        EventType::FileWrite => ("file", "change"),
        EventType::CommandRun => ("process", "start"),
        EventType::WebRequest => ("network", "connection"),
        EventType::DecisionStep => ("configuration", "change"),
        EventType::LlmInference => ("process", "info"),
    };

    // Ensure @timestamp always contains a full ISO-8601 date + time string
    let timestamp_iso = if ev.timestamp.contains('T') && ev.timestamp.len() >= 19 {
        ev.timestamp.clone()
    } else {
        chrono::Utc::now().to_rfc3339()
    };

    let duration_ns = (ev.duration_ms * 1_000_000.0) as u64;

    serde_json::json!({
        "@timestamp": timestamp_iso,
        "ecs": {
            "version": "8.11.0"
        },
        "event": {
            "id": ev.event_id,
            "kind": event_kind,
            "category": [category],
            "type": [ev_type],
            "outcome": outcome,
            "duration": duration_ns,
            "module": "agenttrace_ai_guard",
            "dataset": "agenttrace.ledger"
        },
        "agent": {
            "id": ev.node_id,
            "name": ev.node_id,
            "type": "agenttrace-gateway",
            "version": env!("CARGO_PKG_VERSION")
        },
        "host": {
            "name": ev.node_id.split('@').nth(1).and_then(|s| s.split('#').next()).unwrap_or(&ev.node_id)
        },
        "rule": {
            "name": format!("AgentTrace::{:?}", ev.security_verdict),
            "description": if ev.security_reasons.is_empty() {
                "Verified by AgentTrace Runtime Policy & AI Guard".to_string()
            } else {
                ev.security_reasons.join("; ")
            }
        },
        "message": format!(
            "[{}] Agent '{}' performed {:?} on '{}' (verdict: {:?})",
            timestamp_iso, ev.node_id, ev.event_type, ev.target, ev.security_verdict
        ),
        "agenttrace": {
            "session_id": ev.session_id,
            "event_type": ev.event_type,
            "target": ev.target,
            "security_verdict": ev.security_verdict,
            "security_reasons": ev.security_reasons,
            "duration_ms": ev.duration_ms,
            "prev_hash": ev.prev_hash,
            "event_hash": ev.event_hash,
            "signature": ev.signature,
            "public_key": ev.public_key,
            "action_details": ev.action_details
        }
    })
}

/// Formats a list of `TraceEvent`s into an Elasticsearch `/_bulk` API NDJSON payload.
/// Every document is preceded by an `{"index":{"_index":"...","_id":"..."}}` action metadata line.
pub fn to_elasticsearch_bulk_ndjson(events: &[TraceEvent], index_name: &str) -> String {
    let mut out = String::with_capacity(events.len() * 512);
    for ev in events {
        let meta = serde_json::json!({
            "index": {
                "_index": index_name,
                "_id": ev.event_id
            }
        });
        let doc = to_ecs_document(ev);
        if let (Ok(meta_line), Ok(doc_line)) = (serde_json::to_string(&meta), serde_json::to_string(&doc)) {
            out.push_str(&meta_line);
            out.push('\n');
            out.push_str(&doc_line);
            out.push('\n');
        }
    }
    out
}

/// Formats events into standard 1-line-per-event ECS NDJSON (for Filebeat, FluentBit, Vector, or Splunk).
pub fn to_ecs_ndjson_stream(events: &[TraceEvent]) -> String {
    let mut out = String::with_capacity(events.len() * 400);
    for ev in events {
        let doc = to_ecs_document(ev);
        if let Ok(line) = serde_json::to_string(&doc) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// Appends a single ECS event to a local NDJSON spool file and/or ships asynchronously to Elasticsearch.
pub fn ship_event_to_siem_async(client: reqwest::Client, ev: TraceEvent) {
    let cfg = SiemExportConfig::from_env();
    if !cfg.enabled {
        return;
    }

    // 1. Local NDJSON spool for Filebeat / Vector if configured
    if let Some(ref path) = cfg.local_ndjson_path {
        let doc = to_ecs_document(&ev);
        if let Ok(line) = serde_json::to_string(&doc) {
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "{}", line);
            }
        }
    }

    // 2. Remote Elasticsearch / Logstash HTTP shipping
    if let Some(endpoint) = cfg.elk_endpoint {
        let index = cfg.index_name;
        let api_key = cfg.api_key;
        tokio::spawn(async move {
            let bulk_body = to_elasticsearch_bulk_ndjson(&[ev], &index);
            let url = format!("{}/_bulk", endpoint.trim_end_matches('/'));
            let mut req = client
                .post(&url)
                .header("content-type", "application/x-ndjson")
                .body(bulk_body);
            if let Some(key) = api_key {
                req = req.header("authorization", format!("ApiKey {}", key));
            }
            let _ = req.send().await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ecs_document_and_elasticsearch_bulk_format() {
        let ev = TraceEvent {
            event_id: "ev-elk-001".to_string(),
            node_id: "claude-code:terminal@CORP-WS01#pid412".to_string(),
            session_id: "sess-corp-99".to_string(),
            timestamp: "2026-10-24T00:34:16.120Z".to_string(),
            event_type: EventType::CommandRun,
            target: "rm -rf /".to_string(),
            action_details: serde_json::json!({"command": "rm -rf /"}),
            diff: None,
            provenance_sources: vec![],
            security_verdict: SecurityVerdict::Blocked,
            security_reasons: vec!["[GUARD-CMD-001] Destructive root deletion blocked".to_string()],
            duration_ms: 0.42,
            prev_hash: "0000".to_string(),
            event_hash: "abcd1234".to_string(),
            signature: Some("sig123".to_string()),
            public_key: Some("pub123".to_string()),
        };

        let ecs = to_ecs_document(&ev);
        assert_eq!(ecs["@timestamp"], "2026-10-24T00:34:16.120Z");
        assert_eq!(ecs["ecs"]["version"], "8.11.0");
        assert_eq!(ecs["event"]["kind"], "alert");
        assert_eq!(ecs["event"]["outcome"], "failure");
        assert_eq!(ecs["host"]["name"], "CORP-WS01");

        let bulk = to_elasticsearch_bulk_ndjson(&[ev], "agenttrace-corp-audit");
        let lines: Vec<&str> = bulk.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"_index\":\"agenttrace-corp-audit\""));
        assert!(lines[1].contains("\"ev-elk-001\""));
    }
}
