//! API handlers for PDF cache management
//!
//! These endpoints expose both the on-disk rendered-page cache and the
//! in-memory PDFium handle cache. Administrators can view statistics for each
//! and trigger cleanup or close-handle operations.

use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;
use uuid::Uuid;

use super::super::dto::{
    PdfCacheCleanupResultDto, PdfCacheStatsDto, PdfHandleCacheClearResultDto,
    PdfHandleCacheStatsDto, PdfPageCacheStatsDto, TriggerPdfCacheCleanupResponse,
};
use crate::api::{
    error::ApiError,
    extractors::{AppState, AuthContext},
    permissions::Permission,
};
use crate::db::repositories::TaskRepository;
use crate::require_permission;
use crate::tasks::types::TaskType;

/// Build the page-cache stats DTO from the current AppState.
async fn page_cache_stats(state: &AppState) -> Result<PdfPageCacheStatsDto, ApiError> {
    if !state.pdf_page_cache.is_enabled() {
        return Ok(PdfPageCacheStatsDto {
            total_files: 0,
            total_size_bytes: 0,
            total_size_human: "0 B".to_string(),
            book_count: 0,
            oldest_file_age_days: None,
            cache_dir: state
                .pdf_page_cache
                .cache_dir()
                .to_string_lossy()
                .to_string(),
            cache_enabled: false,
        });
    }

    let stats = state
        .pdf_page_cache
        .get_total_stats()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get PDF cache stats: {}", e)))?;
    let mut dto = PdfPageCacheStatsDto::from(stats);
    dto.cache_enabled = true;
    Ok(dto)
}

/// Build the handle-cache stats DTO from the current AppState.
fn handle_cache_stats(state: &AppState) -> PdfHandleCacheStatsDto {
    state.pdf_handle_cache.snapshot().into()
}

/// Get combined PDF cache statistics
///
/// Returns statistics for both the on-disk rendered-page cache and the
/// in-memory PDFium handle cache in a single payload.
///
/// # Permission Required
/// - Admin access required
#[utoipa::path(
    get,
    path = "/api/v1/admin/pdf-cache",
    responses(
        (status = 200, description = "Combined cache statistics retrieved successfully", body = PdfCacheStatsDto),
        (status = 403, description = "Admin access required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Admin"
)]
pub async fn get_pdf_cache_stats(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<PdfCacheStatsDto>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let pages = page_cache_stats(&state).await?;
    let handles = handle_cache_stats(&state);
    Ok(Json(PdfCacheStatsDto { pages, handles }))
}

/// Get PDFium handle cache statistics
///
/// Returns statistics about the in-memory open-document handle cache, including
/// the list of currently-resident books.
///
/// # Permission Required
/// - Admin access required
#[utoipa::path(
    get,
    path = "/api/v1/admin/pdf-cache/handles",
    responses(
        (status = 200, description = "Handle cache statistics retrieved successfully", body = PdfHandleCacheStatsDto),
        (status = 403, description = "Admin access required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Admin"
)]
pub async fn get_handle_cache_stats(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<PdfHandleCacheStatsDto>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;
    Ok(Json(handle_cache_stats(&state)))
}

/// Trigger PDF page cache cleanup task
///
/// Enqueues a background task to clean up cached PDF pages older than
/// the configured max age (default: 30 days, configurable via settings).
///
/// # Permission Required
/// - Admin access required
///
/// Returns the task ID which can be used to track progress.
#[utoipa::path(
    post,
    path = "/api/v1/admin/pdf-cache/pages/cleanup",
    responses(
        (status = 200, description = "Cleanup task queued successfully", body = TriggerPdfCacheCleanupResponse,
         example = json!({
             "taskId": "550e8400-e29b-41d4-a716-446655440000",
             "message": "PDF cache cleanup task queued successfully",
             "maxAgeDays": 30
         })
        ),
        (status = 403, description = "Admin access required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Admin"
)]
pub async fn trigger_pdf_cache_cleanup(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<TriggerPdfCacheCleanupResponse>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let max_age_days = state
        .settings_service
        .get_uint("pdf_cache.max_age_days", 30)
        .await
        .unwrap_or(30) as u32;

    let task_id = TaskRepository::enqueue(&state.db, TaskType::CleanupPdfCache, None)
        .await
        .map_err(|e| {
            ApiError::Internal(format!("Failed to enqueue PDF cache cleanup task: {}", e))
        })?;

    Ok(Json(TriggerPdfCacheCleanupResponse {
        task_id,
        message: "PDF cache cleanup task queued successfully".to_string(),
        max_age_days,
    }))
}

/// Clear the entire PDF page cache immediately (synchronous)
///
/// Deletes all cached rendered pages on disk. For selective cleanup based on
/// age, use the trigger_pdf_cache_cleanup endpoint instead.
///
/// # Permission Required
/// - Admin access required
#[utoipa::path(
    delete,
    path = "/api/v1/admin/pdf-cache/pages",
    responses(
        (status = 200, description = "Cache cleared successfully", body = PdfCacheCleanupResultDto,
         example = json!({
             "filesDeleted": 1500,
             "bytesReclaimed": 157286400,
             "bytesReclaimedHuman": "150.0 MB"
         })
        ),
        (status = 403, description = "Admin access required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Admin"
)]
pub async fn clear_pdf_cache(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<PdfCacheCleanupResultDto>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let result = state
        .pdf_page_cache
        .clear_all()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to clear PDF cache: {}", e)))?;

    Ok(Json(result.into()))
}

/// Close every PDFium handle currently held in memory.
///
/// Forces a re-open on the next page request for any book that previously had
/// a cached handle. Useful when the underlying library files have been moved
/// outside of the scanner's awareness.
///
/// # Permission Required
/// - Admin access required
#[utoipa::path(
    delete,
    path = "/api/v1/admin/pdf-cache/handles",
    responses(
        (status = 200, description = "Handle cache cleared successfully", body = PdfHandleCacheClearResultDto),
        (status = 403, description = "Admin access required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Admin"
)]
pub async fn clear_handle_cache(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<PdfHandleCacheClearResultDto>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;
    let handles_closed = state.pdf_handle_cache.clear() as u64;
    Ok(Json(PdfHandleCacheClearResultDto { handles_closed }))
}

/// Evict a single book's PDFium handle from the in-memory cache.
///
/// No-op if the book has no cached handle. The next page request for that book
/// will re-open the PDF via PDFium.
///
/// # Permission Required
/// - Admin access required
#[utoipa::path(
    delete,
    path = "/api/v1/admin/pdf-cache/handles/{book_id}",
    params(
        ("book_id" = Uuid, Path, description = "Book identifier")
    ),
    responses(
        (status = 200, description = "Handle eviction result", body = PdfHandleCacheClearResultDto),
        (status = 403, description = "Admin access required"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Admin"
)]
pub async fn evict_book_handle(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<PdfHandleCacheClearResultDto>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;
    let removed = state.pdf_handle_cache.evict(book_id);
    Ok(Json(PdfHandleCacheClearResultDto {
        handles_closed: if removed { 1 } else { 0 },
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_placeholder() {
        // Integration tests are in tests/api/pdf_cache.rs
    }
}
