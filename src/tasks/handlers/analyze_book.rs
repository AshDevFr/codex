use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use tracing::{error, info};

use crate::db::entities::tasks;
use crate::scanner::analyze_book;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

pub struct AnalyzeBookHandler;

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

            match analyze_book(db, book_id, force).await {
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
