//! Handlers for RenumberSeries and RenumberSeriesBatch tasks
//!
//! Renumbers books in a series using the library's number strategy.
//! The single handler calls the existing `renumber_series_books()` function,
//! while the batch handler fans out into individual RenumberSeries tasks.

use anyhow::{Result, anyhow};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::entities::tasks;
use crate::db::repositories::{SeriesRepository, TaskRepository};
use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::{TaskResult, TaskType};

// =============================================================================
// RenumberSeries Handler (Single Series)
// =============================================================================

pub struct RenumberSeriesHandler;

impl RenumberSeriesHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RenumberSeriesHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskHandler for RenumberSeriesHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let series_id = task
                .series_id
                .ok_or_else(|| anyhow!("Missing series_id for RenumberSeries task"))?;

            info!(
                "Task {}: Renumbering books for series {}",
                task.id, series_id
            );

            // Fetch the series to get library_id
            let series = SeriesRepository::get_by_id(db, series_id)
                .await?
                .ok_or_else(|| anyhow!("Series {} not found", series_id))?;

            // Call the existing renumber function
            let updated_count =
                crate::scanner::renumber_series_books(db, series_id, series.library_id).await?;

            // Emit SeriesUpdated event so the frontend can refresh
            if updated_count > 0
                && let Some(broadcaster) = event_broadcaster
            {
                let event = EntityChangeEvent {
                    event: EntityEvent::SeriesUpdated {
                        series_id,
                        library_id: series.library_id,
                        fields: Some(vec!["sort_number".to_string()]),
                    },
                    timestamp: Utc::now(),
                    user_id: None,
                };
                let _ = broadcaster.emit(event);
            }

            info!(
                "Task {}: Renumbered {} books in series {}",
                task.id, updated_count, series_id
            );

            Ok(TaskResult::success_with_data(
                format!("Renumbered {} books", updated_count),
                serde_json::json!({
                    "series_id": series_id,
                    "updated_count": updated_count,
                }),
            ))
        })
    }
}

// =============================================================================
// RenumberSeriesBatch Handler (Fan-out)
// =============================================================================

pub struct RenumberSeriesBatchHandler;

impl RenumberSeriesBatchHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RenumberSeriesBatchHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskHandler for RenumberSeriesBatchHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            info!(
                "Task {}: Starting batch series renumbering (fan-out)",
                task.id
            );

            // Extract series_ids from params
            let series_ids: Option<Vec<Uuid>> = task
                .params
                .as_ref()
                .and_then(|p| p.get("series_ids"))
                .and_then(|v| serde_json::from_value(v.clone()).ok());

            let series_to_process = series_ids
                .ok_or_else(|| anyhow!("RenumberSeriesBatch task requires series_ids"))?;

            let total = series_to_process.len();
            info!("Found {} series to renumber", total);

            if total == 0 {
                return Ok(TaskResult::success_with_data(
                    "No series to renumber".to_string(),
                    serde_json::json!({
                        "total": 0,
                        "enqueued": 0,
                    }),
                ));
            }

            // Enqueue individual RenumberSeries tasks for each series
            let mut enqueued = 0;
            let mut errors = Vec::new();

            for series_id in series_to_process {
                let task_type = TaskType::RenumberSeries { series_id };

                match TaskRepository::enqueue(db, task_type, None).await {
                    Ok(task_id) => {
                        debug!(
                            "Enqueued renumber task {} for series {}",
                            task_id, series_id
                        );
                        enqueued += 1;
                    }
                    Err(e) => {
                        let error_msg = format!(
                            "Failed to enqueue renumber task for series {}: {}",
                            series_id, e
                        );
                        warn!("{}", error_msg);
                        errors.push(error_msg);
                    }
                }
            }

            info!(
                "Batch series renumbering complete: enqueued {} tasks ({} errors)",
                enqueued,
                errors.len()
            );

            Ok(TaskResult::success_with_data(
                format!("Enqueued {} renumber tasks", enqueued),
                serde_json::json!({
                    "total": total,
                    "enqueued": enqueued,
                    "errors": errors,
                }),
            ))
        })
    }
}
