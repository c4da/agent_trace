//! Persistent storage for Projects and Conversation Contexts.

use super::scanner::discover_projects;
use super::types::{
    AppendMessagePayload, ConversationMessage, ConversationRecord, CreateConversationPayload,
    ProjectRecord, RegisterProjectPayload, UpdateConversationPayload, UpdateProjectPayload,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{Arc, OnceLock},
};
use tokio::sync::RwLock;

static GLOBAL_PROJECT_STORE: OnceLock<Arc<RwLock<ProjectStore>>> = OnceLock::new();
const STORAGE_FILE: &str = "cluster_conversations.json";

/// Returns the thread-safe global `ProjectStore` instance.
pub fn global_project_store() -> Arc<RwLock<ProjectStore>> {
    GLOBAL_PROJECT_STORE
        .get_or_init(|| {
            let mut store = ProjectStore::new();
            store.load_from_disk();
            Arc::new(RwLock::new(store))
        })
        .clone()
}

pub struct ProjectStore {
    projects: HashMap<String, ProjectRecord>,
    conversations: HashMap<String, ConversationRecord>,
    active_project_id: String,
}

impl ProjectStore {
    pub fn new() -> Self {
        let discovered = discover_projects();
        let mut map = HashMap::new();
        let mut active_id = "agent_trace_gw".to_string();

        for p in discovered {
            if p.is_active {
                active_id = p.id.clone();
            }
            map.insert(p.id.clone(), p);
        }

        Self {
            projects: map,
            conversations: HashMap::new(),
            active_project_id: active_id,
        }
    }

    /// Lists all projects with updated conversation counts.
    pub fn list_projects(&self) -> Vec<ProjectRecord> {
        let mut list: Vec<ProjectRecord> = self
            .projects
            .values()
            .map(|p| {
                let count = self
                    .conversations
                    .values()
                    .filter(|c| c.project_id == p.id)
                    .count();
                let mut updated = p.clone();
                updated.conversation_count = count;
                updated.is_active = p.id == self.active_project_id;
                updated
            })
            .collect();
        list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        list
    }

    pub fn set_active_project(&mut self, project_id: &str) -> bool {
        if self.projects.contains_key(project_id) {
            self.active_project_id = project_id.to_string();
            self.save_to_disk();
            true
        } else {
            false
        }
    }

    pub fn get_active_project_id(&self) -> String {
        self.active_project_id.clone()
    }

    pub fn get_project(&self, id: &str) -> Option<ProjectRecord> {
        self.projects.get(id).cloned()
    }

    pub fn get_project_by_id_or_name(&self, query: &str) -> Option<ProjectRecord> {
        let q = query.to_lowercase().replace('_', "-").replace(' ', "-");
        if let Some(p) = self.projects.get(&q) {
            return Some(p.clone());
        }
        self.projects.values().find(|p| {
            p.id.eq_ignore_ascii_case(query)
                || p.name.eq_ignore_ascii_case(query)
                || p.id == q
        }).cloned()
    }

    /// Registers or updates a project workspace with a root folder path.
    pub fn upsert_project(&mut self, payload: RegisterProjectPayload) -> ProjectRecord {
        let clean_id = payload.name.to_lowercase().replace(' ', "-").replace('_', "-");
        let id = if clean_id.is_empty() {
            format!("proj-{}", &uuid::Uuid::new_v4().to_string()[..6])
        } else {
            clean_id
        };

        let tech_stack = payload.tech_stack.unwrap_or_else(|| {
            crate::projects::scanner::infer_tech_stack(Path::new(&payload.path))
        });
        let git_branch = crate::projects::scanner::read_git_branch(Path::new(&payload.path));

        let record = ProjectRecord {
            id: id.clone(),
            name: payload.name,
            path: payload.path,
            is_active: true,
            tech_stack,
            conversation_count: 0,
            git_branch,
        };

        self.projects.insert(id.clone(), record.clone());
        self.active_project_id = id;
        self.save_to_disk();
        record
    }

    /// Updates an existing project's name or root folder path.
    pub fn update_project(&mut self, id: &str, payload: UpdateProjectPayload) -> Option<ProjectRecord> {
        let p = self.projects.get_mut(id)?;
        if let Some(n) = payload.name {
            p.name = n;
        }
        if let Some(pt) = payload.path {
            p.path = pt;
            p.tech_stack = crate::projects::scanner::infer_tech_stack(Path::new(&p.path));
            p.git_branch = crate::projects::scanner::read_git_branch(Path::new(&p.path));
        }

        let updated = p.clone();
        self.save_to_disk();
        Some(updated)
    }

