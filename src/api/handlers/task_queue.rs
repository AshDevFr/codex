use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::api::{error::ApiError, extractors::AuthContext, permissions::Permission};
use crate::db::repositories::TaskRepository;
use crate::tasks::types::{TaskStats, TaskType};

use super::AppState;

// DTOs

#[derive(Debug, Deserialize)]
pub struct ListTasksParams {
    pub status: Option<String>,
    pub task_type: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u64,
}

fn default_limit() -> u64 {
    50
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTaskRequest {
    pub task_type: TaskType,
    pub priority: Option<i32>,
    pub scheduled_for: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateTaskResponse {
    pub task_id: Uuid,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskResponse {
    pub id: Uuid,
    pub task_type: String,
    pub status: String,
    pub priority: i32,
    pub library_id: Option<Uuid>,
    pub series_id: Option<Uuid>,
    pub book_id: Option<Uuid>,
    pub params: Option<serde_json::Value>,
    pub locked_by: Option<String>,
    pub locked_until: Option<DateTime<Utc>>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
    pub result: Option<serde_json::Value>,
    pub scheduled_for: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl From<crate::db::entities::tasks::Model> for TaskResponse {
    fn from(task: crate::db::entities::tasks::Model) -> Self {
        Self {
            id: task.id,
            task_type: task.task_type,
            status: task.status,
            priority: task.priority,
            library_id: task.library_id,
            series_id: task.series_id,
            book_id: task.book_id,
            params: task.params,
            locked_by: task.locked_by,
            locked_until: task.locked_until,
            attempts: task.attempts,
            max_attempts: task.max_attempts,
            last_error: task.last_error,
            result: task.result,
            scheduled_for: task.scheduled_for,
            created_at: task.created_at,
            started_at: task.started_at,
            completed_at: task.completed_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PurgeTasksParams {
    #[serde(default = "default_purge_days")]
    pub days: i64,
}

fn default_purge_days() -> i64 {
    30
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PurgeTasksResponse {
    pub deleted: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageResponse {
    pub message: String,
}

// API Handlers

/// List tasks with optional filtering
///
/// # Permission Required
/// - `tasks:read`
#[utoipa::path(
    get,
    path = "/api/v1/tasks",
    params(
        ("status" = Option<String>, Query, description = "Filter by status (pending, processing, completed, failed)"),
        ("task_type" = Option<String>, Query, description = "Filter by task type"),
        ("limit" = Option<u64>, Query, description = "Limit number of results (default: 50)")
    ),
    responses(
        (status = 200, description = "Tasks retrieved successfully", body = Vec<TaskResponse>),
        (status = 403, description = "Permission denied"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Task Queue"
)]
pub async fn list_tasks(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListTasksParams>,
    auth: AuthContext,
) -> Result<Json<Vec<TaskResponse>>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::TasksRead)?;

    let tasks = TaskRepository::list(
        &state.db,
        params.status,
        params.task_type,
        Some(params.limit),
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to list tasks: {}", e)))?;

    Ok(Json(tasks.into_iter().map(TaskResponse::from).collect()))
}

/// Get task by ID
///
/// # Permission Required
/// - `tasks:read`
#[utoipa::path(
    get,
    path = "/api/v1/tasks/{id}",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task retrieved successfully", body = TaskResponse),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Task not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Task Queue"
)]
pub async fn get_task(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<Uuid>,
    auth: AuthContext,
) -> Result<Json<TaskResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::TasksRead)?;

    let task = TaskRepository::get_by_id(&state.db, task_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get task: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    Ok(Json(TaskResponse::from(task)))
}

/// Create a new task
///
/// # Permission Required
/// - `tasks:write`
#[utoipa::path(
    post,
    path = "/api/v1/tasks",
    request_body = CreateTaskRequest,
    responses(
        (status = 200, description = "Task created successfully", body = CreateTaskResponse),
        (status = 400, description = "Invalid request"),
        (status = 403, description = "Permission denied"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Task Queue"
)]
pub async fn create_task(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<CreateTaskRequest>,
) -> Result<Json<CreateTaskResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::TasksWrite)?;

    let task_id = TaskRepository::enqueue(
        &state.db,
        request.task_type,
        request.priority.unwrap_or(0),
        request.scheduled_for,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to create task: {}", e)))?;

    Ok(Json(CreateTaskResponse { task_id }))
}

/// Cancel a task
///
/// # Permission Required
/// - `tasks:write`
#[utoipa::path(
    post,
    path = "/api/v1/tasks/{id}/cancel",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task cancelled successfully", body = MessageResponse),
        (status = 400, description = "Task cannot be cancelled"),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Task not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Task Queue"
)]
pub async fn cancel_task(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<Uuid>,
    auth: AuthContext,
) -> Result<Json<MessageResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::TasksWrite)?;

    TaskRepository::cancel(&state.db, task_id)
        .await
        .map_err(|e| {
            if e.to_string().contains("Cannot cancel") {
                ApiError::BadRequest(e.to_string())
            } else if e.to_string().contains("not found") {
                ApiError::NotFound(e.to_string())
            } else {
                ApiError::Internal(format!("Failed to cancel task: {}", e))
            }
        })?;

    Ok(Json(MessageResponse {
        message: format!("Task {} cancelled", task_id),
    }))
}

