use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::api::{
    dto::{TaskDto, TaskProgressDto},
    error::ApiError,
    extractors::AuthContext,
    permissions::Permission,
};

use super::AppState;

/// List all active tasks in the system
///
/// # Permission Required
/// - `libraries:read` or admin status
#[utoipa::path(
    get,
    path = "/tasks",
    responses(
        (status = 200, description = "List of active tasks", body = Vec<TaskDto>),
        (status = 403, description = "Permission denied"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Tasks"
)]
pub async fn list_tasks(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<Vec<TaskDto>>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::LibrariesRead)?;

    // Get all active scans - these are our tasks
    let scans = state.scan_manager.list_active().await;

    // Convert scan statuses to task DTOs
    let tasks: Vec<TaskDto> = scans
        .into_iter()
        .map(|scan| {
            let progress = if scan.files_total > 0 {
                Some(TaskProgressDto {
                    current: scan.files_processed as i64,
                    total: scan.files_total as i64,
                    percentage: (scan.files_processed as f64 / scan.files_total as f64) * 100.0,
                })
            } else {
                None
            };

            TaskDto {
                task_id: scan.library_id.to_string(),
                task_type: "scan".to_string(),
                status: scan.status.to_string(),
                description: format!("Scanning library {}", scan.library_id),
                started_at: Some(scan.started_at),
                completed_at: scan.completed_at,
                progress,
            }
        })
        .collect();

    Ok(Json(tasks))
}

/// Get a specific task by ID
///
/// # Permission Required
/// - `libraries:read` or admin status
#[utoipa::path(
    get,
    path = "/tasks/{task_id}",
    params(
        ("task_id" = String, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task details retrieved", body = TaskDto),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Task not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Tasks"
)]
pub async fn get_task(
    Path(task_id): Path<String>,
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<TaskDto>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::LibrariesRead)?;

    // Parse task ID as UUID (for scan tasks, the task ID is the library ID)
    let library_id = uuid::Uuid::parse_str(&task_id)
        .map_err(|_| ApiError::BadRequest("Invalid task ID format".to_string()))?;

    // Get scan status
    let scan = state
        .scan_manager
        .get_status(library_id)
        .await
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    let progress = if scan.files_total > 0 {
        Some(TaskProgressDto {
            current: scan.files_processed as i64,
            total: scan.files_total as i64,
            percentage: (scan.files_processed as f64 / scan.files_total as f64) * 100.0,
        })
    } else {
        None
    };

    let task = TaskDto {
        task_id: scan.library_id.to_string(),
        task_type: "scan".to_string(),
        status: scan.status.to_string(),
        description: format!("Scanning library {}", scan.library_id),
        started_at: Some(scan.started_at),
        completed_at: scan.completed_at,
        progress,
    };

    Ok(Json(task))
}

/// Cancel a running task
///
/// # Permission Required
/// - `libraries:write` or admin status
#[utoipa::path(
    post,
    path = "/tasks/{task_id}/cancel",
    params(
        ("task_id" = String, Path, description = "Task ID to cancel")
    ),
    responses(
        (status = 204, description = "Task cancelled successfully"),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Task not found or not cancellable"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Tasks"
)]
pub async fn cancel_task(
    Path(task_id): Path<String>,
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<StatusCode, ApiError> {
    // Check permission
    auth.require_permission(&Permission::LibrariesWrite)?;

    // Parse task ID as UUID (for scan tasks)
    let library_id = uuid::Uuid::parse_str(&task_id)
        .map_err(|_| ApiError::BadRequest("Invalid task ID format".to_string()))?;

    // Cancel the scan
    state
        .scan_manager
        .cancel_scan(library_id)
        .await
        .map_err(|e| ApiError::NotFound(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}
