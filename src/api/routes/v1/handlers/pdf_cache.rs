//! API handlers for PDF cache management
//!
//! These endpoints allow administrators to view cache statistics
//! and trigger cache cleanup operations.

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::api::{
    error::ApiError,
    extractors::{AppState, AuthContext},
    permissions::Permission,
};
use super::super::dto::{PdfCacheCleanupResultDto, PdfCacheStatsDto, TriggerPdfCacheCleanupResponse};
use crate::db::repositories::TaskRepository;
use crate::require_permission;
use crate::tasks::types::TaskType;

/// Get PDF cache statistics
///
/// Returns statistics about the PDF page cache including total files,
/// total size, number of books with cached pages, and cache status.
///
/// # Permission Required
/// - Admin access required
#[utoipa::path(
    get,
    path = "/api/v1/admin/pdf-cache/stats",
    responses(
        (status = 200, description = "Cache statistics retrieved successfully", body = PdfCacheStatsDto,
         example = json!({
             "total_files": 1500,
             "total_size_bytes": 157286400,
             "total_size_human": "150.0 MB",
             "book_count": 45,
             "oldest_file_age_days": 15,
             "cache_dir": "/data/cache",
             "cache_enabled": true
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
pub async fn get_pdf_cache_stats(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<PdfCacheStatsDto>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    // Check if cache is enabled
    if !state.pdf_page_cache.is_enabled() {
        return Ok(Json(PdfCacheStatsDto {
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
        }));
    }

    let stats = state
        .pdf_page_cache
        .get_total_stats()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get PDF cache stats: {}", e)))?;

    let mut dto = PdfCacheStatsDto::from(stats);
    dto.cache_enabled = true;

    Ok(Json(dto))
}

/// Trigger PDF cache cleanup task
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
    path = "/api/v1/admin/pdf-cache/cleanup",
    responses(
        (status = 200, description = "Cleanup task queued successfully", body = TriggerPdfCacheCleanupResponse,
         example = json!({
             "task_id": "550e8400-e29b-41d4-a716-446655440000",
             "message": "PDF cache cleanup task queued successfully",
             "max_age_days": 30
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

    // Get max age from settings for informational purposes
    let max_age_days = state
        .settings_service
        .get_uint("pdf_cache.max_age_days", 30)
        .await
        .unwrap_or(30) as u32;

    // Enqueue the cleanup task with low priority (cleanup runs last)
    let task_id = TaskRepository::enqueue(&state.db, TaskType::CleanupPdfCache, -100, None)
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

/// Clear the entire PDF cache immediately (synchronous)
///
/// Deletes all cached PDF pages immediately. This operation cannot be undone.
/// For selective cleanup based on age, use the trigger_pdf_cache_cleanup endpoint instead.
///
/// # Permission Required
/// - Admin access required
#[utoipa::path(
    delete,
    path = "/api/v1/admin/pdf-cache",
    responses(
        (status = 200, description = "Cache cleared successfully", body = PdfCacheCleanupResultDto,
         example = json!({
             "files_deleted": 1500,
             "bytes_reclaimed": 157286400,
             "bytes_reclaimed_human": "150.0 MB"
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

#[cfg(test)]
mod tests {
    #[test]
    fn test_placeholder() {
        // Integration tests are in tests/api/pdf_cache.rs
    }
}
