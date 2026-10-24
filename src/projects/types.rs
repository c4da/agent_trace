//! Data types for Projects, Workspaces, and Persistent Conversation Contexts.

use serde::{Deserialize, Serialize};

/// A registered or discovered project workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    pub is_active: bool,
    pub tech_stack: String,
    pub conversation_count: usize,
    pub git_branch: Option<String>,
}

/// A message exchanged within a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub id: String,
    pub role: String, // "user", "assistant", "system"
    pub content: String,
    pub timestamp: String,
    pub stage_info: Option<String>,
    pub reasoning: Option<String>,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<f64>,
}

/// A persistent conversation session belonging to a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationRecord {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub context_summary: String,
    pub active_workgroup: String,
    pub messages: Vec<ConversationMessage>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateConversationPayload {
    pub project_id: String,
    pub title: Option<String>,
    pub active_workgroup: Option<String>,
    pub context_summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateConversationPayload {
    pub title: Option<String>,
    pub context_summary: Option<String>,
    pub active_workgroup: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppendMessagePayload {
    pub role: String,
    pub content: String,
    pub stage_info: Option<String>,
    pub reasoning: Option<String>,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SetActiveProjectPayload {
    pub project_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterProjectPayload {
    pub name: String,
    pub path: String,
    pub tech_stack: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateProjectPayload {
    pub name: Option<String>,
    pub path: Option<String>,
}

