use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use tracing::{error, info};

use crate::db::entities::tasks;
use crate::db::repositories::{BookRepository, TaskRepository};
use crate::scanner::{scan_library, ScanMode};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::{TaskResult, TaskType};

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

                    // Auto-queue analysis tasks after successful scan
                    let books_to_analyze = match scan_mode {
                        ScanMode::Normal => {
                            // Normal scan: only analyze unanalyzed books
                            BookRepository::get_unanalyzed_in_library(db, library_id).await?
                        }
                        ScanMode::Deep => {
                            // Deep scan: analyze ALL books (get via series)
                            use crate::db::entities::{books, prelude::*, series};
                            use sea_orm::{
                                ColumnTrait, EntityTrait, JoinType, QueryFilter, QuerySelect,
                                RelationTrait,
                            };

                            Books::find()
                                .join(JoinType::InnerJoin, books::Relation::Series.def())
                                .filter(series::Column::LibraryId.eq(library_id))
                                .filter(books::Column::Deleted.eq(false))
                                .all(db)
                                .await?
                        }
                    };

                    let mut tasks_queued = 0;
                    for book in books_to_analyze {
                        match TaskRepository::enqueue(
                            db,
                            TaskType::AnalyzeBook { book_id: book.id },
                            0, // Priority 0 for auto-queued tasks
                            None,
                        )
                        .await
                        {
                            Ok(_) => tasks_queued += 1,
                            Err(e) => {
                                error!(
                                    "Task {}: Failed to queue analysis for book {}: {}",
                                    task.id, book.id, e
                                );
                            }
                        }
                    }

                    info!(
                        "Task {}: Queued {} analysis tasks for library {}",
                        task.id, tasks_queued, library_id
                    );

                    Ok(TaskResult::success_with_data(
                        format!(
                            "Scanned {} files ({} series, {} books), queued {} analysis tasks",
                            result.files_processed,
                            result.series_created,
                            result.books_created,
                            tasks_queued
                        ),
                        json!({
                            "files_processed": result.files_processed,
                            "series_created": result.series_created,
                            "books_created": result.books_created,
                            "books_updated": result.books_updated,
                            "books_deleted": result.books_deleted,
                            "books_restored": result.books_restored,
                            "tasks_queued": tasks_queued,
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
