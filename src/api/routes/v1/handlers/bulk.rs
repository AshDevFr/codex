//! Bulk operations handlers
//!
//! Handlers for bulk mark read/unread, analyze, thumbnail generation, title reprocessing,
//! and metadata reset operations on books and series.

use super::super::dto::{
    BulkAnalyzeBooksRequest, BulkAnalyzeResponse, BulkAnalyzeSeriesRequest, BulkBooksRequest,
    BulkGenerateBookThumbnailsRequest, BulkGenerateSeriesBookThumbnailsRequest,
    BulkGenerateSeriesThumbnailsRequest, BulkMetadataResetResponse, BulkRenumberSeriesRequest,
    BulkReprocessSeriesTitlesRequest, BulkSeriesRequest, BulkTaskResponse, MarkReadResponse,
};
use crate::api::{AppState, error::ApiError, extractors::AuthContext, permissions::Permission};
use crate::db::repositories::{
    AlternateTitleRepository, BookRepository, ExternalLinkRepository, ExternalRatingRepository,
    GenreRepository, ReadProgressRepository, SeriesCoversRepository, SeriesExternalIdRepository,
    SeriesMetadataRepository, SeriesRepository, SharingTagRepository, TagRepository,
    TaskRepository,
};
use crate::events::{EntityChangeEvent, EntityEvent};
use crate::require_permission;
use crate::tasks::types::TaskType;
use axum::{Json, extract::State};
use chrono::Utc;
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

// ============================================================================
// Thumbnail Bulk Handlers
// ============================================================================

