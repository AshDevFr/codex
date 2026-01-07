use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use tracing::{error, info};

use crate::db::entities::tasks;
use crate::scanner::{analyze_series_books, AnalyzerConfig};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

pub struct AnalyzeSeriesHandler;

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
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let series_id = task
                .series_id
                .ok_or_else(|| anyhow::anyhow!("Missing series_id"))?;

            // Extract concurrency from params (default: 4)
            let concurrency = task
                .params
                .as_ref()
                .and_then(|p| p.get("concurrency"))
                .and_then(|v| v.as_u64())
                .unwrap_or(4) as usize;

            info!(
                "Task {}: Analyzing series {} with concurrency {}",
                task.id, series_id, concurrency
            );

            let config = AnalyzerConfig {
                max_concurrent: concurrency,
            };

            match analyze_series_books(db, series_id, config, None).await {
                Ok(result) => {
                    info!(
                        "Task {}: Series analysis completed - {} books analyzed",
                        task.id, result.books_analyzed
                    );

                    Ok(TaskResult::success_with_data(
                        format!("Analyzed {} books in series", result.books_analyzed),
                        json!({
                            "books_analyzed": result.books_analyzed,
                            "errors": result.errors.len(),
                        }),
                    ))
                }
                Err(e) => {
                    error!("Task {}: Series analysis failed: {}", task.id, e);
                    Err(e)
                }
            }
        })
    }
}