/// Unlock a stuck task
///
/// # Permission Required
/// - `tasks:write`
#[utoipa::path(
    post,
    path = "/api/v1/tasks/{id}/unlock",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task unlocked successfully", body = MessageResponse),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Task not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Task Queue"
)]
pub async fn unlock_task(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<Uuid>,
    auth: AuthContext,
) -> Result<Json<MessageResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::TasksWrite)?;

    TaskRepository::unlock(&state.db, task_id)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                ApiError::NotFound(e.to_string())
            } else {
                ApiError::Internal(format!("Failed to unlock task: {}", e))
            }
        })?;

    Ok(Json(MessageResponse {
        message: format!("Task {} unlocked", task_id),
    }))
}

/// Retry a failed task
///
/// # Permission Required
/// - `tasks:write`
#[utoipa::path(
    post,
    path = "/api/v1/tasks/{id}/retry",
    params(
        ("id" = Uuid, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task queued for retry", body = MessageResponse),
        (status = 400, description = "Task is not in failed state"),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Task not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Task Queue"
)]
pub async fn retry_task(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<Uuid>,
    auth: AuthContext,
) -> Result<Json<MessageResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::TasksWrite)?;

    let task = TaskRepository::get_by_id(&state.db, task_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get task: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;

    if task.status != "failed" {
        return Err(ApiError::BadRequest(format!(
            "Can only retry failed tasks (current status: {})",
            task.status
        )));
    }

    // Reset and unlock (this will retry the task)
    TaskRepository::unlock(&state.db, task_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to retry task: {}", e)))?;

    Ok(Json(MessageResponse {
        message: format!("Task {} queued for retry", task_id),
    }))
}

/// Get queue statistics
///
/// # Permission Required
/// - `tasks:read`
#[utoipa::path(
    get,
    path = "/api/v1/tasks/stats",
    responses(
        (status = 200, description = "Statistics retrieved successfully", body = TaskStats),
        (status = 403, description = "Permission denied"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Task Queue"
)]
pub async fn get_task_stats(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<TaskStats>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::TasksRead)?;

    let stats = TaskRepository::get_stats(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get task stats: {}", e)))?;

    Ok(Json(stats))
}

/// Purge old completed/failed tasks
///
/// # Permission Required
/// - `tasks:write`
#[utoipa::path(
    delete,
    path = "/api/v1/tasks/purge",
    params(
        ("days" = Option<i64>, Query, description = "Delete tasks older than N days (default: 30)")
    ),
    responses(
        (status = 200, description = "Tasks purged successfully", body = PurgeTasksResponse),
        (status = 403, description = "Permission denied"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Task Queue"
)]
pub async fn purge_old_tasks(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PurgeTasksParams>,
    auth: AuthContext,
) -> Result<Json<PurgeTasksResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::TasksWrite)?;

    let deleted = TaskRepository::purge_old_tasks(&state.db, params.days)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to purge tasks: {}", e)))?;

    Ok(Json(PurgeTasksResponse { deleted }))
}

/// Nuclear option: Delete ALL tasks
///
/// # Permission Required
/// - `admin`
#[utoipa::path(
    delete,
    path = "/api/v1/tasks/nuke",
    responses(
        (status = 200, description = "All tasks deleted", body = PurgeTasksResponse),
        (status = 403, description = "Permission denied (admin only)"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Task Queue"
)]
pub async fn nuke_all_tasks(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<PurgeTasksResponse>, ApiError> {
    // Require admin
    if !auth.is_admin {
        return Err(ApiError::Forbidden(
            "Admin access required to nuke all tasks".to_string(),
        ));
    }

    let deleted = TaskRepository::nuke_all_tasks(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to nuke tasks: {}", e)))?;

    Ok(Json(PurgeTasksResponse { deleted }))
}
