use super::super::dto::{
    MarkReadResponse, ReadProgressListResponse, ReadProgressResponse, UpdateProgressRequest,
};
use crate::api::{AppState, error::ApiError, extractors::AuthContext, permissions::Permission};
use crate::db::repositories::{BookRepository, ReadProgressRepository};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
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
        get_progression,
        put_progression,
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

    // Look up the book to get its page count for auto-completion detection
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Auto-detect completion: if the client explicitly set completed to true, use that.
    // Otherwise, mark as completed when current_page reaches the book's page count,
    // or when progress_percentage >= 98% (for EPUB books with reflowable content
    // where reaching exactly 100% is difficult).
    // This handles readers that send page progress but never set completed: true.
    let completed = match request.completed {
        Some(true) => true,
        _ => {
            request.current_page >= book.page_count
                || request.progress_percentage.is_some_and(|p| p >= 0.98)
        }
    };

    // Update progress with optional percentage (used for EPUB books)
    let progress = ReadProgressRepository::upsert_with_percentage(
        &state.db,
        auth.user_id,
        book_id,
        request.current_page,
        request.progress_percentage,
        completed,
        None,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to update reading progress: {}", e)))?;

    Ok(Json(progress.into()))
}

/// Get reading progress for a book
///
/// Returns the user's reading progress for a specific book.
/// If no progress exists, returns `null` with a 200 status.
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/progress",
    responses(
        (status = 200, description = "Reading progress retrieved (null if no progress exists)", body = Option<ReadProgressResponse>),
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
pub async fn get_reading_progress(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<Option<ReadProgressResponse>>, ApiError> {
    // Check permission
    auth.require_permission(&Permission::BooksRead)?;

    // Verify the book exists
    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Get progress (returns None/null if no progress exists)
    let progress = ReadProgressRepository::get_by_user_and_book(&state.db, auth.user_id, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get reading progress: {}", e)))?
        .map(ReadProgressResponse::from);

    Ok(Json(progress))
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

/// Get book progression (R2Progression / Readium standard)
///
/// Returns the stored R2Progression JSON for EPUB reading position sync.
/// Returns 200 with the progression data, or 204 if no progression exists.
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/progression",
    responses(
        (status = 200, description = "Progression data", content_type = "application/json"),
        (status = 204, description = "No progression exists"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Book not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn get_progression(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    auth.require_permission(&Permission::BooksRead)?;

    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let progress = ReadProgressRepository::get_by_user_and_book(&state.db, auth.user_id, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch progress: {}", e)))?;

    match progress.and_then(|p| p.r2_progression) {
        Some(json_str) => {
            let json_value: serde_json::Value = serde_json::from_str(&json_str)
                .map_err(|e| ApiError::Internal(format!("Invalid R2Progression JSON: {}", e)))?;
            Ok(Json(json_value).into_response())
        }
        None => Ok(StatusCode::NO_CONTENT.into_response()),
    }
}

/// Update book progression (R2Progression / Readium standard)
///
/// Stores R2Progression JSON and also updates the underlying read progress
/// (current_page, progress_percentage, completed) for backwards compatibility.
#[utoipa::path(
    put,
    path = "/api/v1/books/{book_id}/progression",
    request_body = serde_json::Value,
    responses(
        (status = 204, description = "Progression updated successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Book not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn put_progression(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::BooksRead)?;

    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let total_progression = body
        .get("locator")
        .and_then(|l| l.get("locations"))
        .and_then(|l| l.get("totalProgression"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let current_page = if book.page_count > 0 {
        (total_progression * book.page_count as f64)
            .round()
            .max(1.0) as i32
    } else {
        1
    };
    let completed =
        total_progression >= 0.98 || (book.page_count > 0 && current_page >= book.page_count);

    let json_str = serde_json::to_string(&body)
        .map_err(|e| ApiError::Internal(format!("Failed to serialize R2Progression: {}", e)))?;

    ReadProgressRepository::upsert_with_percentage(
        &state.db,
        auth.user_id,
        book_id,
        current_page,
        Some(total_progression),
        completed,
        Some(json_str),
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to update progression: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}
