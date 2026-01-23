//! Handler for CleanupPdfCache task
//!
//! Cleans up old pages from the PDF page cache based on the configured
//! maximum age setting.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::info;

use crate::db::entities::tasks;
use crate::events::EventBroadcaster;
use crate::services::{PdfPageCache, SettingsService};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Handler for cleaning up old PDF cache pages
pub struct CleanupPdfCacheHandler {
    pdf_cache: Arc<PdfPageCache>,
    settings_service: Arc<SettingsService>,
}

impl CleanupPdfCacheHandler {
    /// Create a new handler with the given PDF cache and settings service
    pub fn new(pdf_cache: Arc<PdfPageCache>, settings_service: Arc<SettingsService>) -> Self {
        Self {
            pdf_cache,
            settings_service,
        }
    }
}

impl TaskHandler for CleanupPdfCacheHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        _db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            info!("Task {}: Starting PDF cache cleanup", task.id);

            // Get max age from settings (default 30 days)
            let max_age_days = self
                .settings_service
                .get_uint("pdf_cache.max_age_days", 30)
                .await
                .unwrap_or(30) as u32;

            info!(
                "Task {}: Cleaning up PDF pages older than {} days",
                task.id, max_age_days
            );

            // Get stats before cleanup
            let stats_before = self.pdf_cache.get_total_stats().await?;
            info!(
                "Task {}: PDF cache before cleanup: {} files, {}",
                task.id,
                stats_before.total_files,
                stats_before.total_size_human()
            );

            // Perform cleanup
            let result = self.pdf_cache.cleanup_old_pages(max_age_days).await?;

            // Get stats after cleanup
            let stats_after = self.pdf_cache.get_total_stats().await?;

            info!(
                "Task {}: PDF cache cleanup complete - deleted {} files, reclaimed {}",
                task.id,
                result.files_deleted,
                result.bytes_reclaimed_human()
            );
            info!(
                "Task {}: PDF cache after cleanup: {} files, {}",
                task.id,
                stats_after.total_files,
                stats_after.total_size_human()
            );

            Ok(TaskResult::success_with_data(
                format!(
                    "Cleaned up {} PDF cache files ({} reclaimed)",
                    result.files_deleted,
                    result.bytes_reclaimed_human()
                ),
                json!({
                    "files_deleted": result.files_deleted,
                    "bytes_reclaimed": result.bytes_reclaimed,
                    "bytes_reclaimed_human": result.bytes_reclaimed_human(),
                    "max_age_days": max_age_days,
                    "files_remaining": stats_after.total_files,
                    "bytes_remaining": stats_after.total_size_bytes,
                    "bytes_remaining_human": stats_after.total_size_human(),
                }),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_handler_creation() {
        // Note: Full integration tests are in tests/tasks/
        // This just verifies the handler can be instantiated
        let temp_dir = TempDir::new().unwrap();
        let pdf_cache = Arc::new(PdfPageCache::new(temp_dir.path(), true));

        // We need a settings service, but can't create one without a database
        // So this test is limited to just confirming the types align
        assert!(pdf_cache.is_enabled());
    }
}
