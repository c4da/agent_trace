//! Axum REST HTTP handlers for the Tasks subsystem.

use super::manager::global_tasks;
use axum::{
    extract::Path as AxumPath,
    http::StatusCode,
    Json,
};

/// `GET /api/v1/tasks` — Lists all background tasks with status, progress, and logs.
pub async fn list_tasks_handler() -> Result<Json<serde_json::Value>, StatusCode> {
    let mgr = global_tasks();
    let guard = mgr.read().await;
    let tasks = guard.list();
    drop(guard);

    Ok(Json(serde_json::json!({
        "total_count": tasks.len(),
        "tasks": tasks
    })))
}

/// `GET /api/v1/tasks/:id` — Retrieves a single task record by ID.
pub async fn get_task_handler(
    AxumPath(task_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mgr = global_tasks();
    let guard = mgr.read().await;
    let task = guard.get(&task_id).ok_or(StatusCode::NOT_FOUND)?;
    drop(guard);

    Ok(Json(serde_json::to_value(task).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

/// `POST /api/v1/tasks/:id/cancel` — Cancels a running background task.
pub async fn cancel_task_handler(
    AxumPath(task_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mgr = global_tasks();
    let mut guard = mgr.write().await;
    let task = guard.cancel(&task_id).ok_or(StatusCode::NOT_FOUND)?;
    drop(guard);

    Ok(Json(serde_json::json!({
        "status": "cancelled",
        "task": task
    })))
}
