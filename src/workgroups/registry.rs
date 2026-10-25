//! Thread-safe workgroup registry and disk persistence store.

use super::presets::default_workgroups;
use super::types::{MarkdownRuleAttachment, ProblemWorkgroup};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
};

/// Registry storing all active Problem Workgroups in-memory and persisting them to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkgroupRegistry {
    pub groups: HashMap<String, ProblemWorkgroup>,
    #[serde(skip)]
    pub persistence_path: Option<PathBuf>,
}

impl Default for WorkgroupRegistry {
    fn default() -> Self {
        Self::new_with_defaults(Some(PathBuf::from(".agent_trace/workgroups.json")))
    }
}

impl WorkgroupRegistry {
    pub fn new(persistence_path: Option<PathBuf>) -> Self {
        Self {
            groups: HashMap::new(),
            persistence_path,
        }
    }

    /// Initializes a registry populated with the default Problem Workgroups,
    /// overlaying any persisted user modifications from disk when `persistence_path` is enabled.
    pub fn new_with_defaults(persistence_path: Option<PathBuf>) -> Self {
        let mut groups = default_workgroups();

        if persistence_path.is_some() {
            if let Some(loaded) = crate::persistence::global_persistence()
                .load_struct::<WorkgroupRegistry>("workgroups", "registry")
            {
                for (k, v) in loaded.groups {
                    groups.insert(k, v);
                }
            }
        }

        if let Some(ref path) = persistence_path {
            if let Ok(raw) = fs::read_to_string(path) {
                if let Ok(loaded) = serde_json::from_str::<WorkgroupRegistry>(&raw) {
                    for (k, v) in loaded.groups {
                        groups.insert(k, v);
                    }
                }
            }
        }

        let reg = Self {
            groups,
            persistence_path,
        };
        if reg.persistence_path.is_some() {
            reg.save_to_disk();
        }
        reg
    }

    pub fn save_to_disk(&self) {
        if self.persistence_path.is_some() {
            let _ = crate::persistence::global_persistence().save_struct("workgroups", "registry", self);
        }
        if let Some(ref path) = self.persistence_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = fs::write(path, json);
            }
        }
    }

    pub fn list_sorted(&self) -> Vec<ProblemWorkgroup> {
        let mut list: Vec<ProblemWorkgroup> = self.groups.values().cloned().collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
    }

    pub fn get(&self, id: &str) -> Option<ProblemWorkgroup> {
        self.groups.get(id).cloned()
    }

    pub fn upsert(&mut self, mut wg: ProblemWorkgroup) -> ProblemWorkgroup {
        if wg.id.trim().is_empty() {
            let slug: String = wg
                .name
                .to_lowercase()
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect();
            wg.id = format!("wg-{}", slug.trim_matches('-'));
        }
        if wg.icon.trim().is_empty() {
            wg.icon = "⚡".to_string();
        }
        if wg.color.trim().is_empty() {
            wg.color = "#58a6ff".to_string();
        }
        if wg.execution_strategy.trim().is_empty() {
            wg.execution_strategy = "pipeline".to_string();
        }
        self.groups.insert(wg.id.clone(), wg.clone());
        self.save_to_disk();
        wg
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let removed = self.groups.remove(id).is_some();
        if removed {
            self.save_to_disk();
        }
        removed
    }

    pub fn attach_rule(&mut self, workgroup_id: &str, rule: MarkdownRuleAttachment) -> Option<ProblemWorkgroup> {
        let wg = self.groups.get_mut(workgroup_id)?;
        if let Some(existing) = wg.markdown_rules.iter_mut().find(|r| r.id == rule.id) {
            *existing = rule;
        } else {
            wg.markdown_rules.push(rule);
        }
        let updated = wg.clone();
        self.save_to_disk();
        Some(updated)
    }

    pub fn increment_tasks(&mut self, workgroup_id: &str) {
        if let Some(wg) = self.groups.get_mut(workgroup_id) {
            wg.tasks_completed += 1;
            self.save_to_disk();
        }
    }

    /// Automatically matches a prompt to a Problem Workgroup by checking `trigger_keywords`.
    pub fn match_workgroup_by_prompt(&self, prompt: &str) -> Option<ProblemWorkgroup> {
        let lower = prompt.to_lowercase();
        let mut best: Option<(usize, &ProblemWorkgroup)> = None;
        for wg in self.groups.values() {
            let hits = wg
                .trigger_keywords
                .iter()
                .filter(|kw| !kw.is_empty() && lower.contains(&kw.to_lowercase()))
                .count();
            if hits > 0 {
                if best.map_or(true, |(best_hits, _)| hits > best_hits) {
                    best = Some((hits, wg));
                }
            }
        }
        best.map(|(_, wg)| wg.clone())
    }
}
