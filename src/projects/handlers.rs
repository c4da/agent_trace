//! Axum REST HTTP handlers for Projects, Workspaces, and Conversations.

use super::storage::global_project_store;
use super::types::{
    AppendMessagePayload, CreateConversationPayload, RegisterProjectPayload,
    SetActiveProjectPayload, UpdateConversationPayload, UpdateProjectPayload,
};
use axum::{
    extract::Path as AxumPath,
    http::StatusCode,
    Json,
};

/// `GET /api/v1/projects` — Lists all available projects and active status.
pub async fn list_projects_handler() -> Result<Json<serde_json::Value>, StatusCode> {
    let store = global_project_store();
    let guard = store.read().await;
    let projects = guard.list_projects();
    let active_id = guard.get_active_project_id();
    drop(guard);

    Ok(Json(serde_json::json!({
        "active_project_id": active_id,
        "total_projects": projects.len(),
        "projects": projects
    })))
}

/// `POST /api/v1/projects` — Registers or adds a new workspace project with custom root folder.
pub async fn register_project_handler(
    Json(payload): Json<RegisterProjectPayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = global_project_store();
    let mut guard = store.write().await;
    let project = guard.upsert_project(payload);
    drop(guard);

    Ok(Json(serde_json::to_value(project).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

/// `PUT /api/v1/projects/:project_id` — Updates project workspace name or root folder path.
pub async fn update_project_handler(
    AxumPath(project_id): AxumPath<String>,
    Json(payload): Json<UpdateProjectPayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = global_project_store();
    let mut guard = store.write().await;
    let updated = guard
        .update_project(&project_id, payload)
        .ok_or(StatusCode::NOT_FOUND)?;
    drop(guard);

    Ok(Json(serde_json::to_value(updated).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}


/// `POST /api/v1/projects/active` — Sets the active workspace project.
pub async fn set_active_project_handler(
    Json(payload): Json<SetActiveProjectPayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = global_project_store();
    let mut guard = store.write().await;
    let success = guard.set_active_project(&payload.project_id);
    drop(guard);

    if success {
        Ok(Json(serde_json::json!({
            "status": "active_project_updated",
            "active_project_id": payload.project_id
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// `GET /api/v1/projects/:project_id/conversations` — Lists conversations for a project.
pub async fn list_project_conversations_handler(
    AxumPath(project_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = global_project_store();
    let guard = store.read().await;
    let convs = guard.list_conversations(&project_id);
    drop(guard);

    Ok(Json(serde_json::json!({
        "project_id": project_id,
        "total_conversations": convs.len(),
        "conversations": convs
    })))
}

/// `POST /api/v1/conversations` — Creates a new persistent conversation session.
pub async fn create_conversation_handler(
    Json(payload): Json<CreateConversationPayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = global_project_store();
    let mut guard = store.write().await;
    let conv = guard.create_conversation(payload);
    drop(guard);

    Ok(Json(serde_json::to_value(conv).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

/// `GET /api/v1/conversations/:id` — Gets full details, messages, and context summary of a conversation.
pub async fn get_conversation_handler(
    AxumPath(conv_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = global_project_store();
    let guard = store.read().await;
    let conv = guard.get_conversation(&conv_id).ok_or(StatusCode::NOT_FOUND)?;
    drop(guard);

    Ok(Json(serde_json::to_value(conv).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

/// `PUT /api/v1/conversations/:id` — Updates conversation title, workgroup, or context summary.
pub async fn update_conversation_handler(
    AxumPath(conv_id): AxumPath<String>,
    Json(payload): Json<UpdateConversationPayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = global_project_store();
    let mut guard = store.write().await;
    let updated = guard
        .update_conversation(&conv_id, payload)
        .ok_or(StatusCode::NOT_FOUND)?;
    drop(guard);

    Ok(Json(serde_json::to_value(updated).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

/// `POST /api/v1/conversations/:id/messages` — Appends a message to a conversation.
pub async fn append_message_handler(
    AxumPath(conv_id): AxumPath<String>,
    Json(payload): Json<AppendMessagePayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = global_project_store();
    let mut guard = store.write().await;
    let updated = guard
        .append_message(&conv_id, payload)
        .ok_or(StatusCode::NOT_FOUND)?;
    drop(guard);

    Ok(Json(serde_json::to_value(updated).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

/// `DELETE /api/v1/conversations/:id` — Deletes a conversation session.
pub async fn delete_conversation_handler(
    AxumPath(conv_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let store = global_project_store();
    let mut guard = store.write().await;
    let deleted = guard.delete_conversation(&conv_id);
    drop(guard);

    if deleted {
        Ok(Json(serde_json::json!({ "status": "deleted", "id": conv_id })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
