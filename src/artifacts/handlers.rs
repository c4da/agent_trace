//! Axum REST HTTP handlers for the Artifacts subsystem.

use super::scanner::scan_artifacts;
use super::types::{ArtifactDetail, ArtifactKind};
use crate::zones::resolve_canonical_target;
use axum::{
    body::Body,
    extract::{Path as AxumPath, Query},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde::Deserialize;
use std::fs;

/// `GET /api/v1/artifacts` — Returns all discovered artifacts across workspace and Antigravity brain.
pub async fn list_artifacts_handler() -> Result<Json<serde_json::Value>, StatusCode> {
    let items = scan_artifacts();
    Ok(Json(serde_json::json!({
        "total_count": items.len(),
        "artifacts": items
    })))
}

#[derive(Debug, Deserialize)]
pub struct ArtifactQuery {
    pub raw: Option<bool>,
}

/// `GET /api/v1/artifacts/:id` — Fetches artifact details or raw content by artifact ID.
pub async fn get_artifact_handler(
    AxumPath(artifact_id): AxumPath<String>,
    Query(query): Query<ArtifactQuery>,
) -> Result<Response, StatusCode> {
    let items = scan_artifacts();
    let item = items
        .into_iter()
        .find(|a| a.id == artifact_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let path_norm = resolve_canonical_target(&item.file_path);
    let lower = path_norm.to_lowercase();
    if lower.contains(".secrets") || lower.contains(".ssh") || lower.ends_with(".env") {
        return Err(StatusCode::FORBIDDEN);
    }

    let is_raw = query.raw.unwrap_or(false);

    if is_raw || item.kind == ArtifactKind::Image {
        let bytes = fs::read(&item.file_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let mime = item.mime_type.clone();
        return Response::builder()
            .header(header::CONTENT_TYPE, mime)
            .body(Body::from(bytes))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    }

    let content = fs::read_to_string(&item.file_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let detail = ArtifactDetail {
        item,
        is_binary: false,
        content,
    };

    let json_body = serde_json::to_string(&detail).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Response::builder()
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .body(Body::from(json_body))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
