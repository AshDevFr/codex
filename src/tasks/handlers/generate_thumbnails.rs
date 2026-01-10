use anyhow::{anyhow, Result};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{info, warn};

use crate::db::entities::tasks;
use crate::db::repositories::BookRepository;
use crate::events::EventBroadcaster;
use crate::services::ThumbnailService;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::{TaskResult, TaskType};

pub struct GenerateThumbnailsHandler {
    thumbnail_service: Arc<ThumbnailService>,
}

impl GenerateThumbnailsHandler {
    pub fn new(thumbnail_service: Arc<ThumbnailService>) -> Self {
        Self { thumbnail_service }
    }
}

impl TaskHandler for GenerateThumbnailsHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            info!("Task {}: Starting thumbnail generation", task.id);

            // Parse task parameters
            let params: TaskType = if let Some(params_value) = &task.params {
                serde_json::from_value(params_value.clone())
                    .map_err(|e| anyhow!("Failed to parse task params: {}", e))?
            } else {
                return Err(anyhow!("Missing task params"));
            };

            let library_id = match params {
                TaskType::GenerateThumbnails { library_id } => library_id,
                _ => {
                    return Err(anyhow!(
                        "Invalid task type for GenerateThumbnails: expected GenerateThumbnails, got {:?}",
                        params
                    ));
                }
            };

            // Get books to process
            let books = if let Some(lib_id) = library_id {
                info!("Generating thumbnails for library {}", lib_id);
                // Get non-deleted books from the library
                let (books, _total) =
                    BookRepository::list_by_library(db, lib_id, false, 0, 1000000).await?;
                books
            } else {
                info!("Generating thumbnails for all books");
                // Get all non-deleted books
                let (books, _total) = BookRepository::list_all(db, false, 0, 1000000).await?;
                books
            };

            let book_ids: Vec<_> = books.iter().map(|b| b.id).collect();
            let total = book_ids.len();
            info!("Found {} books to process", total);

            // Generate thumbnails in batch
            let stats = self
                .thumbnail_service
                .generate_thumbnails_batch(db, book_ids)
                .await?;

            info!(
                "Thumbnail generation complete: {}/{} generated, {} skipped, {} failed",
                stats.generated, stats.total, stats.skipped, stats.failed
            );

            if stats.failed > 0 {
                warn!(
                    "Some thumbnails failed to generate. Errors: {:?}",
                    stats.errors
                );
            }

            Ok(TaskResult::success_with_data(
                format!(
                    "Generated {}/{} thumbnails ({} skipped, {} failed)",
                    stats.generated, stats.total, stats.skipped, stats.failed
                ),
                serde_json::json!({
                    "total": stats.total,
                    "generated": stats.generated,
                    "skipped": stats.skipped,
                    "failed": stats.failed,
                    "errors": stats.errors,
                }),
            ))
        })
    }
}
