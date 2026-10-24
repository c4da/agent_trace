//! Safe file reader and writer for attached Markdown (.md) rule governance files.

use crate::zones::resolve_canonical_target;
use axum::{
    extract::Query,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Deserialize)]
pub struct RuleFileQuery {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleFileResponse {
    pub file_path: String,
    pub content: String,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SaveRuleFilePayload {
    pub file_path: String,
    pub content: String,
}

/// `GET /api/v1/rules/file?path=...` — Safely reads a Markdown rule file from the workspace.
pub async fn get_rule_file_handler(
    Query(query): Query<RuleFileQuery>,
) -> Result<Json<RuleFileResponse>, StatusCode> {
    let trimmed = query.path.trim();
    if trimmed.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let norm = resolve_canonical_target(trimmed);
    let lower = norm.to_lowercase();
    if lower.contains(".secrets") || lower.contains(".ssh") || lower.ends_with(".env") {
        return Err(StatusCode::FORBIDDEN);
    }

    let path = Path::new(trimmed);
    if !path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let content = fs::read_to_string(path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let meta = fs::metadata(path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let modified = meta.modified().ok().map(|t| {
        let dt: chrono::DateTime<chrono::Utc> = t.into();
        dt.to_rfc3339()
    });

    Ok(Json(RuleFileResponse {
        file_path: trimmed.to_string(),
        content,
        size_bytes: meta.len(),
        modified_at: modified,
    }))
}

/// `POST /api/v1/rules/file` — Safely saves or updates a Markdown rule file in the workspace.
pub async fn save_rule_file_handler(
    Json(payload): Json<SaveRuleFilePayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let trimmed = payload.file_path.trim();
    if trimmed.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Safety guard: only permit editing markdown or doc files to protect binary/code files
    let lower_path = trimmed.to_lowercase();
    if !lower_path.ends_with(".md") && !lower_path.ends_with(".markdown") && !lower_path.ends_with(".txt") {
        return Err(StatusCode::BAD_REQUEST);
    }

    let norm = resolve_canonical_target(trimmed);
    let lower = norm.to_lowercase();
    if lower.contains(".secrets") || lower.contains(".ssh") || lower.ends_with(".env") {
        return Err(StatusCode::FORBIDDEN);
    }

    let path = Path::new(trimmed);
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }

    fs::write(path, &payload.content).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let meta = fs::metadata(path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "status": "saved",
        "file_path": trimmed,
        "size_bytes": meta.len(),
        "message": format!("Successfully saved {} bytes to {}", meta.len(), trimmed)
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_and_save_rule_file_lifecycle() {
        let test_path = "docs/test_rule_edit.md";
        let payload = SaveRuleFilePayload {
            file_path: test_path.to_string(),
            content: "# Test Rule Header\n- Rule 1: Always verify\n".to_string(),
        };

        let res = save_rule_file_handler(Json(payload)).await;
        assert!(res.is_ok(), "Should successfully save rule file");

        let read_res = get_rule_file_handler(Query(RuleFileQuery {
            path: test_path.to_string(),
        }))
        .await;
        assert!(read_res.is_ok());
        let body = read_res.unwrap().0;
        assert_eq!(body.file_path, test_path);
        assert!(body.content.contains("# Test Rule Header"));

        // Clean up test file
        let _ = fs::remove_file(test_path);
    }

    #[tokio::test]
    async fn test_forbidden_vault_paths() {
        let payload = SaveRuleFilePayload {
            file_path: ".secrets/attack.md".to_string(),
            content: "malicious".to_string(),
        };
        let res = save_rule_file_handler(Json(payload)).await;
        assert_eq!(res.err(), Some(StatusCode::FORBIDDEN));
    }
}
