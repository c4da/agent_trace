//! Data types for Background Task management and observability.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl ToString for TaskStatus {
    fn to_string(&self) -> String {
        match self {
            Self::Running => "running".to_string(),
            Self::Completed => "completed".to_string(),
            Self::Failed => "failed".to_string(),
            Self::Cancelled => "cancelled".to_string(),
        }
    }
}

/// A background task running or tracked in the cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub name: String,
    pub status: TaskStatus,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub duration_ms: Option<f64>,
    pub log_tail: String,
    pub agent_id: Option<String>,
    pub node_id: Option<String>,
    pub progress: u8,
}
