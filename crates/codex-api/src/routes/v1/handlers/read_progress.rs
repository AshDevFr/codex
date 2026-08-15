use super::super::dto::{
    MarkReadResponse, ReadCompletionDto, ReadHistoryResponse, ReadProgressListResponse,
    ReadProgressResponse, UpdateProgressRequest,
};
use crate::{AppState, error::ApiError, extractors::AuthContext, permissions::Permission};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use codex_db::repositories::{
    BookRepository, ReadCompletionRepository, ReadProgressRepository, SeriesRepository,
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
        get_book_read_history,
        clear_book_read_history,
        get_series_read_history,
        clear_series_read_history,
        clear_my_read_history,
    ),
    components(schemas(
        UpdateProgressRequest,
        ReadProgressResponse,
        ReadProgressListResponse,
        MarkReadResponse,
        ReadCompletionDto,
        ReadHistoryResponse,
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
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn update_reading_progress(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    headers: axum::http::HeaderMap,
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
    let progress = ReadProgressRepository::upsert_with_percentage_and_device(
        &state.db,
        auth.user_id,
        book_id,
        request.current_page,
        request.progress_percentage,
        completed,
        None,
        &auth.client_device_context(&headers),
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to update reading progress: {}", e)))?;

    Ok(Json(progress.into()))
}

/// Get reading progress for a book
///
/// Returns the user's reading progress for a specific book, or `204 No Content`
/// if the user has not started it.
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/progress",
    responses(
        (status = 200, description = "Reading progress retrieved", body = ReadProgressResponse),
        (status = 204, description = "No reading progress recorded for this book"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Book not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn get_reading_progress(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    // Check permission
    auth.require_permission(&Permission::BooksRead)?;

    // Verify the book exists
    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let progress = ReadProgressRepository::get_by_user_and_book(&state.db, auth.user_id, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get reading progress: {}", e)))?
        .map(ReadProgressResponse::from);

    // "Not started" is a normal state, not an error. A 204 keeps the success
    // body a single concrete type so generated clients can decode it; a 200
    // with a `null` body forces the schema into a union with `null`, which
    // strict generators skip entirely.
    Ok(match progress {
        Some(progress) => Json(progress).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    })
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
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn delete_reading_progress(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    headers: axum::http::HeaderMap,
    Path(book_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    // Check permission
    auth.require_permission(&Permission::BooksRead)?;

    // Delete progress
    ReadProgressRepository::delete_with_device(
        &state.db,
        auth.user_id,
        book_id,
        &auth.client_device_context(&headers),
    )
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
        ("jwt_bearer" = []),
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
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn mark_book_as_read(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    headers: axum::http::HeaderMap,
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
    let progress = ReadProgressRepository::upsert_with_device(
        &state.db,
        auth.user_id,
        book_id,
        book.page_count,
        true,
        &auth.client_device_context(&headers),
    )
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
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn mark_book_as_unread(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    headers: axum::http::HeaderMap,
    Path(book_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    // Check permission
    auth.require_permission(&Permission::BooksRead)?;

    // Mark as unread (delete progress)
    ReadProgressRepository::delete_with_device(
        &state.db,
        auth.user_id,
        book_id,
        &auth.client_device_context(&headers),
    )
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
        ("jwt_bearer" = []),
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
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn put_progression(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    headers: axum::http::HeaderMap,
    Path(book_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::BooksRead)?;

    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let locations = body.get("locator").and_then(|l| l.get("locations"));

    let client_total_progression = locations
        .and_then(|l| l.get("totalProgression"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let client_href = body
        .get("locator")
        .and_then(|l| l.get("href"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Detect if the client is character-based (epub.js sends CFI, Readium clients don't)
    let has_cfi = locations
        .and_then(|l| l.get("cfi"))
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.is_empty());

    // Convert char-based progression to byte-based if spine items are available
    let canonical_progression = if has_cfi {
        if let Some(ref spine_json) = book.epub_spine_items {
            if let Ok(spine_items) =
                serde_json::from_str::<Vec<codex_parsers::SpineItem>>(spine_json)
            {
                codex_parsers::char_to_byte_progression(&spine_items, client_total_progression)
            } else {
                client_total_progression
            }
        } else {
            client_total_progression
        }
    } else {
        client_total_progression
    };

    // Normalize totalProgression using server-side positions if available
    let (total_progression, current_page) = if let Some(ref positions_json) = book.epub_positions {
        if let Ok(positions) =
            serde_json::from_str::<Vec<codex_parsers::EpubPosition>>(positions_json)
        {
            if let Some((normalized, position)) =
                codex_parsers::normalize_progression(&positions, client_href, canonical_progression)
            {
                (normalized, position)
            } else {
                let page = if book.page_count > 0 {
                    (canonical_progression * book.page_count as f64)
                        .round()
                        .max(1.0) as i32
                } else {
                    1
                };
                (canonical_progression, page)
            }
        } else {
            let page = if book.page_count > 0 {
                (canonical_progression * book.page_count as f64)
                    .round()
                    .max(1.0) as i32
            } else {
                1
            };
            (canonical_progression, page)
        }
    } else {
        let page = if book.page_count > 0 {
            (canonical_progression * book.page_count as f64)
                .round()
                .max(1.0) as i32
        } else {
            1
        };
        (canonical_progression, page)
    };

    let completed =
        total_progression >= 0.98 || (book.page_count > 0 && current_page >= book.page_count);

    // Store the R2Progression as-is from the client.
    // Each client uses its own locator (href + progression/CFI) for navigation.
    // The normalized values are only used for internal tracking (current_page, percentage).
    let json_str = serde_json::to_string(&body)
        .map_err(|e| ApiError::Internal(format!("Failed to serialize R2Progression: {}", e)))?;

    ReadProgressRepository::upsert_with_percentage_and_device(
        &state.db,
        auth.user_id,
        book_id,
        current_page,
        Some(total_progression),
        completed,
        Some(json_str),
        &auth.client_device_context(&headers),
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to update progression: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Read completion history
//
// The history is a separate record from current progress: marking something
// unread resets progress and leaves history intact, and clearing history leaves
// progress intact. Every endpoint here acts on the authenticated user only;
// there is no way to read or clear anyone else's history.
// ============================================================================

/// Get a book's completion history for the current user
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/read-history",
    responses(
        (status = 200, description = "Completion history", body = ReadHistoryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Book not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn get_book_read_history(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<ReadHistoryResponse>, ApiError> {
    auth.require_permission(&Permission::BooksRead)?;

    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let entries = ReadCompletionRepository::list_for_book(&state.db, auth.user_id, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get read history: {}", e)))?;

    // Newest first from the query, so the first entry is the latest completion.
    let last_completed_at = entries.first().map(|e| e.completed_at);
    Ok(Json(ReadHistoryResponse {
        read_count: entries.len() as i64,
        last_completed_at,
        entries: entries.into_iter().map(ReadCompletionDto::from).collect(),
    }))
}

/// Clear a book's completion history for the current user
///
/// Does not touch current reading progress.
#[utoipa::path(
    delete,
    path = "/api/v1/books/{book_id}/read-history",
    responses(
        (status = 204, description = "History cleared"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Book not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn clear_book_read_history(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::BooksRead)?;

    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    ReadCompletionRepository::delete_for_book(&state.db, auth.user_id, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to clear read history: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Get a series' completion history for the current user
///
/// The series counts as read once every one of its books has been read, so
/// `readCount` is the minimum across them and each entry spans from the earliest
/// book start to the latest book finish of that pass.
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/read-history",
    responses(
        (status = 200, description = "Completion history", body = ReadHistoryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn get_series_read_history(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<ReadHistoryResponse>, ApiError> {
    auth.require_permission(&Permission::SeriesRead)?;

    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let history = ReadCompletionRepository::series_history(&state.db, auth.user_id, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get read history: {}", e)))?;

    Ok(Json(ReadHistoryResponse {
        read_count: history.read_count,
        last_completed_at: history.last_completed_at,
        entries: history
            .passes
            .into_iter()
            .map(|(started_at, completed_at)| ReadCompletionDto {
                started_at,
                completed_at,
            })
            .collect(),
    }))
}

/// Clear the completion history of every book in a series, for the current user
///
/// Does not touch current reading progress.
#[utoipa::path(
    delete,
    path = "/api/v1/series/{series_id}/read-history",
    responses(
        (status = 204, description = "History cleared"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn clear_series_read_history(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::SeriesRead)?;

    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    ReadCompletionRepository::delete_for_series(&state.db, auth.user_id, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to clear read history: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Clear the current user's entire completion history
///
/// Does not touch current reading progress.
#[utoipa::path(
    delete,
    path = "/api/v1/user/read-history",
    responses(
        (status = 204, description = "History cleared"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress"
)]
pub async fn clear_my_read_history(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<StatusCode, ApiError> {
    ReadCompletionRepository::delete_all_for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to clear read history: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}
