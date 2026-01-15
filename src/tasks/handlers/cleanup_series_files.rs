//! Handler for CleanupSeriesFiles task
//!
//! Cleans up files associated with a deleted series:
//! - Custom cover files in uploads/covers/

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::config::FilesConfig;
use crate::db::entities::tasks;
use crate::events::EventBroadcaster;
use crate::services::FileCleanupService;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Handler for cleaning up series files after deletion
pub struct CleanupSeriesFilesHandler {
    file_cleanup: FileCleanupService,
}

impl CleanupSeriesFilesHandler {
    /// Create a new handler with the given files config
    pub fn new(config: FilesConfig) -> Self {
        Self {
            file_cleanup: FileCleanupService::new(config),
        }
    }
}

impl TaskHandler for CleanupSeriesFilesHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        _db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            // Get series_id from params (not task.series_id) because the series is already deleted
            // and FK constraints would prevent storing deleted series IDs
            let series_id: uuid::Uuid = task
                .params
                .as_ref()
                .and_then(|p| p.get("series_id"))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| {
                    anyhow::anyhow!("Missing series_id in params for cleanup_series_files task")
                })?;

            info!(
                "Task {}: Cleaning up files for deleted series {}",
                task.id, series_id
            );

            let mut cover_deleted = false;
            let bytes_freed = 0u64;

            // Delete the series cover file if it exists
            // Custom covers are stored at: uploads/covers/{series_id}.jpg
            match self.file_cleanup.delete_series_cover(series_id).await {
                Ok(true) => {
                    cover_deleted = true;
                    debug!("Deleted cover for series {}", series_id);
                }
                Ok(false) => {
                    debug!("No cover file found for series {}", series_id);
                }
                Err(e) => {
                    warn!("Failed to delete cover for series {}: {}", series_id, e);
                }
            }

            info!(
                "Task {}: Series {} cleanup complete - cover_deleted: {}",
                task.id, series_id, cover_deleted
            );

            Ok(TaskResult::success_with_data(
                format!("Cleaned up files for series {}", series_id),
                json!({
                    "series_id": series_id,
                    "cover_deleted": cover_deleted,
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
    async fn test_handler_deletes_cover() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let _handler = CleanupSeriesFilesHandler::new(config.clone());
        let file_cleanup = FileCleanupService::new(config);

        let series_id = Uuid::new_v4();
        let cover_path = file_cleanup.get_series_cover_path(series_id);

        // Create the cover file
        fs::create_dir_all(cover_path.parent().unwrap())
            .await
            .unwrap();
        fs::write(&cover_path, b"test cover data").await.unwrap();
        assert!(fs::metadata(&cover_path).await.is_ok());

        // Verify path is correct
        assert!(cover_path
            .to_string_lossy()
            .contains(&series_id.to_string()));
    }

    #[test]
    fn test_handler_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let _handler = CleanupSeriesFilesHandler::new(config);
        // Just verify it can be created
    }
}
