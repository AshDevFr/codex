//! API handlers for file cleanup operations
//!
//! These endpoints allow administrators to scan for and clean up orphaned files
//! (thumbnails and covers that no longer have corresponding database entries).

use axum::{
    extract::{Query, State},
    Json,
};
use std::sync::Arc;

use crate::api::{
    dto::{
        CleanupResultDto, OrphanStatsDto, OrphanStatsQuery, OrphanedFileDto, TriggerCleanupResponse,
    },
    error::ApiError,
    extractors::{AppState, AuthContext},
    permissions::Permission,
};
use crate::db::repositories::{BookRepository, SeriesRepository, TaskRepository};
use crate::require_permission;
use crate::services::file_cleanup::OrphanedFileType;
use crate::tasks::types::TaskType;

/// Get statistics about orphaned files
///
/// Scans the thumbnail and cover directories for files that don't have
/// corresponding database entries. This is a read-only operation.
///
/// # Permission Required
/// - Admin access required
///
/// # Query Parameters
/// - `include_files`: If true, includes the list of orphaned files in the response
#[utoipa::path(
    get,
    path = "/api/v1/admin/cleanup-orphans/stats",
    params(
        ("include_files" = Option<bool>, Query, description = "Include list of orphaned files in response")
    ),
    responses(
        (status = 200, description = "Orphan statistics retrieved successfully", body = OrphanStatsDto,
         example = json!({
             "orphaned_thumbnails": 42,
             "orphaned_covers": 5,
             "total_size_bytes": 1073741824
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
pub async fn get_orphan_stats(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Query(query): Query<OrphanStatsQuery>,
) -> Result<Json<OrphanStatsDto>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let cleanup_service = &state.file_cleanup_service;

    // Scan thumbnails and covers from filesystem
    let thumbnails = cleanup_service
        .scan_thumbnails()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to scan thumbnails: {}", e)))?;

    let covers = cleanup_service
        .scan_covers()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to scan covers: {}", e)))?;

    // Batch query: get all existing book IDs in a single query
    let book_ids: Vec<_> = thumbnails.iter().map(|(_, id)| *id).collect();
    let existing_book_ids = BookRepository::get_existing_ids(&state.db, &book_ids)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check book existence: {}", e)))?;

    // Batch query: get all existing series IDs in a single query
    let series_ids: Vec<_> = covers.iter().map(|(_, id)| *id).collect();
    let existing_series_ids = SeriesRepository::get_existing_ids(&state.db, &series_ids)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check series existence: {}", e)))?;

    // Check which files are orphaned (no DB record)
    let mut orphaned_thumbnails = 0u32;
    let mut orphaned_covers = 0u32;
    let mut total_size_bytes = 0u64;
    let mut orphaned_files = Vec::new();

    // Check thumbnails against existing IDs (O(1) lookup per file)
    for (path, book_id) in &thumbnails {
        if !existing_book_ids.contains(book_id) {
            orphaned_thumbnails += 1;
            let size = cleanup_service.get_file_size(path).await;
            total_size_bytes += size;

            if query.include_files {
                orphaned_files.push(OrphanedFileDto {
                    path: path.to_string_lossy().to_string(),
                    entity_id: Some(*book_id),
                    size_bytes: size,
                    file_type: "thumbnail".to_string(),
                });
            }
        }
    }

    // Check covers against existing IDs (O(1) lookup per file)
    for (path, series_id) in &covers {
        if !existing_series_ids.contains(series_id) {
            orphaned_covers += 1;
            let size = cleanup_service.get_file_size(path).await;
            total_size_bytes += size;

            if query.include_files {
                orphaned_files.push(OrphanedFileDto {
                    path: path.to_string_lossy().to_string(),
                    entity_id: Some(*series_id),
                    size_bytes: size,
                    file_type: "cover".to_string(),
                });
            }
        }
    }

    Ok(Json(OrphanStatsDto {
        orphaned_thumbnails,
        orphaned_covers,
        total_size_bytes,
        files: if query.include_files {
            Some(orphaned_files)
        } else {
            None
        },
    }))
}

/// Trigger orphan cleanup task
///
/// Enqueues a background task to scan and delete orphaned files
/// (thumbnails and covers without database entries).
///
/// # Permission Required
/// - Admin access required
///
/// Returns the task ID which can be used to track progress.
#[utoipa::path(
    post,
    path = "/api/v1/admin/cleanup-orphans",
    responses(
        (status = 200, description = "Cleanup task queued successfully", body = TriggerCleanupResponse,
         example = json!({
             "task_id": "550e8400-e29b-41d4-a716-446655440000",
             "message": "Cleanup task queued successfully"
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
pub async fn trigger_cleanup(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<TriggerCleanupResponse>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    // Enqueue the cleanup task with low priority (cleanup runs last)
    let task_id = TaskRepository::enqueue(&state.db, TaskType::CleanupOrphanedFiles, -100, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to enqueue cleanup task: {}", e)))?;

    Ok(Json(TriggerCleanupResponse {
        task_id,
        message: "Cleanup task queued successfully".to_string(),
    }))
}

/// Delete orphaned files immediately (synchronous)
///
/// Scans for and deletes orphaned files immediately, returning
/// the results. For large numbers of files, prefer using the
/// async trigger_cleanup endpoint instead.
///
/// # Permission Required
/// - Admin access required
#[utoipa::path(
    delete,
    path = "/api/v1/admin/cleanup-orphans",
    responses(
        (status = 200, description = "Cleanup completed successfully", body = CleanupResultDto,
         example = json!({
             "thumbnails_deleted": 42,
             "covers_deleted": 5,
             "bytes_freed": 1073741824,
             "failures": 0
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
pub async fn delete_orphans(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<CleanupResultDto>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let cleanup_service = &state.file_cleanup_service;

    // Scan thumbnails and covers from filesystem
    let thumbnails = cleanup_service
        .scan_thumbnails()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to scan thumbnails: {}", e)))?;

    let covers = cleanup_service
        .scan_covers()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to scan covers: {}", e)))?;

    // Batch query: get all existing book IDs in a single query
    let book_ids: Vec<_> = thumbnails.iter().map(|(_, id)| *id).collect();
    let existing_book_ids = BookRepository::get_existing_ids(&state.db, &book_ids)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check book existence: {}", e)))?;

    // Batch query: get all existing series IDs in a single query
    let series_ids: Vec<_> = covers.iter().map(|(_, id)| *id).collect();
    let existing_series_ids = SeriesRepository::get_existing_ids(&state.db, &series_ids)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check series existence: {}", e)))?;

    // Find orphaned thumbnails (O(1) lookup per file)
    let orphaned_thumbnail_paths: Vec<_> = thumbnails
        .into_iter()
        .filter(|(_, book_id)| !existing_book_ids.contains(book_id))
        .map(|(path, _)| path)
        .collect();

    // Find orphaned covers (O(1) lookup per file)
    let orphaned_cover_paths: Vec<_> = covers
        .into_iter()
        .filter(|(_, series_id)| !existing_series_ids.contains(series_id))
        .map(|(path, _)| path)
        .collect();

    // Delete orphaned files
    let mut stats = cleanup_service
        .delete_files(orphaned_thumbnail_paths, OrphanedFileType::Thumbnail)
        .await;

    let cover_stats = cleanup_service
        .delete_files(orphaned_cover_paths, OrphanedFileType::Cover)
        .await;

    stats.merge(cover_stats);

    Ok(Json(stats.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orphan_stats_query_default() {
        let query: OrphanStatsQuery = serde_json::from_str("{}").unwrap();
        assert!(!query.include_files);
    }

    #[test]
    fn test_orphan_stats_query_with_files() {
        let query: OrphanStatsQuery = serde_json::from_str(r#"{"include_files": true}"#).unwrap();
        assert!(query.include_files);
    }
}
