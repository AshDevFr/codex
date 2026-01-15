//! Handler for CleanupOrphanedFiles task
//!
//! Scans the filesystem for orphaned files and deletes them:
//! - Thumbnails without corresponding book records
//! - Covers without corresponding series records

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, info, warn};

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
        let mut orphaned_paths = Vec::new();

        for (path, book_id) in thumbnails {
            // Check if the book exists in the database
            match BookRepository::get_by_id(db, book_id).await {
                Ok(Some(_)) => {
                    // Book exists, keep the thumbnail
                    debug!("Thumbnail for book {} has valid owner", book_id);
                }
                Ok(None) => {
                    // Book doesn't exist, mark for deletion
                    debug!("Thumbnail for book {} is orphaned", book_id);
                    orphaned_paths.push(path);
                }
                Err(e) => {
                    // Error checking, skip this file
                    warn!("Error checking book {} existence: {}", book_id, e);
                }
            }
        }

        let stats = self
            .file_cleanup
            .delete_files(orphaned_paths, OrphanedFileType::Thumbnail)
            .await;

        Ok(stats)
    }

    /// Scan covers directory and delete files for series that no longer exist
    async fn cleanup_orphaned_covers(&self, db: &DatabaseConnection) -> Result<CleanupStats> {
        let covers = self.file_cleanup.scan_covers().await?;
        let mut orphaned_paths = Vec::new();

        for (path, series_id) in covers {
            // Check if the series exists in the database
            match SeriesRepository::get_by_id(db, series_id).await {
                Ok(Some(_)) => {
                    // Series exists, keep the cover
                    debug!("Cover for series {} has valid owner", series_id);
                }
                Ok(None) => {
                    // Series doesn't exist, mark for deletion
                    debug!("Cover for series {} is orphaned", series_id);
                    orphaned_paths.push(path);
                }
                Err(e) => {
                    // Error checking, skip this file
                    warn!("Error checking series {} existence: {}", series_id, e);
                }
            }
        }

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
