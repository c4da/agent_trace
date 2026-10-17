//! Axum REST HTTP handlers for Problem Workgroup management.

use super::presets::available_agents_catalog;
use super::registry::WorkgroupRegistry;
use super::scanner::discover_workspace_markdown_files;
use super::types::{MarkdownRuleAttachment, ProblemWorkgroup, WorkgroupMember};
use crate::zones::resolve_canonical_target;
use axum::{
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use std::{
    fs,
    sync::{Arc, OnceLock},
};
use tokio::sync::RwLock;

static GLOBAL_WORKGROUPS: OnceLock<Arc<RwLock<WorkgroupRegistry>>> = OnceLock::new();

/// Returns the shared thread-safe `WorkgroupRegistry` singleton.
pub fn global_workgroups() -> Arc<RwLock<WorkgroupRegistry>> {
    GLOBAL_WORKGROUPS
        .get_or_init(|| Arc::new(RwLock::new(WorkgroupRegistry::default())))
        .clone()
}

/// `GET /api/v1/workgroups` — Lists all Problem Workgroups, available cluster agents,
/// and discovered `.md` files in the workspace.
pub async fn list_workgroups_handler() -> Result<Json<serde_json::Value>, StatusCode> {
    let reg = global_workgroups();
    let guard = reg.read().await;
    let workgroups = guard.list_sorted();
    drop(guard);

    Ok(Json(serde_json::json!({
        "workgroups": workgroups,
        "available_agents": available_agents_catalog(),
        "discovered_markdown_files": discover_workspace_markdown_files(),
    })))
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertWorkgroupPayload {
    pub id: Option<String>,
    pub name: String,
    pub problem_domain: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub execution_strategy: Option<String>,
    pub members: Vec<WorkgroupMember>,
    pub markdown_rules: Option<Vec<MarkdownRuleAttachment>>,
    pub trigger_keywords: Option<Vec<String>>,
}

/// `POST /api/v1/workgroups` — Creates or updates a Problem Workgroup.
pub async fn upsert_workgroup_handler(
    State(state): State<crate::handlers::AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpsertWorkgroupPayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let reg = global_workgroups();
    let mut guard = reg.write().await;

    let existing_tasks = payload
        .id
        .as_deref()
        .and_then(|id| guard.get(id))
        .map(|w| w.tasks_completed)
        .unwrap_or(0);

    let existing_rules = payload
        .id
        .as_deref()
        .and_then(|id| guard.get(id))
        .map(|w| w.markdown_rules)
        .unwrap_or_default();

    let wg = ProblemWorkgroup {
        id: payload.id.unwrap_or_default(),
        name: payload.name,
        problem_domain: payload.problem_domain,
        icon: payload.icon.unwrap_or_else(|| "⚡".to_string()),
        color: payload.color.unwrap_or_else(|| "#58a6ff".to_string()),
        description: payload.description.unwrap_or_default(),
        execution_strategy: payload.execution_strategy.unwrap_or_else(|| "pipeline".to_string()),
        members: payload.members,
        markdown_rules: payload.markdown_rules.unwrap_or(existing_rules),
        trigger_keywords: payload.trigger_keywords.unwrap_or_default(),
        tasks_completed: existing_tasks,
    };

    let saved = guard.upsert(wg);
    drop(guard);

    Ok(Json(serde_json::json!({
        "status": "saved",
        "workgroup": saved,
        "compiled_system_prompt": saved.compile_system_prompt()
    })))
}

/// `DELETE /api/v1/workgroups/:workgroup_id` — Deletes a Problem Workgroup.
pub async fn delete_workgroup_handler(
    State(state): State<crate::handlers::AppState>,
    headers: HeaderMap,
    AxumPath(workgroup_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let reg = global_workgroups();
    let mut guard = reg.write().await;
    let deleted = guard.delete(&workgroup_id);
    Ok(Json(serde_json::json!({
        "status": if deleted { "deleted" } else { "not_found" },
        "id": workgroup_id
    })))
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttachRulePayload {
    pub id: Option<String>,
    pub title: String,
    pub file_path: Option<String>,
    pub inline_markdown: Option<String>,
    pub enforce_strict: Option<bool>,
}

/// `POST /api/v1/workgroups/:workgroup_id/rules` — Attaches a Markdown (`.md`) rule file or inline rule.
pub async fn attach_markdown_rule_handler(
    State(state): State<crate::handlers::AppState>,
    headers: HeaderMap,
    AxumPath(workgroup_id): AxumPath<String>,
    Json(payload): Json<AttachRulePayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let rule_id = payload.id.unwrap_or_else(|| {
        let slug: String = payload
            .title
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        format!("rule-{}", slug.trim_matches('-'))
    });

    let rule = MarkdownRuleAttachment {
        id: rule_id,
        title: payload.title,
        file_path: payload.file_path.filter(|s| !s.trim().is_empty()),
        inline_markdown: payload.inline_markdown.filter(|s| !s.trim().is_empty()),
        enforce_strict: payload.enforce_strict.unwrap_or(true),
    };

    let reg = global_workgroups();
    let mut guard = reg.write().await;
    let updated = guard
        .attach_rule(&workgroup_id, rule)
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::json!({
        "status": "rule_attached",
        "workgroup": updated,
        "compiled_system_prompt": updated.compile_system_prompt()
    })))
}

/// `DELETE /api/v1/workgroups/:workgroup_id/rules/:rule_id` — Removes an attached Markdown rule.
pub async fn remove_markdown_rule_handler(
    State(state): State<crate::handlers::AppState>,
    headers: HeaderMap,
    AxumPath((workgroup_id, rule_id)): AxumPath<(String, String)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.security.is_authorized(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let reg = global_workgroups();
    let mut guard = reg.write().await;
    let wg = guard.groups.get_mut(&workgroup_id).ok_or(StatusCode::NOT_FOUND)?;
    wg.markdown_rules.retain(|r| r.id != rule_id);
    let updated = wg.clone();
    guard.save_to_disk();

    Ok(Json(serde_json::json!({
        "status": "rule_removed",
        "workgroup": updated,
        "compiled_system_prompt": updated.compile_system_prompt()
    })))
}

/// `GET /api/v1/workgroups/:workgroup_id/preview` — Previews the compiled `.md` rules and agent roster.
pub async fn preview_workgroup_prompt_handler(
    AxumPath(workgroup_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let reg = global_workgroups();
    let guard = reg.read().await;
    let wg = guard.get(&workgroup_id).ok_or(StatusCode::NOT_FOUND)?;
    let resolved_rules: Vec<serde_json::Value> = wg
        .markdown_rules
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "title": r.title,
                "file_path": r.file_path,
                "enforce_strict": r.enforce_strict,
                "resolved_preview": r.resolve_content(1200),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "workgroup_id": wg.id,
        "name": wg.name,
        "problem_domain": wg.problem_domain,
        "execution_strategy": wg.execution_strategy,
        "members": wg.members,
        "rules_count": wg.markdown_rules.len(),
        "resolved_rules": resolved_rules,
        "yaml": wg.to_yaml(),
        "compiled_system_prompt": wg.compile_system_prompt(),
    })))
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplyWorkgroupYamlPayload {
    pub yaml: Option<String>,
    pub file_path: Option<String>,
}

/// `GET /api/v1/workgroups/:workgroup_id/yaml` — Exports the workgroup as declarative YAML.
pub async fn get_workgroup_yaml_handler(
    AxumPath(workgroup_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let reg = global_workgroups();
    let guard = reg.read().await;
    let wg = guard.get(&workgroup_id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::json!({
        "workgroup_id": wg.id,
        "yaml": wg.to_yaml(),
    })))
}

/// `POST /api/v1/workgroups/yaml` — Applies a declarative YAML workgroup config from inline text or a file path.
pub async fn apply_workgroup_yaml_handler(
    State(state): State<crate::handlers::AppState>,
    headers: HeaderMap,
    Json(payload): Json<ApplyWorkgroupYamlPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if !state.security.is_authorized(&headers) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Unauthorized"})),
        ));
    }

    let raw_yaml = if let Some(ref path_str) = payload.file_path {
        let trimmed = path_str.trim();
        if !trimmed.is_empty() {
            let norm = resolve_canonical_target(trimmed);
            let lower = norm.to_lowercase();
            if lower.contains(".secrets") || lower.contains(".ssh") || lower.ends_with(".env") {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({"error": "Access to vault paths is forbidden"})),
                ));
            }
            fs::read_to_string(trimmed).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": format!("Failed to read YAML file '{}': {}", trimmed, e)})),
                )
            })?
        } else {
            payload.yaml.clone().unwrap_or_default()
        }
    } else {
        payload.yaml.clone().unwrap_or_default()
    };

    let parsed = ProblemWorkgroup::from_yaml(&raw_yaml).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
    })?;

    let reg = global_workgroups();
    let mut guard = reg.write().await;
    let saved = guard.upsert(parsed);
    drop(guard);

    Ok(Json(serde_json::json!({
        "status": "applied_yaml",
        "workgroup": saved,
        "yaml": saved.to_yaml(),
        "compiled_system_prompt": saved.compile_system_prompt(),
    })))
}
