//! Deep Telemetry Analytics & Behavioral Anomaly Detection Engine.
//!
//! Provides multi-dimensional telemetry analysis over the cryptographic Ed25519 trace ledger:
//! - Target Hotspots (frequently accessed / modified targets)
//! - Per-Agent Behavioral Profiling & Threat Scoring
//! - Time-Series Activity Aggregation (read/write/command/LLM rate)
//! - Security Anomaly Classification & Audit Breakdown

use crate::schema::{EventType, SecurityVerdict};
use serde::{Deserialize, Serialize};

/// Represents an accessed or modified file/target hotspot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetHotspot {
    pub target: String,
    pub event_type: EventType,
    pub count: u64,
    pub risk_flagged: bool,
}

/// Behavioral profile and threat assessment for a specific cluster agent/node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub agent_id: String,
    pub total_events: u64,
    pub reads: u64,
    pub writes: u64,
    pub commands: u64,
    pub llm_inferences: u64,
    pub violations: u64,
    pub read_to_write_ratio: f64,
    pub blast_radius_score: f64,
    pub threat_level: String,
}

/// Aggregated activity counts within a temporal time window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesBucket {
    pub bucket: String,
    pub reads: u64,
    pub writes: u64,
    pub commands: u64,
    pub llm_calls: u64,
    pub alerts: u64,
}

/// Anomaly pattern detected in security violations or suspicious target accesses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAnomaly {
    pub verdict: SecurityVerdict,
    pub reason: String,
    pub count: u64,
    pub latest_timestamp: String,
    pub sample_target: String,
}

/// Comprehensive analytics report produced for operators and external AI agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryAnalyticsReport {
    pub generated_at: String,
    pub total_events: u64,
    pub top_read_targets: Vec<TargetHotspot>,
    pub top_write_targets: Vec<TargetHotspot>,
    pub agent_profiles: Vec<AgentProfile>,
    pub time_series: Vec<TimeSeriesBucket>,
    pub security_anomalies: Vec<SecurityAnomaly>,
    pub ledger_valid: bool,
    pub ledger_head: String,
}

impl AgentProfile {
    /// Computes threat level and blast radius for an agent based on action weights.
    pub fn compute_threat(_reads: u64, writes: u64, commands: u64, violations: u64) -> (f64, String) {
        let score = ((writes * 10 + commands * 5 + violations * 25).min(100)) as f64;
        let level = if violations > 0 || score >= 75.0 {
            "CRITICAL".to_string()
        } else if score >= 50.0 {
            "HIGH".to_string()
        } else if score >= 20.0 {
            "MEDIUM".to_string()
        } else {
            "LOW".to_string()
        };
        (score, level)
    }

    pub fn compute_ratio(reads: u64, writes: u64) -> f64 {
        if writes == 0 {
            reads as f64
        } else {
            ((reads as f64) / (writes as f64) * 100.0).round() / 100.0
        }
    }
}
