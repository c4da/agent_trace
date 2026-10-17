use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Defines the type of action recorded in the telemetry ledger.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventType {
    FileRead,
    FileWrite,
    CommandRun,
    WebRequest,
    DecisionStep,
    LlmInference,
}

/// Defines the security assessment of a recorded event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecurityVerdict {
    Allowed,
    Flagged,
    Blocked,
}

/// Represents a single, cryptographically linked event in the AI telemetry ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    // Metadata
    pub event_id: String,
    pub node_id: String,
    pub session_id: String,
    pub timestamp: String,
    pub event_type: EventType,
    pub target: String,
    pub action_details: Value,
    pub diff: Option<String>,
    pub provenance_sources: Vec<String>,
    pub security_verdict: SecurityVerdict,
    pub security_reasons: Vec<String>,
    pub duration_ms: f64,

    // Cryptographic Ledger fields (Blockchain/Hash-Chain)
    /// SHA-256 hash of the preceding event. Use 64 zeros for the genesis block.
    pub prev_hash: String,
    /// SHA-256 hash of the current event's canonical fields.
    pub event_hash: String,
    /// Ed25519 signature of the event data.
    pub signature: Option<String>,
    /// Ed25519 public key of the node that generated the event.
    pub public_key: Option<String>,
}

/// Represents the status and identity of a node participating in the cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHeartbeat {
    pub node_id: String,
    pub hostname: String,
    pub ip_address: String,
    pub role: String,
    pub status: String,
    pub last_seen: String,
    /// Public key used for authentication and verification.
    pub public_key: Option<String>,
    /// Optional self-announced model manifests deployed on this node.
    pub models: Option<Vec<crate::registry::ModelManifest>>,
}

/// Provides a summary of the current state of the entire telemetry cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterSummary {
    pub total_nodes: usize,
    pub active_nodes: Vec<NodeHeartbeat>,
    pub total_events: u64,
    pub total_reads: u64,
    pub total_writes: u64,
    pub total_commands: u64,
    pub security_alerts: u64,
    /// A composite score indicating the potential blast radius of a compromise.
    pub blast_radius_score: f64,
    /// The hash of the latest verified event in the ledger.
    pub latest_ledger_hash: String,
    /// Flag indicating if the entire ledger chain has been cryptographically verified.
    pub ledger_verified: bool,
}