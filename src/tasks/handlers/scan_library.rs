use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use tracing::{debug, error, info, warn};

use crate::db::entities::tasks;
use crate::db::repositories::{BookRepository, LibraryRepository, TaskRepository};
use crate::scanner::{scan_library, ScanMode, ScanningConfig};
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
                    // Deep scans should force re-analysis even if file hash hasn't changed
                    let force = matches!(scan_mode, ScanMode::Deep);

                    for book in books_to_analyze {
                        match TaskRepository::enqueue(
                            db,
                            TaskType::AnalyzeBook {
                                book_id: book.id,
                                force,
                            },
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

                    // Check if purge_deleted_on_scan is enabled and purge deleted books
                    let purged_count = match LibraryRepository::get_by_id(db, library_id).await {
                        Ok(Some(library)) => {
                            if let Some(config_json) = &library.scanning_config {
                                match serde_json::from_str::<ScanningConfig>(config_json) {
                                    Ok(config) if config.purge_deleted_on_scan => {
                                        info!(
                                            "Task {}: purge_deleted_on_scan is enabled, purging deleted books from library {}",
                                            task.id, library_id
                                        );
                                        match BookRepository::purge_deleted_in_library(
                                            db, library_id,
                                        )
                                        .await
                                        {
                                            Ok(count) => {
                                                if count > 0 {
                                                    info!(
                                                        "Task {}: Purged {} deleted books from library {}",
                                                        task.id, count, library_id
                                                    );
                                                }
                                                count
                                            }
                                            Err(e) => {
                                                warn!(
                                                    "Task {}: Failed to purge deleted books from library {}: {}",
                                                    task.id, library_id, e
                                                );
                                                0
                                            }
                                        }
                                    }
                                    Ok(_) => {
                                        debug!(
                                            "Task {}: purge_deleted_on_scan is disabled",
                                            task.id
                                        );
                                        0
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Task {}: Failed to parse scanning_config for library {}: {}",
                                            task.id, library_id, e
                                        );
                                        0
                                    }
                                }
                            } else {
                                0
                            }
                        }
                        Ok(None) => {
                            warn!(
                                "Task {}: Library {} not found for purge check",
                                task.id, library_id
                            );
                            0
                        }
                        Err(e) => {
                            warn!(
                                "Task {}: Failed to load library {} for purge check: {}",
                                task.id, library_id, e
                            );
                            0
                        }
                    };

                    Ok(TaskResult::success_with_data(
                        format!(
                            "Scanned {} files ({} series, {} books), queued {} analysis tasks{}",
                            result.files_processed,
                            result.series_created,
                            result.books_created,
                            tasks_queued,
                            if purged_count > 0 {
                                format!(", purged {} deleted books", purged_count)
                            } else {
                                String::new()
                            }
                        ),
                        json!({
                            "files_processed": result.files_processed,
                            "series_created": result.series_created,
                            "books_created": result.books_created,
                            "books_updated": result.books_updated,
                            "books_deleted": result.books_deleted,
                            "books_restored": result.books_restored,
                            "tasks_queued": tasks_queued,
                            "books_purged": purged_count,
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