    /// Lists all conversations belonging to a project, newest first.
    pub fn list_conversations(&self, project_id: &str) -> Vec<ConversationRecord> {
        let mut list: Vec<ConversationRecord> = self
            .conversations
            .values()
            .filter(|c| c.project_id == project_id)
            .cloned()
            .collect();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        list
    }

    /// Creates a new conversation under a project.
    pub fn create_conversation(
        &mut self,
        payload: CreateConversationPayload,
    ) -> ConversationRecord {
        let now = Utc::now().to_rfc3339();
        let conv_id = format!("conv-{}", uuid::Uuid::new_v4().to_string()[..8].to_string());
        let title = payload
            .title
            .unwrap_or_else(|| "New Investigation Session".to_string());
        let active_wg = payload
            .active_workgroup
            .unwrap_or_else(|| "auto".to_string());
        let summary = payload.context_summary.unwrap_or_default();

        let conv = ConversationRecord {
            id: conv_id.clone(),
            project_id: payload.project_id,
            title,
            context_summary: summary,
            active_workgroup: active_wg,
            messages: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        };

        self.conversations.insert(conv_id, conv.clone());
        self.save_to_disk();
        conv
    }

    pub fn get_conversation(&self, id: &str) -> Option<ConversationRecord> {
        self.conversations.get(id).cloned()
    }

    pub fn update_conversation(
        &mut self,
        id: &str,
        payload: UpdateConversationPayload,
    ) -> Option<ConversationRecord> {
        let conv = self.conversations.get_mut(id)?;
        if let Some(t) = payload.title {
            conv.title = t;
        }
        if let Some(s) = payload.context_summary {
            conv.context_summary = s;
        }
        if let Some(wg) = payload.active_workgroup {
            conv.active_workgroup = wg;
        }
        conv.updated_at = Utc::now().to_rfc3339();
        let updated = conv.clone();
        self.save_to_disk();
        Some(updated)
    }

    pub fn append_message(
        &mut self,
        id: &str,
        payload: AppendMessagePayload,
    ) -> Option<ConversationRecord> {
        let conv = self.conversations.get_mut(id)?;
        let msg_id = format!("msg-{}", uuid::Uuid::new_v4().to_string()[..8].to_string());
        conv.messages.push(ConversationMessage {
            id: msg_id,
            role: payload.role,
            content: payload.content,
            timestamp: Utc::now().to_rfc3339(),
            stage_info: payload.stage_info,
            reasoning: payload.reasoning,
            node_id: payload.node_id,
            agent_id: payload.agent_id,
            duration_ms: payload.duration_ms,
        });
        conv.updated_at = Utc::now().to_rfc3339();
        let updated = conv.clone();
        self.save_to_disk();
        Some(updated)
    }

    pub fn delete_conversation(&mut self, id: &str) -> bool {
        let removed = self.conversations.remove(id).is_some();
        if removed {
            self.save_to_disk();
        }
        removed
    }

    pub fn save_to_disk(&self) {
        #[derive(Serialize)]
        struct DiskState<'a> {
            active_project_id: &'a str,
            conversations: &'a HashMap<String, ConversationRecord>,
        }
        let state = DiskState {
            active_project_id: &self.active_project_id,
            conversations: &self.conversations,
        };
        if let Ok(json) = serde_json::to_string_pretty(&state) {
            let _ = fs::write(STORAGE_FILE, json);
        }
    }

    pub fn load_from_disk(&mut self) {
        if !Path::new(STORAGE_FILE).exists() {
            return;
        }
        #[derive(Deserialize)]
        struct DiskState {
            active_project_id: Option<String>,
            conversations: Option<HashMap<String, ConversationRecord>>,
        }
        if let Ok(content) = fs::read_to_string(STORAGE_FILE) {
            if let Ok(state) = serde_json::from_str::<DiskState>(&content) {
                if let Some(active) = state.active_project_id {
                    if self.projects.contains_key(&active) {
                        self.active_project_id = active;
                    }
                }
                if let Some(convs) = state.conversations {
                    self.conversations = convs;
                }
            }
        }
    }
}
