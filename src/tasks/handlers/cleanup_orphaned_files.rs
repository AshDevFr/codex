//! Handler for CleanupOrphanedFiles task
//!
//! Scans the filesystem for orphaned files and deletes them:
//! - Thumbnails without corresponding book records
//! - Covers without corresponding series records

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, info};

use crate::config::FilesConfig;
use crate::db::entities::tasks;
use crate::db::repositories::{BookRepository, SeriesRepository};
use crate::events::EventBroadcaster;
use crate::services::{CleanupStats, FileCleanupService, OrphanedFileType};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Handler for cleaning up orphaned files
pub struct CleanupOrphanedFilesHandler {
    file_cleanup: FileCleanupService,
}

impl CleanupOrphanedFilesHandler {
    /// Create a new handler with the given files config
    pub fn new(config: FilesConfig) -> Self {
        Self {
            file_cleanup: FileCleanupService::new(config),
        }
    }
}

impl TaskHandler for CleanupOrphanedFilesHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            info!("Task {}: Starting orphaned files cleanup", task.id);

            let mut total_stats = CleanupStats::new();

            // 1. Scan and clean orphaned thumbnails
            info!("Task {}: Scanning for orphaned thumbnails...", task.id);
            let thumbnail_stats = self.cleanup_orphaned_thumbnails(db).await?;
            info!(
                "Task {}: Found and deleted {} orphaned thumbnails ({} bytes)",
                task.id, thumbnail_stats.thumbnails_deleted, thumbnail_stats.bytes_freed
            );
            total_stats.merge(thumbnail_stats);

            // 2. Scan and clean orphaned covers
            info!("Task {}: Scanning for orphaned covers...", task.id);
            let cover_stats = self.cleanup_orphaned_covers(db).await?;
            info!(
                "Task {}: Found and deleted {} orphaned covers ({} bytes)",
                task.id, cover_stats.covers_deleted, cover_stats.bytes_freed
            );
            total_stats.merge(cover_stats);

            info!(
                "Task {}: Orphaned files cleanup complete - thumbnails: {}, covers: {}, bytes: {}, failures: {}",
                task.id,
                total_stats.thumbnails_deleted,
                total_stats.covers_deleted,
                total_stats.bytes_freed,
                total_stats.failures
            );

            Ok(TaskResult::success_with_data(
                format!(
                    "Cleaned up {} orphaned files ({} bytes freed)",
                    total_stats.thumbnails_deleted + total_stats.covers_deleted,
                    total_stats.bytes_freed
                ),
                json!({
                    "thumbnails_deleted": total_stats.thumbnails_deleted,
                    "covers_deleted": total_stats.covers_deleted,
                    "bytes_freed": total_stats.bytes_freed,
                    "failures": total_stats.failures,
                    "errors": total_stats.errors,
                }),
            ))
        })
    }
}

impl CleanupOrphanedFilesHandler {
    /// Scan thumbnails directory and delete files for books that no longer exist
    async fn cleanup_orphaned_thumbnails(&self, db: &DatabaseConnection) -> Result<CleanupStats> {
        let thumbnails = self.file_cleanup.scan_thumbnails().await?;

        // Batch query: get all existing book IDs in a single query
        let book_ids: Vec<_> = thumbnails.iter().map(|(_, id)| *id).collect();
        let existing_book_ids = BookRepository::get_existing_ids(db, &book_ids).await?;

        // Find orphaned thumbnails (O(1) lookup per file)
        let orphaned_paths: Vec<_> = thumbnails
            .into_iter()
            .filter(|(_, book_id)| {
                let is_orphaned = !existing_book_ids.contains(book_id);
                if is_orphaned {
                    debug!("Thumbnail for book {} is orphaned", book_id);
                }
                is_orphaned
            })
            .map(|(path, _)| path)
            .collect();

        let stats = self
            .file_cleanup
            .delete_files(orphaned_paths, OrphanedFileType::Thumbnail)
            .await;

        Ok(stats)
    }

    /// Scan covers directory and delete files for series that no longer exist
    async fn cleanup_orphaned_covers(&self, db: &DatabaseConnection) -> Result<CleanupStats> {
        let covers = self.file_cleanup.scan_covers().await?;

        // Batch query: get all existing series IDs in a single query
        let series_ids: Vec<_> = covers.iter().map(|(_, id)| *id).collect();
        let existing_series_ids = SeriesRepository::get_existing_ids(db, &series_ids).await?;

        // Find orphaned covers (O(1) lookup per file)
        let orphaned_paths: Vec<_> = covers
            .into_iter()
            .filter(|(_, series_id)| {
                let is_orphaned = !existing_series_ids.contains(series_id);
                if is_orphaned {
                    debug!("Cover for series {} is orphaned", series_id);
                }
                is_orphaned
            })
            .map(|(path, _)| path)
            .collect();

        let stats = self
            .file_cleanup
            .delete_files(orphaned_paths, OrphanedFileType::Cover)
            .await;

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(temp_dir: &TempDir) -> FilesConfig {
        FilesConfig {
            thumbnail_dir: temp_dir
                .path()
                .join("thumbnails")
                .to_string_lossy()
                .to_string(),
            uploads_dir: temp_dir
                .path()
                .join("uploads")
                .to_string_lossy()
                .to_string(),
            plugins_dir: temp_dir
                .path()
                .join("plugins")
                .to_string_lossy()
                .to_string(),
        }
    }

    #[test]
    fn test_handler_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let _handler = CleanupOrphanedFilesHandler::new(config);
        // Just verify it can be created
    }
}
