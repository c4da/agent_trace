//! Central in-memory registry and lifecycle coordinator for cluster tasks.

use super::types::{TaskRecord, TaskStatus};
use chrono::Utc;
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};
use tokio::sync::RwLock;

static GLOBAL_TASK_MANAGER: OnceLock<Arc<RwLock<TaskManager>>> = OnceLock::new();

/// Returns the thread-safe global `TaskManager` instance.
pub fn global_tasks() -> Arc<RwLock<TaskManager>> {
    GLOBAL_TASK_MANAGER
        .get_or_init(|| Arc::new(RwLock::new(TaskManager::new())))
        .clone()
}

pub struct TaskManager {
    tasks: HashMap<String, TaskRecord>,
    order: Vec<String>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Registers a new running task.
    pub fn register(
        &mut self,
        id: String,
        name: String,
        agent_id: Option<String>,
        node_id: Option<String>,
    ) -> TaskRecord {
        let record = TaskRecord {
            id: id.clone(),
            name,
            status: TaskStatus::Running,
            started_at: Utc::now().to_rfc3339(),
            completed_at: None,
            duration_ms: None,
            log_tail: "Task initiated.".to_string(),
            agent_id,
            node_id,
            progress: 0,
        };

        if !self.tasks.contains_key(&id) {
            self.order.push(id.clone());
        }
        self.tasks.insert(id, record.clone());

        // Bound memory: keep most recent 200 tasks
        if self.order.len() > 200 {
            let removed_id = self.order.remove(0);
            self.tasks.remove(&removed_id);
        }

        record
    }

    /// Updates task progress and appends a log snippet.
    pub fn append_log(&mut self, id: &str, progress: u8, log_snippet: &str) -> Option<TaskRecord> {
        let task = self.tasks.get_mut(id)?;
        task.progress = progress.min(100);
        if !log_snippet.is_empty() {
            task.log_tail = format!("{}\n{}", task.log_tail, log_snippet);
            if task.log_tail.len() > 2000 {
                task.log_tail = task.log_tail.chars().rev().take(2000).collect::<String>().chars().rev().collect();
            }
        }
        Some(task.clone())
    }

    /// Completes a task with success or failure.
    pub fn finish(&mut self, id: &str, success: bool, log_snippet: &str) -> Option<TaskRecord> {
        let task = self.tasks.get_mut(id)?;
        task.status = if success {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };
        let now = Utc::now();
        if let Ok(st) = chrono::DateTime::parse_from_rfc3339(&task.started_at) {
            task.duration_ms = Some((now - st.with_timezone(&Utc)).num_milliseconds() as f64);
        }
        task.completed_at = Some(now.to_rfc3339());
        task.progress = 100;
        if !log_snippet.is_empty() {
            task.log_tail = format!("{}\n{}", task.log_tail, log_snippet);
        }
        Some(task.clone())
    }

    /// Marks a task as cancelled.
    pub fn cancel(&mut self, id: &str) -> Option<TaskRecord> {
        let task = self.tasks.get_mut(id)?;
        if task.status == TaskStatus::Running {
            task.status = TaskStatus::Cancelled;
            task.completed_at = Some(Utc::now().to_rfc3339());
            task.log_tail = format!("{}\n[SYSTEM] Task cancelled by operator.", task.log_tail);
        }
        Some(task.clone())
    }

    /// Lists all tasks sorted by start time descending (newest first).
    pub fn list(&self) -> Vec<TaskRecord> {
        let mut list: Vec<TaskRecord> = self.tasks.values().cloned().collect();
        list.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        list
    }

    /// Gets a single task by ID.
    pub fn get(&self, id: &str) -> Option<TaskRecord> {
        self.tasks.get(id).cloned()
    }
}
