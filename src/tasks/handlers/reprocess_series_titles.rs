//! Handlers for ReprocessSeriesTitle and ReprocessSeriesTitles tasks
//!
//! Reprocesses series titles using library preprocessing rules. This is useful when
//! preprocessing rules are added or changed after series have already been created.

use anyhow::{Result, anyhow};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::entities::{series_metadata, tasks};
use crate::db::repositories::{
    LibraryRepository, SeriesMetadataRepository, SeriesRepository, TaskRepository,
};
use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster};
use crate::services::metadata::preprocessing::apply_rules;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::{TaskResult, TaskType};

// =============================================================================
// ReprocessSeriesTitle Handler (Single Series)
// =============================================================================

pub struct ReprocessSeriesTitleHandler;

impl ReprocessSeriesTitleHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReprocessSeriesTitleHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskHandler for ReprocessSeriesTitleHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let series_id = task
                .series_id
                .ok_or_else(|| anyhow!("Missing series_id for ReprocessSeriesTitle task"))?;

            info!(
                "Task {}: Reprocessing title for series {}",
                task.id, series_id
            );

            // Process the single series
            let result = reprocess_single_series(db, series_id, event_broadcaster).await?;

            if result.skipped {
                info!(
                    "Task {}: Series {} skipped (reason: {})",
                    task.id,
                    series_id,
                    result.skip_reason.as_deref().unwrap_or("unknown")
                );
            } else if result.changed {
                info!(
                    "Task {}: Series {} title changed: '{}' -> '{}'",
                    task.id, series_id, result.original_title, result.new_title
                );
            } else {
                debug!(
                    "Task {}: Series {} title unchanged: '{}'",
                    task.id, series_id, result.original_title
                );
            }

            Ok(TaskResult::success_with_data(
                if result.changed {
                    format!(
                        "Title changed: '{}' -> '{}'",
                        result.original_title, result.new_title
                    )
                } else if result.skipped {
                    format!(
                        "Skipped: {}",
                        result.skip_reason.as_deref().unwrap_or("unknown")
                    )
                } else {
                    "Title unchanged".to_string()
                },
                serde_json::json!({
                    "series_id": series_id,
                    "original_title": result.original_title,
                    "new_title": result.new_title,
                    "changed": result.changed,
                    "title_sort_cleared": result.title_sort_cleared,
                    "skipped": result.skipped,
                    "skip_reason": result.skip_reason,
                }),
            ))
        })
    }
}

// =============================================================================
// ReprocessSeriesTitles Handler (Fan-out/Batch)
// =============================================================================

pub struct ReprocessSeriesTitlesHandler;

impl ReprocessSeriesTitlesHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReprocessSeriesTitlesHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskHandler for ReprocessSeriesTitlesHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            info!(
                "Task {}: Starting batch series title reprocessing (fan-out)",
                task.id
            );

            // Extract parameters
            let library_id = task.library_id;
            let series_ids: Option<Vec<Uuid>> = task
                .params
                .as_ref()
                .and_then(|p| p.get("series_ids"))
                .and_then(|v| serde_json::from_value(v.clone()).ok());

            // Determine which series to process
            let series_to_process: Vec<Uuid> = if let Some(ids) = series_ids {
                // Bulk selection - use specific series IDs
                info!("Processing {} specifically selected series", ids.len());
                ids
            } else if let Some(lib_id) = library_id {
                // Library scope - get all series in library
                info!("Processing all series in library {}", lib_id);
                let series_list = SeriesRepository::list_by_library(db, lib_id).await?;
                series_list.into_iter().map(|s| s.id).collect()
            } else {
                return Err(anyhow!(
                    "ReprocessSeriesTitles task requires either library_id or series_ids"
                ));
            };

            let total = series_to_process.len();
            info!("Found {} series to process", total);

            if total == 0 {
                return Ok(TaskResult::success_with_data(
                    "No series to process".to_string(),
                    serde_json::json!({
                        "total": 0,
                        "enqueued": 0,
                    }),
                ));
            }

            // Enqueue individual ReprocessSeriesTitle tasks for each series
            let mut enqueued = 0;
            let mut errors = Vec::new();

            for series_id in series_to_process {
                let task_type = TaskType::ReprocessSeriesTitle { series_id };

                match TaskRepository::enqueue(db, task_type, 0, None).await {
                    Ok(task_id) => {
                        debug!(
                            "Enqueued reprocess title task {} for series {}",
                            task_id, series_id
                        );
                        enqueued += 1;
                    }
                    Err(e) => {
                        let error_msg = format!(
                            "Failed to enqueue reprocess title task for series {}: {}",
                            series_id, e
                        );
                        warn!("{}", error_msg);
                        errors.push(error_msg);
                    }
                }
            }

            info!(
                "Batch series title reprocessing complete: enqueued {} tasks ({} errors)",
                enqueued,
                errors.len()
            );

            Ok(TaskResult::success_with_data(
                format!("Enqueued {} reprocess title tasks", enqueued),
                serde_json::json!({
                    "total": total,
                    "enqueued": enqueued,
                    "errors": errors,
                }),
            ))
        })
    }
}

