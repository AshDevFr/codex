//! Bulk operations handlers
//!
//! Handlers for bulk mark read/unread and analyze operations on books and series.

use super::super::dto::{
    BulkAnalyzeBooksRequest, BulkAnalyzeResponse, BulkAnalyzeSeriesRequest, BulkBooksRequest,
    BulkSeriesRequest, MarkReadResponse,
};
use crate::api::{error::ApiError, extractors::AuthContext, permissions::Permission, AppState};
use crate::db::repositories::{
    BookRepository, ReadProgressRepository, SeriesRepository, TaskRepository,
};
use crate::require_permission;
use crate::tasks::types::TaskType;
use axum::{extract::State, Json};
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// Books Bulk Handlers
// ============================================================================

/// Bulk mark multiple books as read
///
/// Marks all specified books as read for the authenticated user.
/// Books that don't exist are silently skipped.
#[utoipa::path(
    post,
    path = "/api/v1/books/bulk/read",
    request_body = BulkBooksRequest,
    responses(
        (status = 200, description = "Books marked as read", body = MarkReadResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_mark_books_as_read(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkBooksRequest>,
) -> Result<Json<MarkReadResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    if request.book_ids.is_empty() {
        return Ok(Json(MarkReadResponse {
            count: 0,
            message: "No books specified".to_string(),
        }));
    }

    // Get book data (id, page_count) for all valid books
    let mut book_data: Vec<(Uuid, i32)> = Vec::new();
    for book_id in &request.book_ids {
        if let Ok(Some(book)) = BookRepository::get_by_id(&state.db, *book_id).await {
            book_data.push((book.id, book.page_count));
        }
    }

    if book_data.is_empty() {
        return Ok(Json(MarkReadResponse {
            count: 0,
            message: "No valid books found".to_string(),
        }));
    }

    // Mark all books as read
    let count = ReadProgressRepository::mark_series_as_read(&state.db, auth.user_id, book_data)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to mark books as read: {}", e)))?;

    Ok(Json(MarkReadResponse {
        count,
        message: format!("Marked {} books as read", count),
    }))
}

/// Bulk mark multiple books as unread
///
/// Marks all specified books as unread for the authenticated user.
/// Books that don't exist or have no progress are silently skipped.
#[utoipa::path(
    post,
    path = "/api/v1/books/bulk/unread",
    request_body = BulkBooksRequest,
    responses(
        (status = 200, description = "Books marked as unread", body = MarkReadResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_mark_books_as_unread(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkBooksRequest>,
) -> Result<Json<MarkReadResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    if request.book_ids.is_empty() {
        return Ok(Json(MarkReadResponse {
            count: 0,
            message: "No books specified".to_string(),
        }));
    }

    // Mark all books as unread (delete progress records)
    let count =
        ReadProgressRepository::mark_series_as_unread(&state.db, auth.user_id, request.book_ids)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to mark books as unread: {}", e)))?;

    Ok(Json(MarkReadResponse {
        count: count as usize,
        message: format!("Marked {} books as unread", count),
    }))
}

/// Bulk analyze multiple books
///
/// Enqueues analysis tasks for all specified books.
/// Books that don't exist are silently skipped.
#[utoipa::path(
    post,
    path = "/api/v1/books/bulk/analyze",
    request_body = BulkAnalyzeBooksRequest,
    responses(
        (status = 200, description = "Analysis tasks enqueued", body = BulkAnalyzeResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_analyze_books(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkAnalyzeBooksRequest>,
) -> Result<Json<BulkAnalyzeResponse>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    if request.book_ids.is_empty() {
        return Ok(Json(BulkAnalyzeResponse {
            tasks_enqueued: 0,
            message: "No books specified".to_string(),
        }));
    }

    let mut enqueued = 0;
    for book_id in &request.book_ids {
        // Verify book exists
        if BookRepository::get_by_id(&state.db, *book_id)
            .await
            .ok()
            .flatten()
            .is_none()
        {
            continue;
        }

        // Enqueue AnalyzeBook task
        let task_type = TaskType::AnalyzeBook {
            book_id: *book_id,
            force: request.force,
        };

        match TaskRepository::enqueue(&state.db, task_type, 0, None).await {
            Ok(_) => enqueued += 1,
            Err(e) => {
                tracing::error!("Failed to enqueue task for book {}: {}", book_id, e);
            }
        }
    }

    Ok(Json(BulkAnalyzeResponse {
        tasks_enqueued: enqueued,
        message: format!("Enqueued {} analysis tasks", enqueued),
    }))
}

// ============================================================================
// Series Bulk Handlers
// ============================================================================

/// Bulk mark multiple series as read
///
/// Marks all books in the specified series as read for the authenticated user.
/// Series that don't exist are silently skipped.
#[utoipa::path(
    post,
    path = "/api/v1/series/bulk/read",
    request_body = BulkSeriesRequest,
    responses(
        (status = 200, description = "Series marked as read", body = MarkReadResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_mark_series_as_read(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkSeriesRequest>,
) -> Result<Json<MarkReadResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    if request.series_ids.is_empty() {
        return Ok(Json(MarkReadResponse {
            count: 0,
            message: "No series specified".to_string(),
        }));
    }

    let mut total_count = 0;

    for series_id in &request.series_ids {
        // Verify series exists
        if SeriesRepository::get_by_id(&state.db, *series_id)
            .await
            .ok()
            .flatten()
            .is_none()
        {
            continue;
        }

        // Get all books in the series
        let books = match BookRepository::list_by_series(&state.db, *series_id, false).await {
            Ok(books) => books,
            Err(_) => continue,
        };

        if books.is_empty() {
            continue;
        }

        // Create book data for marking as read
        let book_data: Vec<(Uuid, i32)> = books
            .iter()
            .map(|book| (book.id, book.page_count))
            .collect();

        // Mark all books as read
        match ReadProgressRepository::mark_series_as_read(&state.db, auth.user_id, book_data).await
        {
            Ok(count) => total_count += count,
            Err(e) => {
                tracing::error!("Failed to mark series {} as read: {}", series_id, e);
            }
        }
    }

    Ok(Json(MarkReadResponse {
        count: total_count,
        message: format!("Marked {} books as read", total_count),
    }))
}

/// Bulk mark multiple series as unread
///
/// Marks all books in the specified series as unread for the authenticated user.
/// Series that don't exist are silently skipped.
#[utoipa::path(
    post,
    path = "/api/v1/series/bulk/unread",
    request_body = BulkSeriesRequest,
    responses(
        (status = 200, description = "Series marked as unread", body = MarkReadResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_mark_series_as_unread(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkSeriesRequest>,
) -> Result<Json<MarkReadResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    if request.series_ids.is_empty() {
        return Ok(Json(MarkReadResponse {
            count: 0,
            message: "No series specified".to_string(),
        }));
    }

    let mut total_count: u64 = 0;

    for series_id in &request.series_ids {
        // Verify series exists
        if SeriesRepository::get_by_id(&state.db, *series_id)
            .await
            .ok()
            .flatten()
            .is_none()
        {
            continue;
        }

        // Get all books in the series
        let books = match BookRepository::list_by_series(&state.db, *series_id, false).await {
            Ok(books) => books,
            Err(_) => continue,
        };

        if books.is_empty() {
            continue;
        }

        let book_ids: Vec<Uuid> = books.iter().map(|book| book.id).collect();

        // Mark all books as unread
        match ReadProgressRepository::mark_series_as_unread(&state.db, auth.user_id, book_ids).await
        {
            Ok(count) => total_count += count,
            Err(e) => {
                tracing::error!("Failed to mark series {} as unread: {}", series_id, e);
            }
        }
    }

    Ok(Json(MarkReadResponse {
        count: total_count as usize,
        message: format!("Marked {} books as unread", total_count),
    }))
}

/// Bulk analyze multiple series
///
/// Enqueues analysis tasks for all books in the specified series.
/// Series that don't exist are silently skipped.
#[utoipa::path(
    post,
    path = "/api/v1/series/bulk/analyze",
    request_body = BulkAnalyzeSeriesRequest,
    responses(
        (status = 200, description = "Analysis tasks enqueued", body = BulkAnalyzeResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_analyze_series(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkAnalyzeSeriesRequest>,
) -> Result<Json<BulkAnalyzeResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    if request.series_ids.is_empty() {
        return Ok(Json(BulkAnalyzeResponse {
            tasks_enqueued: 0,
            message: "No series specified".to_string(),
        }));
    }

    let mut enqueued = 0;

    for series_id in &request.series_ids {
        // Verify series exists
        if SeriesRepository::get_by_id(&state.db, *series_id)
            .await
            .ok()
            .flatten()
            .is_none()
        {
            continue;
        }

        // Enqueue AnalyzeSeries task (which will create individual book tasks)
        // Note: We enqueue individual book tasks for more granular control
        let books = match BookRepository::list_by_series(&state.db, *series_id, false).await {
            Ok(books) => books,
            Err(_) => continue,
        };

        for book in books {
            let task_type = TaskType::AnalyzeBook {
                book_id: book.id,
                force: request.force,
            };

            match TaskRepository::enqueue(&state.db, task_type, 0, None).await {
                Ok(_) => enqueued += 1,
                Err(e) => {
                    tracing::error!("Failed to enqueue task for book {}: {}", book.id, e);
                }
            }
        }
    }

    Ok(Json(BulkAnalyzeResponse {
        tasks_enqueued: enqueued,
        message: format!("Enqueued {} analysis tasks", enqueued),
    }))
}
