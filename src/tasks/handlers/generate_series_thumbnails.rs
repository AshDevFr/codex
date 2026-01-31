//! Handler for GenerateSeriesThumbnails task (fan-out)
//!
//! Generates thumbnails for all series in a scope (library or all).
//! This is a fan-out task that enqueues individual GenerateSeriesThumbnail tasks
//! for each series that needs a thumbnail.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::db::entities::tasks;
use crate::db::repositories::{SeriesRepository, TaskRepository};
use crate::events::EventBroadcaster;
use crate::services::ThumbnailService;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::{TaskResult, TaskType};

pub struct GenerateSeriesThumbnailsHandler {
    thumbnail_service: Arc<ThumbnailService>,
}

impl GenerateSeriesThumbnailsHandler {
    pub fn new(thumbnail_service: Arc<ThumbnailService>) -> Self {
        Self { thumbnail_service }
    }
}

impl TaskHandler for GenerateSeriesThumbnailsHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            info!(
                "Task {}: Starting batch series thumbnail generation (fan-out)",
                task.id
            );

            // Extract parameters from task
            let library_id = task.library_id;
            let force = task
                .params
                .as_ref()
                .and_then(|p| p.get("force"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // Get series based on scope
            let series_list = if let Some(lib_id) = library_id {
                info!(
                    "Generating series thumbnails for library {} (force={})",
                    lib_id, force
                );
                SeriesRepository::list_by_library(db, lib_id).await?
            } else {
                info!(
                    "Generating series thumbnails for all series (force={})",
                    force
                );
                SeriesRepository::list_all(db).await?
            };

            let total = series_list.len();
            info!("Found {} series to process", total);

            // Filter series if not forcing - only include series without thumbnails
            let series_to_process: Vec<_> = if force {
                series_list
            } else {
                let mut filtered = Vec::new();
                for series in series_list {
                    if self
                        .thumbnail_service
                        .get_series_thumbnail_metadata(series.id)
                        .await
                        .is_none()
                    {
                        filtered.push(series);
                    }
                }
                filtered
            };

            let to_process = series_to_process.len();
            let skipped = total - to_process;

            if skipped > 0 {
                info!("Skipping {} series that already have thumbnails", skipped);
            }

            if to_process == 0 {
                info!("No series need thumbnail generation");
                return Ok(TaskResult::success_with_data(
                    "No series need thumbnail generation".to_string(),
                    serde_json::json!({
                        "total": total,
                        "enqueued": 0,
                        "skipped": skipped,
                    }),
                ));
            }

            // Enqueue individual GenerateSeriesThumbnail tasks for each series
            let mut enqueued = 0;
            let mut errors = Vec::new();

            for series in series_to_process {
                let task_type = TaskType::GenerateSeriesThumbnail {
                    series_id: series.id,
                    force,
                };

                match TaskRepository::enqueue(db, task_type, 0, None).await {
                    Ok(task_id) => {
                        debug!(
                            "Enqueued series thumbnail task {} for series {} (force={})",
                            task_id, series.id, force
                        );
                        enqueued += 1;
                    }
                    Err(e) => {
                        let error_msg = format!(
                            "Failed to enqueue series thumbnail task for series {}: {}",
                            series.id, e
                        );
                        warn!("{}", error_msg);
                        errors.push(error_msg);
                    }
                }
            }

            info!(
                "Batch series thumbnail generation complete: enqueued {} tasks ({} skipped, {} errors)",
                enqueued,
                skipped,
                errors.len()
            );

            Ok(TaskResult::success_with_data(
                format!(
                    "Enqueued {} series thumbnail tasks ({} skipped)",
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