// =============================================================================
// Shared Logic
// =============================================================================

/// Result of reprocessing a single series title
struct ReprocessResult {
    original_title: String,
    new_title: String,
    changed: bool,
    title_sort_cleared: bool,
    skipped: bool,
    skip_reason: Option<String>,
}

/// Reprocess a single series title using library preprocessing rules
async fn reprocess_single_series(
    db: &DatabaseConnection,
    series_id: Uuid,
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
) -> Result<ReprocessResult> {
    // Fetch the series
    let series = SeriesRepository::get_by_id(db, series_id)
        .await?
        .ok_or_else(|| anyhow!("Series not found: {}", series_id))?;

    // Fetch the series metadata
    let metadata = SeriesMetadataRepository::get_by_series_id(db, series_id)
        .await?
        .ok_or_else(|| anyhow!("Series metadata not found for series: {}", series_id))?;

    // Check if title is locked
    if metadata.title_lock {
        return Ok(ReprocessResult {
            original_title: metadata.title.clone(),
            new_title: metadata.title,
            changed: false,
            title_sort_cleared: false,
            skipped: true,
            skip_reason: Some("title_locked".to_string()),
        });
    }

    // Fetch the library to get preprocessing rules
    let library = LibraryRepository::get_by_id(db, series.library_id)
        .await?
        .ok_or_else(|| anyhow!("Library not found: {}", series.library_id))?;

    // Get preprocessing rules from library
    let rules = LibraryRepository::get_preprocessing_rules(&library);

    // Apply rules to the series name (original directory name)
    let new_title = if rules.is_empty() {
        series.name.clone()
    } else {
        apply_rules(&series.name, &rules)
    };

    let original_title = metadata.title.clone();
    let changed = new_title != original_title;

    // Determine if we should clear title_sort
    let should_clear_title_sort =
        changed && !metadata.title_sort_lock && metadata.title_sort.is_some();

    // Apply the changes if title changed
    if changed {
        let mut active_metadata: series_metadata::ActiveModel = metadata.into();
        active_metadata.title = Set(new_title.clone());

        if should_clear_title_sort {
            active_metadata.title_sort = Set(None);
        }

        active_metadata.updated_at = Set(Utc::now());

        active_metadata.update(db).await?;

        // Emit update event
        if let Some(broadcaster) = event_broadcaster {
            let event = EntityChangeEvent {
                event: EntityEvent::SeriesUpdated {
                    series_id,
                    library_id: series.library_id,
                    fields: Some(if should_clear_title_sort {
                        vec!["title".to_string(), "title_sort".to_string()]
                    } else {
                        vec!["title".to_string()]
                    }),
                },
                timestamp: Utc::now(),
                user_id: None,
            };
            let _ = broadcaster.emit(event);
        }
    }

    Ok(ReprocessResult {
        original_title,
        new_title,
        changed,
        title_sort_cleared: should_clear_title_sort && changed,
        skipped: false,
        skip_reason: None,
    })
}
