use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info};

use crate::db::entities::tasks;
use crate::db::repositories::{BookRepository, TaskRepository};
use crate::events::{EventBroadcaster, TaskProgressEvent};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::{TaskResult, TaskType};

pub struct AnalyzeSeriesHandler;

impl Default for AnalyzeSeriesHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyzeSeriesHandler {
    pub fn new() -> Self {
        Self
    }
}

impl TaskHandler for AnalyzeSeriesHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let series_id = task
                .series_id
                .ok_or_else(|| anyhow::anyhow!("Missing series_id"))?;

            info!(
                "Task {}: Enqueuing forced analysis for all books in series {}",
                task.id, series_id
            );

            // Get all books in series from database
            // Note: This queries DB, not filesystem (unlike deep scan)
            let books = BookRepository::list_by_series(db, series_id, false).await?;

            let total_books = books.len();
            if total_books == 0 {
                return Ok(TaskResult::success_with_data(
                    "No books found in series",
                    json!({ "tasks_enqueued": 0 }),
                ));
            }

            if let Some(broadcaster) = event_broadcaster {
                let _ = broadcaster.emit_task(TaskProgressEvent::progress(
                    task.id,
                    "analyze_series",
                    0,
                    total_books,
                    Some(format!("Enqueueing analysis for {} book(s)", total_books)),
                    task.library_id,
                    Some(series_id),
                    None,
                ));
            }

            // Enqueue individual AnalyzeBook tasks with force=true
            let mut enqueued = 0;
            let mut errors = Vec::new();

            for (idx, book) in books.iter().enumerate() {
                match TaskRepository::enqueue(
                    db,
                    TaskType::AnalyzeBook {
                        book_id: book.id,
                        force: true, // ALWAYS force for series analysis
                    },
                    None, // schedule now
                )
                .await
                {
                    Ok(_) => enqueued += 1,
                    Err(e) => {
                        let err_msg = format!("Failed to enqueue task for book {}: {}", book.id, e);
                        error!("{}", err_msg);
                        errors.push(err_msg);
                    }
                }

                if let Some(broadcaster) = event_broadcaster {
                    let current = idx + 1;
                    let _ = broadcaster.emit_task(TaskProgressEvent::progress(
                        task.id,
                        "analyze_series",
                        current,
                        total_books,
                        Some(format!(
                            "Enqueueing analysis ({}/{}, {} failed)",
                            current,
                            total_books,
                            errors.len()
                        )),
                        task.library_id,
                        Some(series_id),
                        Some(book.id),
                    ));
                }
            }

            info!(
                "Task {}: Enqueued {} of {} forced book analysis tasks for series",
                task.id, enqueued, total_books
            );

            Ok(TaskResult::success_with_data(
                format!("Enqueued {} book analysis tasks (force=true)", enqueued),
                json!({
                    "tasks_enqueued": enqueued,
                    "total_books": total_books,
                    "errors": errors.len(),
                }),
            ))
        })
    }
}
