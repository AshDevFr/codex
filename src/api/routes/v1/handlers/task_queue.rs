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
use crate::db::repositories::{
    LibraryRepository, SeriesMetadataRepository, SeriesRepository, TaskRepository,
};
use crate::require_permission;
use crate::tasks::types::{TaskStats, TaskType};

use super::super::dto::series::{
    EnqueueReprocessTitleRequest, EnqueueReprocessTitleResponse, ReprocessSeriesTitlesRequest,
    ReprocessTitleRequest,
};
use crate::api::AppState;

// DTOs

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
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
    /// Type of task to create
    pub task_type: TaskType,

    /// Priority level (higher = more urgent)
    #[schema(example = 0)]
    pub priority: Option<i32>,

    /// When to run the task (defaults to now)
    #[schema(example = "2024-01-15T12:00:00Z")]
    pub scheduled_for: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateTaskResponse {
    /// ID of the created task
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub task_id: Uuid,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskResponse {
    /// Unique task identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// Type of task (scan_library, generate_thumbnail, etc.)
    #[schema(example = "scan_library")]
    pub task_type: String,

    /// Current status (pending, processing, completed, failed)
    #[schema(example = "pending")]
    pub status: String,

    /// Priority level (higher = more urgent)
    #[schema(example = 0)]
    pub priority: i32,

    /// Associated library ID (if applicable)
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub library_id: Option<Uuid>,

    /// Associated series ID (if applicable)
    pub series_id: Option<Uuid>,

    /// Associated book ID (if applicable)
    pub book_id: Option<Uuid>,

    /// Task-specific parameters
    pub params: Option<serde_json::Value>,

    /// Worker ID that has locked this task
    #[schema(example = "worker-1")]
    pub locked_by: Option<String>,

    /// When the lock expires
    pub locked_until: Option<DateTime<Utc>>,

    /// Number of execution attempts
    #[schema(example = 0)]
    pub attempts: i32,

    /// Maximum number of allowed attempts
    #[schema(example = 3)]
    pub max_attempts: i32,

    /// Error message from last failed attempt
    pub last_error: Option<String>,

    /// Task execution result
    pub result: Option<serde_json::Value>,

    /// When the task is scheduled to run
    #[schema(example = "2024-01-15T12:00:00Z")]
    pub scheduled_for: DateTime<Utc>,

    /// When the task was created
    #[schema(example = "2024-01-15T10:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When task execution started
    pub started_at: Option<DateTime<Utc>>,

    /// When task execution completed
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
    /// Number of tasks deleted
    #[schema(example = 42)]
    pub deleted: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageResponse {
    /// Response message
    #[schema(example = "Task 550e8400-e29b-41d4-a716-446655440000 cancelled")]
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
    params(ListTasksParams),
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
    path = "/api/v1/tasks/{task_id}",
    params(
        ("task_id" = Uuid, Path, description = "Task ID")
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
    path = "/api/v1/tasks/{task_id}/cancel",
    params(
        ("task_id" = Uuid, Path, description = "Task ID")
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
    path = "/api/v1/tasks/{task_id}/unlock",
    params(
        ("task_id" = Uuid, Path, description = "Task ID")
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
    path = "/api/v1/tasks/{task_id}/retry",
    params(
        ("task_id" = Uuid, Path, description = "Task ID")
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
    require_permission!(auth, Permission::SystemAdmin)?;

    let deleted = TaskRepository::nuke_all_tasks(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to nuke tasks: {}", e)))?;

    Ok(Json(PurgeTasksResponse { deleted }))
}

// Thumbnail generation endpoints

/// Request body for batch book thumbnail generation
#[derive(Debug, Deserialize, ToSchema)]
pub struct GenerateBookThumbnailsRequest {
    /// If true, regenerate all thumbnails even if they exist. If false (default), only generate missing thumbnails.
    #[serde(default)]
    #[schema(example = false)]
    pub force: bool,

    /// Optional: scope to a specific library
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub library_id: Option<Uuid>,

    /// Optional: scope to a specific series (within library if both provided)
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub series_id: Option<Uuid>,

    /// Optional: specific series IDs to generate thumbnails for books within (takes precedence over series_id and library_id)
    #[serde(default)]
    pub series_ids: Option<Vec<Uuid>>,

    /// Optional: specific book IDs to generate thumbnails for (takes precedence over all other scopes)
    #[serde(default)]
    pub book_ids: Option<Vec<Uuid>>,
}

/// Request body for batch series thumbnail generation
#[derive(Debug, Deserialize, ToSchema)]
pub struct GenerateSeriesThumbnailsRequest {
    /// If true, regenerate all thumbnails even if they exist. If false (default), only generate missing thumbnails.
    #[serde(default)]
    #[schema(example = false)]
    pub force: bool,

    /// Optional: scope to a specific library
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub library_id: Option<Uuid>,

    /// Optional: specific series IDs to generate thumbnails for (takes precedence over library_id)
    #[serde(default)]
    pub series_ids: Option<Vec<Uuid>>,
}

/// Generate thumbnails for books in a scope
///
/// This queues a fan-out task that enqueues individual thumbnail generation tasks for each book.
///
/// **Scope priority:**
/// 1. If `book_ids` is provided, only those specific books
/// 2. If `series_ids` is provided, only books in those specific series
/// 3. If `series_id` is provided, only books in that series
/// 4. If `library_id` is provided, only books in that library
/// 5. If none provided, all books in all libraries
///
/// **Force behavior:**
/// - `force: false` (default): Only generates thumbnails for books that don't have one
/// - `force: true`: Regenerates all thumbnails, replacing existing ones
///
/// # Permission Required
/// - `tasks:write`
#[utoipa::path(
    post,
    path = "/api/v1/books/thumbnails/generate",
    request_body = GenerateBookThumbnailsRequest,
    responses(
        (status = 200, description = "Thumbnail generation task queued", body = CreateTaskResponse),
        (status = 403, description = "Permission denied"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Thumbnails"
)]
pub async fn generate_book_thumbnails(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<GenerateBookThumbnailsRequest>,
) -> Result<Json<CreateTaskResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::TasksWrite)?;

    // Validate scope IDs if no explicit book_ids or series_ids provided
    if request.book_ids.is_none() && request.series_ids.is_none() {
        if let Some(library_id) = request.library_id {
            use crate::db::repositories::LibraryRepository;
            LibraryRepository::get_by_id(&state.db, library_id)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to check library: {}", e)))?
                .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;
        }

        if let Some(series_id) = request.series_id {
            use crate::db::repositories::SeriesRepository;
            SeriesRepository::get_by_id(&state.db, series_id)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to check series: {}", e)))?
                .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;
        }
    }

    let task_type = TaskType::GenerateThumbnails {
        library_id: request.library_id,
        series_id: request.series_id,
        series_ids: request.series_ids,
        book_ids: request.book_ids,
        force: request.force,
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to queue thumbnail generation: {}", e)))?;

    Ok(Json(CreateTaskResponse { task_id }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ForceRequest {
    /// If true, regenerate thumbnails even if they exist. If false (default), only generate missing thumbnails.
    #[serde(default)]
    #[schema(example = false)]
    pub force: bool,
}

/// Generate thumbnails for all books in a library
///
/// Queues a fan-out task that enqueues individual thumbnail generation tasks for each book in the library.
///
/// # Permission Required
/// - `tasks:write`
#[utoipa::path(
    post,
    path = "/api/v1/libraries/{library_id}/books/thumbnails/generate",
    params(
        ("library_id" = Uuid, Path, description = "Library ID")
    ),
    request_body = ForceRequest,
    responses(
        (status = 200, description = "Thumbnail generation task queued", body = CreateTaskResponse),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Library not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Thumbnails"
)]
pub async fn generate_library_book_thumbnails(
    State(state): State<Arc<AppState>>,
    Path(library_id): Path<Uuid>,
    auth: AuthContext,
    Json(request): Json<ForceRequest>,
) -> Result<Json<CreateTaskResponse>, ApiError> {
    use crate::db::repositories::LibraryRepository;

    // Check permission
    auth.require_permission(&Permission::TasksWrite)?;

    // Verify library exists
    LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    let task_type = TaskType::GenerateThumbnails {
        library_id: Some(library_id),
        series_id: None,
        series_ids: None,
        book_ids: None,
        force: request.force,
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to queue thumbnail generation: {}", e)))?;

    Ok(Json(CreateTaskResponse { task_id }))
}

/// Generate thumbnail for a single book
///
/// Queues a task to generate (or regenerate) the thumbnail for a specific book.
///
/// # Permission Required
/// - `tasks:write`
#[utoipa::path(
    post,
    path = "/api/v1/books/{book_id}/thumbnail/generate",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = ForceRequest,
    responses(
        (status = 200, description = "Thumbnail generation task queued", body = CreateTaskResponse),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Book not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Thumbnails"
)]
pub async fn generate_book_thumbnail(
    State(state): State<Arc<AppState>>,
    Path(book_id): Path<Uuid>,
    auth: AuthContext,
    Json(request): Json<ForceRequest>,
) -> Result<Json<CreateTaskResponse>, ApiError> {
    use crate::db::repositories::BookRepository;

    // Check permission
    auth.require_permission(&Permission::TasksWrite)?;

    // Verify book exists
    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let task_type = TaskType::GenerateThumbnail {
        book_id,
        force: request.force,
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to queue thumbnail generation: {}", e)))?;

    Ok(Json(CreateTaskResponse { task_id }))
}

/// Generate thumbnail for a series
///
/// Queues a task to generate (or regenerate) the thumbnail for a specific series.
/// The series thumbnail is derived from the first book's cover.
///
/// # Permission Required
/// - `tasks:write`
#[utoipa::path(
    post,
    path = "/api/v1/series/{series_id}/thumbnail/generate",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = ForceRequest,
    responses(
        (status = 200, description = "Thumbnail generation task queued", body = CreateTaskResponse),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Thumbnails"
)]
pub async fn generate_series_thumbnail(
    State(state): State<Arc<AppState>>,
    Path(series_id): Path<Uuid>,
    auth: AuthContext,
    Json(request): Json<ForceRequest>,
) -> Result<Json<CreateTaskResponse>, ApiError> {
    use crate::db::repositories::SeriesRepository;

    // Check permission
    auth.require_permission(&Permission::TasksWrite)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let task_type = TaskType::GenerateSeriesThumbnail {
        series_id,
        force: request.force,
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| {
            ApiError::Internal(format!(
                "Failed to queue series thumbnail generation: {}",
                e
            ))
        })?;

    Ok(Json(CreateTaskResponse { task_id }))
}

/// Generate thumbnails for series in a scope
///
/// This queues a fan-out task that enqueues individual series thumbnail generation tasks.
/// Series thumbnails are the cover images displayed for each series (derived from the first book's cover).
///
/// **Scope priority:**
/// 1. If `series_ids` is provided, only those specific series
/// 2. If `library_id` is provided, only series in that library
/// 3. If neither provided, all series in all libraries
///
/// **Force behavior:**
/// - `force: false` (default): Only generates thumbnails for series that don't have one
/// - `force: true`: Regenerates all thumbnails, replacing existing ones
///
/// # Permission Required
/// - `tasks:write`
#[utoipa::path(
    post,
    path = "/api/v1/series/thumbnails/generate",
    request_body = GenerateSeriesThumbnailsRequest,
    responses(
        (status = 200, description = "Series thumbnail generation task queued", body = CreateTaskResponse),
        (status = 403, description = "Permission denied"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Thumbnails"
)]
pub async fn generate_series_thumbnails(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<GenerateSeriesThumbnailsRequest>,
) -> Result<Json<CreateTaskResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::TasksWrite)?;

    // If library_id provided (and no series_ids), verify it exists
    if request.series_ids.is_none() {
        if let Some(library_id) = request.library_id {
            use crate::db::repositories::LibraryRepository;
            LibraryRepository::get_by_id(&state.db, library_id)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to check library: {}", e)))?
                .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;
        }
    }

    let task_type = TaskType::GenerateSeriesThumbnails {
        library_id: request.library_id,
        series_ids: request.series_ids,
        force: request.force,
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| {
            ApiError::Internal(format!(
                "Failed to queue series thumbnail generation: {}",
                e
            ))
        })?;

    Ok(Json(CreateTaskResponse { task_id }))
}

/// Generate thumbnails for all series in a library
///
/// Queues a fan-out task that generates thumbnails for all series in the specified library.
///
/// # Permission Required
/// - `tasks:write`
#[utoipa::path(
    post,
    path = "/api/v1/libraries/{library_id}/series/thumbnails/generate",
    params(
        ("library_id" = Uuid, Path, description = "Library ID")
    ),
    request_body = ForceRequest,
    responses(
        (status = 200, description = "Series thumbnail generation task queued", body = CreateTaskResponse),
        (status = 403, description = "Permission denied"),
        (status = 404, description = "Library not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Thumbnails"
)]
pub async fn generate_library_series_thumbnails(
    State(state): State<Arc<AppState>>,
    Path(library_id): Path<Uuid>,
    auth: AuthContext,
    Json(request): Json<ForceRequest>,
) -> Result<Json<CreateTaskResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::TasksWrite)?;

    // Verify library exists
    LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    let task_type = TaskType::GenerateSeriesThumbnails {
        library_id: Some(library_id),
        series_ids: None,
        force: request.force,
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| {
            ApiError::Internal(format!(
                "Failed to queue series thumbnail generation: {}",
                e
            ))
        })?;

    Ok(Json(CreateTaskResponse { task_id }))
}

// =============================================================================
// Reprocess Title Task Endpoints
// =============================================================================

/// Reprocess a series title using library preprocessing rules
///
/// Applies the library's preprocessing rules to the series' original directory name
/// to regenerate the display title. This is useful when preprocessing rules are added
/// or changed after series have already been created.
///
/// The title will only be updated if:
/// - The `title_lock` is false (respects user edits)
/// - The preprocessing rules produce a different title
///
/// If the title is changed and `title_sort_lock` is false, the `title_sort` will be
/// cleared (set to None) to let it fall back to the new title for sorting.
///
/// - With `dryRun: true`: Returns a synchronous preview of what would change
/// - With `dryRun: false` (default): Enqueues a background task to process
///
/// # Permission Required
/// - `series:write`
#[utoipa::path(
    post,
    path = "/api/v1/series/{series_id}/title/reprocess",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body(content = EnqueueReprocessTitleRequest, content_type = "application/json"),
    responses(
        (status = 200, description = "Task enqueued or dry run preview", body = EnqueueReprocessTitleResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Tasks"
)]
pub async fn reprocess_series_title(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<EnqueueReprocessTitleRequest>,
) -> Result<Json<EnqueueReprocessTitleResponse>, ApiError> {
    use crate::services::metadata::preprocessing::apply_rules;

    auth.require_permission(&Permission::SeriesWrite)?;

    // Verify series exists
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // If dry run, do synchronous preview
    if request.dry_run {
        // Fetch the series metadata
        let metadata = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .ok_or_else(|| ApiError::Internal("Series metadata not found".to_string()))?;

        // Check if title is locked
        if metadata.title_lock {
            return Ok(Json(EnqueueReprocessTitleResponse {
                success: true,
                tasks_enqueued: 0,
                task_ids: vec![],
                message: format!(
                    "Dry run: Series title is locked. Original: '{}', would remain unchanged.",
                    metadata.title
                ),
            }));
        }

        // Fetch the library to get preprocessing rules
        let library = LibraryRepository::get_by_id(&state.db, series.library_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .ok_or_else(|| ApiError::Internal("Library not found".to_string()))?;

        // Get preprocessing rules from library
        let rules = LibraryRepository::get_preprocessing_rules(&library);

        // Apply rules to the series name (original directory name)
        let new_title = if rules.is_empty() {
            series.name.clone()
        } else {
            apply_rules(&series.name, &rules)
        };

        let original_title = metadata.title.clone();
        let changed = new_title != original_title;

        return Ok(Json(EnqueueReprocessTitleResponse {
            success: true,
            tasks_enqueued: 0,
            task_ids: vec![],
            message: if changed {
                format!(
                    "Dry run: Title would change from '{}' to '{}'",
                    original_title, new_title
                )
            } else {
                format!(
                    "Dry run: Title would remain unchanged: '{}'",
                    original_title
                )
            },
        }));
    }

    // Enqueue the task
    let task_type = TaskType::ReprocessSeriesTitle { series_id };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to enqueue task: {}", e)))?;

    Ok(Json(EnqueueReprocessTitleResponse {
        success: true,
        tasks_enqueued: 1,
        task_ids: vec![task_id],
        message: "Reprocess title task enqueued".to_string(),
    }))
}

/// Reprocess all series titles in a library using preprocessing rules
///
/// Applies the library's preprocessing rules to all series' original directory names
/// to regenerate the display titles. This is useful when preprocessing rules are added
/// or changed after series have already been created.
///
/// Each series title will only be updated if:
/// - The `title_lock` is false (respects user edits)
/// - The preprocessing rules produce a different title
///
/// If a title is changed and `title_sort_lock` is false, the `title_sort` will be
/// cleared (set to None) to let it fall back to the new title for sorting.
///
/// - With `dryRun: true`: Returns a synchronous preview of what would change
/// - With `dryRun: false` (default): Enqueues a background task to process
///
/// # Permission Required
/// - `libraries:write`
#[utoipa::path(
    post,
    path = "/api/v1/libraries/{library_id}/series/titles/reprocess",
    params(
        ("library_id" = Uuid, Path, description = "Library ID")
    ),
    request_body(content = ReprocessTitleRequest, content_type = "application/json"),
    responses(
        (status = 200, description = "Task enqueued or dry run preview", body = EnqueueReprocessTitleResponse),
        (status = 404, description = "Library not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Tasks"
)]
pub async fn reprocess_library_series_titles(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Json(request): Json<ReprocessTitleRequest>,
) -> Result<Json<EnqueueReprocessTitleResponse>, ApiError> {
    use crate::services::metadata::preprocessing::apply_rules;

    auth.require_permission(&Permission::LibrariesWrite)?;

    // Fetch the library
    let library = LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    // If dry run, do synchronous preview
    if request.dry_run {
        // Get preprocessing rules from library
        let rules = LibraryRepository::get_preprocessing_rules(&library);

        // Fetch all series in library
        let series_list = SeriesRepository::list_by_library(&state.db, library_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

        let total_series = series_list.len();

        if series_list.is_empty() {
            return Ok(Json(EnqueueReprocessTitleResponse {
                success: true,
                tasks_enqueued: 0,
                task_ids: vec![],
                message: "Dry run: No series in library".to_string(),
            }));
        }

        // Batch fetch all metadata
        let series_ids: Vec<Uuid> = series_list.iter().map(|s| s.id).collect();
        let metadata_map = SeriesMetadataRepository::get_by_series_ids(&state.db, &series_ids)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

        let mut skipped = 0;
        let mut changed = 0;

        // Preview each series
        for series in &series_list {
            let metadata = match metadata_map.get(&series.id) {
                Some(m) => m,
                None => {
                    skipped += 1;
                    continue;
                }
            };

            if metadata.title_lock {
                skipped += 1;
                continue;
            }

            let new_title = if rules.is_empty() {
                series.name.clone()
            } else {
                apply_rules(&series.name, &rules)
            };

            if new_title != metadata.title {
                changed += 1;
            }
        }

        let processed = total_series - skipped;

        return Ok(Json(EnqueueReprocessTitleResponse {
            success: true,
            tasks_enqueued: 0,
            task_ids: vec![],
            message: format!(
                "Dry run: {} total series, {} would be processed, {} would change, {} would be skipped",
                total_series, processed, changed, skipped
            ),
        }));
    }

    // Enqueue the fan-out task
    let task_type = TaskType::ReprocessSeriesTitles {
        library_id: Some(library_id),
        series_ids: None,
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to enqueue task: {}", e)))?;

    Ok(Json(EnqueueReprocessTitleResponse {
        success: true,
        tasks_enqueued: 1,
        task_ids: vec![task_id],
        message: "Reprocess library titles task enqueued".to_string(),
    }))
}

/// Reprocess series titles in a scope
///
/// This queues a fan-out task that enqueues individual series title reprocessing tasks.
/// Applies the library's preprocessing rules to regenerate display titles.
///
/// **Scope priority:**
/// 1. If `series_ids` is provided, only those specific series
/// 2. If `library_id` is provided, only series in that library
/// 3. If neither provided, all series in all libraries
///
/// **Lock behavior:**
/// - Series with `title_lock: true` are skipped
/// - If title changes and `title_sort_lock` is false, `title_sort` is cleared
///
/// # Permission Required
/// - `series:write`
#[utoipa::path(
    post,
    path = "/api/v1/series/titles/reprocess",
    request_body = ReprocessSeriesTitlesRequest,
    responses(
        (status = 200, description = "Task enqueued", body = EnqueueReprocessTitleResponse),
        (status = 400, description = "Invalid request"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Tasks"
)]
pub async fn reprocess_series_titles(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<ReprocessSeriesTitlesRequest>,
) -> Result<Json<EnqueueReprocessTitleResponse>, ApiError> {
    auth.require_permission(&Permission::SeriesWrite)?;

    // Limit bulk request size if series_ids provided
    if let Some(ref series_ids) = request.series_ids {
        const MAX_BULK_SERIES_COUNT: usize = 100;
        if series_ids.len() > MAX_BULK_SERIES_COUNT {
            return Err(ApiError::BadRequest(format!(
                "Too many series in request. Maximum is {}, got {}. Please split into smaller batches.",
                MAX_BULK_SERIES_COUNT,
                series_ids.len()
            )));
        }
    }

    // If library_id provided (and no series_ids), verify it exists
    if request.series_ids.is_none() {
        if let Some(library_id) = request.library_id {
            LibraryRepository::get_by_id(&state.db, library_id)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to check library: {}", e)))?
                .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;
        }
    }

    // Enqueue the fan-out task
    let task_type = TaskType::ReprocessSeriesTitles {
        library_id: request.library_id,
        series_ids: request.series_ids.clone(),
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to enqueue task: {}", e)))?;

    Ok(Json(EnqueueReprocessTitleResponse {
        success: true,
        tasks_enqueued: 1,
        task_ids: vec![task_id],
        message: "Reprocess series titles task enqueued".to_string(),
    }))
}