/// Bulk generate thumbnails for books in multiple series
///
/// Enqueues a fan-out task that will generate thumbnails for all books in the specified series.
/// This is useful for regenerating thumbnails after changing thumbnail settings or fixing
/// corrupt thumbnails.
#[utoipa::path(
    post,
    path = "/api/v1/series/bulk/thumbnails/books/generate",
    request_body = BulkGenerateSeriesBookThumbnailsRequest,
    responses(
        (status = 200, description = "Thumbnail generation task queued", body = BulkTaskResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_generate_series_book_thumbnails(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkGenerateSeriesBookThumbnailsRequest>,
) -> Result<Json<BulkTaskResponse>, ApiError> {
    require_permission!(auth, Permission::TasksWrite)?;

    if request.series_ids.is_empty() {
        return Err(ApiError::BadRequest("No series specified".to_string()));
    }

    // Limit bulk request size
    const MAX_BULK_SERIES_COUNT: usize = 100;
    if request.series_ids.len() > MAX_BULK_SERIES_COUNT {
        return Err(ApiError::BadRequest(format!(
            "Too many series in request. Maximum is {}, got {}. Please split into smaller batches.",
            MAX_BULK_SERIES_COUNT,
            request.series_ids.len()
        )));
    }

    // Create a fan-out task for generating book thumbnails
    let task_type = TaskType::GenerateThumbnails {
        library_id: None,
        series_id: None,
        series_ids: Some(request.series_ids.clone()),
        book_ids: None,
        force: request.force,
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to queue thumbnail generation: {}", e)))?;

    Ok(Json(BulkTaskResponse {
        task_id,
        message: format!(
            "Thumbnail generation task queued for {} series",
            request.series_ids.len()
        ),
    }))
}

/// Bulk generate thumbnails for books (by book IDs)
///
/// Enqueues a fan-out task that will generate thumbnails for the specified books.
/// This is useful for regenerating thumbnails after changing thumbnail settings or fixing
/// corrupt thumbnails.
#[utoipa::path(
    post,
    path = "/api/v1/books/bulk/thumbnails/generate",
    request_body = BulkGenerateBookThumbnailsRequest,
    responses(
        (status = 200, description = "Thumbnail generation task queued", body = BulkTaskResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_generate_book_thumbnails(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkGenerateBookThumbnailsRequest>,
) -> Result<Json<BulkTaskResponse>, ApiError> {
    require_permission!(auth, Permission::TasksWrite)?;

    if request.book_ids.is_empty() {
        return Err(ApiError::BadRequest("No books specified".to_string()));
    }

    // Limit bulk request size
    const MAX_BULK_BOOK_COUNT: usize = 500;
    if request.book_ids.len() > MAX_BULK_BOOK_COUNT {
        return Err(ApiError::BadRequest(format!(
            "Too many books in request. Maximum is {}, got {}. Please split into smaller batches.",
            MAX_BULK_BOOK_COUNT,
            request.book_ids.len()
        )));
    }

    // Create a fan-out task for generating book thumbnails
    let task_type = TaskType::GenerateThumbnails {
        library_id: None,
        series_id: None,
        series_ids: None,
        book_ids: Some(request.book_ids.clone()),
        force: request.force,
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to queue thumbnail generation: {}", e)))?;

    Ok(Json(BulkTaskResponse {
        task_id,
        message: format!(
            "Thumbnail generation task queued for {} books",
            request.book_ids.len()
        ),
    }))
}

/// Bulk generate series thumbnails
///
/// Enqueues a fan-out task that will generate thumbnails for the specified series.
/// Series thumbnails are derived from the first book's cover in each series.
#[utoipa::path(
    post,
    path = "/api/v1/series/bulk/thumbnails/generate",
    request_body = BulkGenerateSeriesThumbnailsRequest,
    responses(
        (status = 200, description = "Series thumbnail generation task queued", body = BulkTaskResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_generate_series_thumbnails(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkGenerateSeriesThumbnailsRequest>,
) -> Result<Json<BulkTaskResponse>, ApiError> {
    require_permission!(auth, Permission::TasksWrite)?;

    if request.series_ids.is_empty() {
        return Err(ApiError::BadRequest("No series specified".to_string()));
    }

    // Limit bulk request size
    const MAX_BULK_SERIES_COUNT: usize = 100;
    if request.series_ids.len() > MAX_BULK_SERIES_COUNT {
        return Err(ApiError::BadRequest(format!(
            "Too many series in request. Maximum is {}, got {}. Please split into smaller batches.",
            MAX_BULK_SERIES_COUNT,
            request.series_ids.len()
        )));
    }

    // Create a fan-out task for generating series thumbnails
    let task_type = TaskType::GenerateSeriesThumbnails {
        library_id: None,
        series_ids: Some(request.series_ids.clone()),
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

    Ok(Json(BulkTaskResponse {
        task_id,
        message: format!(
            "Series thumbnail generation task queued for {} series",
            request.series_ids.len()
        ),
    }))
}

// ============================================================================
// Title Reprocessing Bulk Handlers
// ============================================================================

/// Bulk reprocess series titles
///
/// Enqueues a fan-out task that will reprocess titles for the specified series
/// using their library's preprocessing rules. This is useful when preprocessing
/// rules are added or changed after series have already been created.
#[utoipa::path(
    post,
    path = "/api/v1/series/bulk/titles/reprocess",
    request_body = BulkReprocessSeriesTitlesRequest,
    responses(
        (status = 200, description = "Title reprocessing task queued", body = BulkTaskResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_reprocess_series_titles(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkReprocessSeriesTitlesRequest>,
) -> Result<Json<BulkTaskResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    if request.series_ids.is_empty() {
        return Err(ApiError::BadRequest("No series specified".to_string()));
    }

    // Limit bulk request size
    const MAX_BULK_SERIES_COUNT: usize = 100;
    if request.series_ids.len() > MAX_BULK_SERIES_COUNT {
        return Err(ApiError::BadRequest(format!(
            "Too many series in request. Maximum is {}, got {}. Please split into smaller batches.",
            MAX_BULK_SERIES_COUNT,
            request.series_ids.len()
        )));
    }

    // Create a fan-out task for reprocessing series titles
    let task_type = TaskType::ReprocessSeriesTitles {
        library_id: None,
        series_ids: Some(request.series_ids.clone()),
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to queue title reprocessing: {}", e)))?;

    Ok(Json(BulkTaskResponse {
        task_id,
        message: format!(
            "Title reprocessing task queued for {} series",
            request.series_ids.len()
        ),
    }))
}

/// Bulk reset metadata for multiple series
///
/// Resets all metadata for the specified series back to filesystem-derived defaults.
/// Each series has its metadata row deleted and recreated, and all associated data
/// (genres, tags, alternate titles, external IDs/ratings/links, covers, metadata sources,
/// sharing tags) is cleared. User ratings, read progress, and book data are preserved.
///
/// Series that don't exist are silently skipped.
#[utoipa::path(
    post,
    path = "/api/v1/series/bulk/metadata/reset",
    request_body = BulkSeriesRequest,
    responses(
        (status = 200, description = "Metadata reset for specified series", body = BulkMetadataResetResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_reset_series_metadata(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkSeriesRequest>,
) -> Result<Json<BulkMetadataResetResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    if request.series_ids.is_empty() {
        return Ok(Json(BulkMetadataResetResponse {
            count: 0,
            message: "No series specified".to_string(),
        }));
    }

    let mut count = 0;
    for series_id in &request.series_ids {
        // Verify series exists
        let series = match SeriesRepository::get_by_id(&state.db, *series_id).await {
            Ok(Some(s)) => s,
            _ => continue,
        };

        // Clear all associated data
        let (
            genres_res,
            tags_res,
            alt_res,
            ext_ids_res,
            ext_ratings_res,
            ext_links_res,
            covers_res,
            sharing_res,
        ) = tokio::join!(
            GenreRepository::set_genres_for_series(&state.db, *series_id, vec![]),
            TagRepository::set_tags_for_series(&state.db, *series_id, vec![]),
            AlternateTitleRepository::delete_all_for_series(&state.db, *series_id),
            SeriesExternalIdRepository::delete_all_for_series(&state.db, *series_id),
            ExternalRatingRepository::delete_all_for_series(&state.db, *series_id),
            ExternalLinkRepository::delete_all_for_series(&state.db, *series_id),
            SeriesCoversRepository::delete_by_series(&state.db, *series_id),
            SharingTagRepository::set_tags_for_series(&state.db, *series_id, vec![]),
        );

        // Log errors but continue with other series
        if let Err(e) = genres_res {
            tracing::error!("Failed to clear genres for {}: {}", series_id, e);
        }
        if let Err(e) = tags_res {
            tracing::error!("Failed to clear tags for {}: {}", series_id, e);
        }
        if let Err(e) = alt_res {
            tracing::error!("Failed to clear alternate titles for {}: {}", series_id, e);
        }
        if let Err(e) = ext_ids_res {
            tracing::error!("Failed to clear external IDs for {}: {}", series_id, e);
        }
        if let Err(e) = ext_ratings_res {
            tracing::error!("Failed to clear external ratings for {}: {}", series_id, e);
        }
        if let Err(e) = ext_links_res {
            tracing::error!("Failed to clear external links for {}: {}", series_id, e);
        }
        if let Err(e) = covers_res {
            tracing::error!("Failed to clear covers for {}: {}", series_id, e);
        }
        if let Err(e) = sharing_res {
            tracing::error!("Failed to clear sharing tags for {}: {}", series_id, e);
        }

        // Delete metadata sources
        use crate::db::entities::metadata_sources;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        if let Err(e) = metadata_sources::Entity::delete_many()
            .filter(metadata_sources::Column::SeriesId.eq(*series_id))
            .exec(&state.db)
            .await
        {
            tracing::error!("Failed to clear metadata sources for {}: {}", series_id, e);
        }

        // Reset metadata
        match SeriesMetadataRepository::reset(&state.db, *series_id, &series.name).await {
            Ok(_) => {
                count += 1;
                // Touch series timestamp
                let _ = SeriesRepository::touch(&state.db, *series_id).await;

                // Emit event
                let event = EntityChangeEvent {
                    event: EntityEvent::SeriesUpdated {
                        series_id: *series_id,
                        library_id: series.library_id,
                        fields: Some(vec!["metadata_reset".to_string()]),
                    },
                    timestamp: Utc::now(),
                    user_id: Some(auth.user_id),
                };
                let _ = state.event_broadcaster.emit(event);
            }
            Err(e) => {
                tracing::error!("Failed to reset metadata for series {}: {}", series_id, e);
            }
        }
    }

    Ok(Json(BulkMetadataResetResponse {
        count,
        message: format!("Reset metadata for {} series", count),
    }))
}

/// Bulk renumber books in multiple series
///
/// Enqueues a fan-out task that will renumber books in the specified series
/// using each library's number strategy. Returns a task ID for tracking progress via SSE.
#[utoipa::path(
    post,
    path = "/api/v1/series/bulk/renumber",
    request_body = BulkRenumberSeriesRequest,
    responses(
        (status = 200, description = "Renumber task queued", body = BulkTaskResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Bulk Operations"
)]
pub async fn bulk_renumber_series(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkRenumberSeriesRequest>,
) -> Result<Json<BulkTaskResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    if request.series_ids.is_empty() {
        return Err(ApiError::BadRequest("No series specified".to_string()));
    }

    // Limit bulk request size
    const MAX_BULK_SERIES_COUNT: usize = 100;
    if request.series_ids.len() > MAX_BULK_SERIES_COUNT {
        return Err(ApiError::BadRequest(format!(
            "Too many series in request. Maximum is {}, got {}. Please split into smaller batches.",
            MAX_BULK_SERIES_COUNT,
            request.series_ids.len()
        )));
    }

    // Create a fan-out task for renumbering series
    let task_type = TaskType::RenumberSeriesBatch {
        series_ids: Some(request.series_ids.clone()),
    };

    let task_id = TaskRepository::enqueue(&state.db, task_type, 0, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to enqueue task: {}", e)))?;

    Ok(Json(BulkTaskResponse {
        task_id,
        message: format!(
            "Renumber task queued for {} series",
            request.series_ids.len()
        ),
    }))
}
