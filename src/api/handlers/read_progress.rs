use crate::api::{
    dto::{ReadProgressListResponse, ReadProgressResponse, UpdateProgressRequest},
    error::ApiError,
    extractors::AuthContext,
    permissions::Permission,
    AppState,
};
use crate::db::repositories::ReadProgressRepository;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use utoipa::OpenApi;
use uuid::Uuid;

#[derive(OpenApi)]
#[openapi(
    paths(
        update_reading_progress,
        get_reading_progress,
        delete_reading_progress,
        get_user_progress,
        get_currently_reading,
    ),
    components(schemas(
        UpdateProgressRequest,
        ReadProgressResponse,
        ReadProgressListResponse,
    )),
    tags(
        (name = "Reading Progress", description = "Reading progress tracking endpoints")
    )
)]
pub struct ReadProgressApi;

/// Update reading progress for a book
#[utoipa::path(
    put,
    path = "/api/v1/books/{book_id}/progress",
    request_body = UpdateProgressRequest,
    responses(
        (status = 200, description = "Progress updated successfully", body = ReadProgressResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Book not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn update_reading_progress(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<UpdateProgressRequest>,
) -> Result<Json<ReadProgressResponse>, ApiError> {
    // Check permission - users can manage their own reading progress
    auth.require_permission(&Permission::BooksRead)?;

    // Update progress
    let progress = ReadProgressRepository::upsert(
        &state.db,
        auth.user_id,
        book_id,
        request.current_page,
        request.completed,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to update reading progress: {}", e)))?;

    Ok(Json(progress.into()))
}

/// Get reading progress for a book
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/progress",
    responses(
        (status = 200, description = "Reading progress retrieved", body = ReadProgressResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Progress not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn get_reading_progress(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<ReadProgressResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::BooksRead)?;

    // Get progress
    let progress = ReadProgressRepository::get_by_user_and_book(&state.db, auth.user_id, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get reading progress: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Reading progress not found".to_string()))?;

    Ok(Json(progress.into()))
}

/// Delete reading progress for a book
#[utoipa::path(
    delete,
    path = "/api/v1/books/{book_id}/progress",
    responses(
        (status = 204, description = "Progress deleted successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn delete_reading_progress(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    // Check permission
    auth.require_permission(&Permission::BooksRead)?;

    // Delete progress
    ReadProgressRepository::delete(&state.db, auth.user_id, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete reading progress: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Get all reading progress for the authenticated user
#[utoipa::path(
    get,
    path = "/api/v1/progress",
    responses(
        (status = 200, description = "User reading progress retrieved", body = ReadProgressListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn get_user_progress(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<ReadProgressListResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::BooksRead)?;

    // Get all progress for user
    let progress_list = ReadProgressRepository::get_by_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get user progress: {}", e)))?;

    let total = progress_list.len();
    let progress: Vec<ReadProgressResponse> = progress_list.into_iter().map(Into::into).collect();

    Ok(Json(ReadProgressListResponse { progress, total }))
}

/// Get currently reading books for the authenticated user
#[utoipa::path(
    get,
    path = "/api/v1/progress/currently-reading",
    responses(
        (status = 200, description = "Currently reading books retrieved", body = ReadProgressListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn get_currently_reading(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<ReadProgressListResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::BooksRead)?;

    // Get currently reading books (limit to 50)
    let progress_list = ReadProgressRepository::get_currently_reading(&state.db, auth.user_id, 50)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get currently reading: {}", e)))?;

    let total = progress_list.len();
    let progress: Vec<ReadProgressResponse> = progress_list.into_iter().map(Into::into).collect();

    Ok(Json(ReadProgressListResponse { progress, total }))
}
