use super::super::dto::{
    MarkReadResponse, ReadProgressListResponse, ReadProgressResponse, UpdateProgressRequest,
};
use crate::api::{AppState, error::ApiError, extractors::AuthContext, permissions::Permission};
use crate::db::repositories::{BookRepository, ReadProgressRepository};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
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
        mark_book_as_read,
        mark_book_as_unread,
    ),
    components(schemas(
        UpdateProgressRequest,
        ReadProgressResponse,
        ReadProgressListResponse,
        MarkReadResponse,
    )),
    tags(
        (name = "Reading Progress", description = "Reading progress tracking endpoints")
    )
)]
#[allow(dead_code)] // OpenAPI documentation struct - referenced by utoipa derive macros
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

    // Update progress with optional percentage (used for EPUB books)
    let progress = ReadProgressRepository::upsert_with_percentage(
        &state.db,
        auth.user_id,
        book_id,
        request.current_page,
        request.progress_percentage,
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

/// Mark a book as read (completed)
#[utoipa::path(
    post,
    path = "/api/v1/books/{book_id}/read",
    responses(
        (status = 200, description = "Book marked as read", body = ReadProgressResponse),
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
pub async fn mark_book_as_read(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<ReadProgressResponse>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::BooksRead)?;

    // Get the book to get its page count
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Mark as read
    let progress =
        ReadProgressRepository::mark_as_read(&state.db, auth.user_id, book_id, book.page_count)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to mark book as read: {}", e)))?;

    Ok(Json(progress.into()))
}

/// Mark a book as unread (removes reading progress)
#[utoipa::path(
    post,
    path = "/api/v1/books/{book_id}/unread",
    responses(
        (status = 204, description = "Book marked as unread"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn mark_book_as_unread(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    // Check permission
    auth.require_permission(&Permission::BooksRead)?;

    // Mark as unread (delete progress)
    ReadProgressRepository::mark_as_unread(&state.db, auth.user_id, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to mark book as unread: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}
