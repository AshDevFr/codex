use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::scanner::analyze_book;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;
use codex_db::entities::tasks;
use codex_db::repositories::BookRepository;
use codex_events::{EventBroadcaster, TaskProgressEvent};

pub struct AnalyzeBookHandler;

impl Default for AnalyzeBookHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyzeBookHandler {
    pub fn new() -> Self {
        Self
    }
}

impl TaskHandler for AnalyzeBookHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let book_id = task
                .book_id
                .ok_or_else(|| anyhow::anyhow!("Missing book_id"))?;

            // Extract force parameter from task params (default: false)
            let force = task
                .params
                .as_ref()
                .and_then(|p| p.get("force"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            info!(
                "Task {}: Analyzing book {} (force={})",
                task.id, book_id, force
            );

            // Emit a progress event so the Active Tasks UI shows which book is
            // being processed instead of a generic "Processing..." label.
            // analyze_book has no inner loop to report from, so without this
            // emit the only events for this task are `started` (no message)
            // and `completed`, leaving the UI blank for the whole run.
            if let Some(broadcaster) = event_broadcaster {
                match BookRepository::get_by_id(db, book_id).await {
                    Ok(Some(book)) => {
                        let _ = broadcaster.emit_task(TaskProgressEvent::progress(
                            task.id,
                            &task.task_type,
                            0,
                            1,
                            Some(format!("Analyzing {}", book.file_name)),
                            task.library_id,
                            task.series_id,
                            task.book_id,
                        ));
                    }
                    Ok(None) => {
                        warn!(
                            "Task {}: Book {} not found for progress label",
                            task.id, book_id
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Task {}: Failed to load book {} for progress label: {}",
                            task.id, book_id, e
                        );
                    }
                }
            }

            match analyze_book(db, book_id, force, event_broadcaster).await {
                Ok(result) => {
                    info!(
                        "Task {}: Book analysis completed - {} books analyzed",
                        task.id, result.books_analyzed
                    );

                    Ok(TaskResult::success_with_data(
                        format!("Analyzed {} book(s)", result.books_analyzed),
                        json!({
                            "books_analyzed": result.books_analyzed,
                            "errors": result.errors.len(),
                        }),
                    ))
                }
                Err(e) => {
                    error!("Task {}: Book analysis failed: {}", task.id, e);
                    Err(e)
                }
            }
        })
    }
}
