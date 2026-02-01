use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::db::entities::tasks;
use crate::db::repositories::{BookRepository, TaskRepository};
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
            info!(
                "Task {}: Starting batch thumbnail generation (fan-out)",
                task.id
            );

            // Extract parameters from task
            let library_id = task.library_id;
            let series_id = task.series_id;
            let params = task.params.as_ref();
            let force = params
                .and_then(|p| p.get("force"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let book_ids: Option<Vec<uuid::Uuid>> = params
                .and_then(|p| p.get("book_ids"))
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            let series_ids: Option<Vec<uuid::Uuid>> = params
                .and_then(|p| p.get("series_ids"))
                .and_then(|v| serde_json::from_value(v.clone()).ok());

            // Determine scope and get books
            // Priority: book_ids > series_ids > series_id > library_id > all
            let books = if let Some(ids) = &book_ids {
                // Explicit book IDs take precedence
                info!(
                    "Generating thumbnails for {} specific books (force={})",
                    ids.len(),
                    force
                );
                BookRepository::get_by_ids(db, ids).await?
            } else if let Some(ids) = &series_ids {
                // Explicit series IDs take precedence over single series_id
                info!(
                    "Generating thumbnails for books in {} specific series (force={})",
                    ids.len(),
                    force
                );
                BookRepository::list_by_series_ids(db, ids).await?
            } else if let Some(ser_id) = series_id {
                // Single series scope
                info!(
                    "Generating thumbnails for series {} (force={})",
                    ser_id, force
                );
                BookRepository::list_by_series(db, ser_id, false).await?
            } else if let Some(lib_id) = library_id {
                // Library scope
                info!(
                    "Generating thumbnails for library {} (force={})",
                    lib_id, force
                );
                let (books, _total) =
                    BookRepository::list_by_library(db, lib_id, false, 0, 1000000).await?;
                books
            } else {
                // All libraries
                info!("Generating thumbnails for all books (force={})", force);
                let (books, _total) = BookRepository::list_all(db, false, 0, 1000000).await?;
                books
            };

            let total = books.len();
            info!("Found {} books to process", total);

            // Filter books if not forcing - only include books without thumbnails
            let books_to_process: Vec<_> = if force {
                books
            } else {
                let mut filtered = Vec::new();
                for book in books {
                    if !self.thumbnail_service.thumbnail_exists(book.id).await {
                        filtered.push(book);
                    }
                }
                filtered
            };

            let to_process = books_to_process.len();
            let skipped = total - to_process;

            if skipped > 0 {
                info!("Skipping {} books that already have thumbnails", skipped);
            }

            if to_process == 0 {
                info!("No books need thumbnail generation");
                return Ok(TaskResult::success_with_data(
                    "No books need thumbnail generation".to_string(),
                    serde_json::json!({
                        "total": total,
                        "enqueued": 0,
                        "skipped": skipped,
                    }),
                ));
            }

            // Enqueue individual GenerateThumbnail tasks for each book
            let mut enqueued = 0;
            let mut errors = Vec::new();

            for book in books_to_process {
                let task_type = TaskType::GenerateThumbnail {
                    book_id: book.id,
                    force,
                };

                match TaskRepository::enqueue(db, task_type, 0, None).await {
                    Ok(task_id) => {
                        debug!(
                            "Enqueued thumbnail task {} for book {} (force={})",
                            task_id, book.id, force
                        );
                        enqueued += 1;
                    }
                    Err(e) => {
                        let error_msg = format!(
                            "Failed to enqueue thumbnail task for book {}: {}",
                            book.id, e
                        );
                        warn!("{}", error_msg);
                        errors.push(error_msg);
                    }
                }
            }

            info!(
                "Batch thumbnail generation complete: enqueued {} tasks ({} skipped, {} errors)",
                enqueued,
                skipped,
                errors.len()
            );

            Ok(TaskResult::success_with_data(
                format!(
                    "Enqueued {} thumbnail tasks ({} skipped)",
                    enqueued, skipped
                ),
                serde_json::json!({
                    "total": total,
                    "enqueued": enqueued,
                    "skipped": skipped,
                    "errors": errors,
                }),
            ))
        })
    }
}
