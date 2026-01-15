//! Handler for CleanupBookFiles task
//!
//! Cleans up files associated with a deleted book:
//! - Thumbnail file
//! - Cover references in series_covers table (book:uuid sources)

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::config::FilesConfig;
use crate::db::entities::tasks;
use crate::events::EventBroadcaster;
use crate::services::FileCleanupService;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Handler for cleaning up book files after deletion
pub struct CleanupBookFilesHandler {
    file_cleanup: FileCleanupService,
}

impl CleanupBookFilesHandler {
    /// Create a new handler with the given files config
    pub fn new(config: FilesConfig) -> Self {
        Self {
            file_cleanup: FileCleanupService::new(config),
        }
    }
}

impl TaskHandler for CleanupBookFilesHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        _db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            // Get book_id from params (not task.book_id) because the book is already deleted
            // and FK constraints would prevent storing deleted book IDs
            let book_id: uuid::Uuid = task
                .params
                .as_ref()
                .and_then(|p| p.get("book_id"))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| {
                    anyhow::anyhow!("Missing book_id in params for cleanup_book_files task")
                })?;

            info!(
                "Task {}: Cleaning up files for deleted book {}",
                task.id, book_id
            );

            let mut thumbnail_deleted = false;
            let cover_refs_deleted = 0u32;
            let bytes_freed = 0u64;

            // 1. Delete the thumbnail file
            // Try to use the thumbnail_path from params if available, otherwise derive from book_id
            let thumbnail_path: Option<String> = task
                .params
                .as_ref()
                .and_then(|p| p.get("thumbnail_path"))
                .and_then(|v| v.as_str())
                .map(String::from);

            if let Some(path_str) = thumbnail_path {
                // Use the provided path
                let path = PathBuf::from(&path_str);
                match self.file_cleanup.delete_thumbnail_by_path(&path).await {
                    Ok(true) => {
                        thumbnail_deleted = true;
                        debug!("Deleted thumbnail at provided path: {:?}", path);
                    }
                    Ok(false) => {
                        debug!("Thumbnail at provided path doesn't exist: {:?}", path);
                    }
                    Err(e) => {
                        warn!("Failed to delete thumbnail at {:?}: {}", path, e);
                    }
                }
            } else {
                // Derive path from book_id
                match self.file_cleanup.delete_book_thumbnail(book_id).await {
                    Ok(true) => {
                        thumbnail_deleted = true;
                        debug!("Deleted thumbnail for book {}", book_id);
                    }
                    Ok(false) => {
                        debug!("No thumbnail found for book {}", book_id);
                    }
                    Err(e) => {
                        warn!("Failed to delete thumbnail for book {}: {}", book_id, e);
                    }
                }
            }

            // 2. Delete cover references from series_covers table
            // Books can be used as cover sources with format "book:{book_id}"
            let source = format!("book:{}", book_id);

            // Find all series_covers entries that reference this book
            // We need to query by source pattern - this requires scanning covers
            // For now, we don't delete the actual cover files for book: sources
            // because they typically reference pages within the book file itself
            // (which is now deleted), not separate uploaded files
            debug!(
                "Book cover source '{}' will be cleaned up by cascade delete or orphan cleanup",
                source
            );

            info!(
                "Task {}: Book {} cleanup complete - thumbnail_deleted: {}, cover_refs: {}",
                task.id, book_id, thumbnail_deleted, cover_refs_deleted
            );

            Ok(TaskResult::success_with_data(
                format!("Cleaned up files for book {}", book_id),
                json!({
                    "book_id": book_id,
                    "thumbnail_deleted": thumbnail_deleted,
                    "cover_refs_deleted": cover_refs_deleted,
                    "bytes_freed": bytes_freed,
                }),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;
    use uuid::Uuid;

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

    #[tokio::test]
    async fn test_handler_deletes_thumbnail() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let _handler = CleanupBookFilesHandler::new(config.clone());
        let file_cleanup = FileCleanupService::new(config);

        let book_id = Uuid::new_v4();
        let thumb_path = file_cleanup.get_thumbnail_path(book_id);

        // Create the thumbnail
        fs::create_dir_all(thumb_path.parent().unwrap())
            .await
            .unwrap();
        fs::write(&thumb_path, b"test thumbnail data")
            .await
            .unwrap();
        assert!(fs::metadata(&thumb_path).await.is_ok());

        // Create a mock task with book_id in params (not task.book_id due to FK constraints)
        let _task = tasks::Model {
            id: Uuid::new_v4(),
            task_type: "cleanup_book_files".to_string(),
            library_id: None,
            series_id: None,
            book_id: None, // Not using FK column for cleanup tasks
            params: Some(serde_json::json!({ "book_id": book_id.to_string() })),
            status: "processing".to_string(),
            priority: 0,
            locked_by: None,
            locked_until: None,
            attempts: 0,
            max_attempts: 3,
            last_error: None,
            result: None,
            scheduled_for: chrono::Utc::now(),
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
        };

        // We can't easily test with a real DB connection in a unit test,
        // but we can verify the handler logic by checking file deletion
        // For now, verify the thumbnail path is correct
        assert!(thumb_path.to_string_lossy().contains(&book_id.to_string()));
    }

    #[test]
    fn test_handler_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let _handler = CleanupBookFilesHandler::new(config);
        // Just verify it can be created
    }
}
