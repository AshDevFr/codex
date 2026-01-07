use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use tracing::{error, info};

use crate::db::entities::tasks;
use crate::scanner::{scan_library, ScanMode};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

pub struct ScanLibraryHandler;

impl ScanLibraryHandler {
    pub fn new() -> Self {
        Self
    }
}

impl TaskHandler for ScanLibraryHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let library_id = task
                .library_id
                .ok_or_else(|| anyhow::anyhow!("Missing library_id"))?;

            // Extract mode from params
            let mode_str = task
                .params
                .as_ref()
                .and_then(|p| p.get("mode"))
                .and_then(|v| v.as_str())
                .unwrap_or("normal");

            let scan_mode = match mode_str {
                "deep" => ScanMode::Deep,
                _ => ScanMode::Normal,
            };

            info!(
                "Task {}: Scanning library {} in {} mode",
                task.id, library_id, scan_mode
            );

            // Execute scan (without progress channel for now)
            match scan_library(db, library_id, scan_mode, None).await {
                Ok(result) => {
                    info!(
                        "Task {}: Library scan completed - {} files processed, {} series, {} books",
                        task.id,
                        result.files_processed,
                        result.series_created,
                        result.books_created
                    );

                    Ok(TaskResult::success_with_data(
                        format!(
                            "Scanned {} files ({} series, {} books)",
                            result.files_processed, result.series_created, result.books_created
                        ),
                        json!({
                            "files_processed": result.files_processed,
                            "series_created": result.series_created,
                            "books_created": result.books_created,
                            "books_updated": result.books_updated,
                            "books_deleted": result.books_deleted,
                            "books_restored": result.books_restored,
                            "errors": result.errors.len(),
                        }),
                    ))
                }
                Err(e) => {
                    error!("Task {}: Library scan failed: {}", task.id, e);
                    Err(e)
                }
            }
        })
    }
}
