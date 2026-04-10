//! Handler for ExportSeries task
//!
//! Loads the series export record, collects data, writes to a file via
//! ExportStorage, and emits SSE progress events. On completion, enforces
//! the per-user export cap by evicting the oldest exports.
//! On failure, marks the export record as failed and cleans up partial files.

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::db::entities::tasks;
use crate::db::repositories::SeriesExportRepository;
use crate::events::{EventBroadcaster, TaskProgressEvent};
use crate::services::SettingsService;
use crate::services::export_storage::ExportStorage;
use crate::services::series_export_collector::{self, ExportField};
use crate::services::series_export_writer;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Default maximum number of completed exports kept per user.
const DEFAULT_MAX_PER_USER: u64 = 10;

pub struct ExportSeriesHandler {
    export_storage: Arc<ExportStorage>,
    settings_service: Arc<SettingsService>,
}

impl ExportSeriesHandler {
    pub fn new(export_storage: Arc<ExportStorage>, settings_service: Arc<SettingsService>) -> Self {
        Self {
            export_storage,
            settings_service,
        }
    }
}

impl TaskHandler for ExportSeriesHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let task_id = task.id;
            let params = &task.params;

            // Extract export_id and user_id from task params
            let export_id: Uuid = params
                .as_ref()
                .and_then(|p| p.get("export_id"))
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| anyhow::anyhow!("Missing export_id in task params"))?;

            let user_id: Uuid = params
                .as_ref()
                .and_then(|p| p.get("user_id"))
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| anyhow::anyhow!("Missing user_id in task params"))?;

            info!("Task {task_id}: Starting series export {export_id} for user {user_id}");

            // Load the export record
            let export = SeriesExportRepository::find_by_id(db, export_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Export record not found: {export_id}"))?;

            // Mark as running
            SeriesExportRepository::mark_running(db, export_id, task_id).await?;
            let started_at = Utc::now();
            let format = export.format.clone();

            // Emit started event
            if let Some(broadcaster) = event_broadcaster {
                let _ = broadcaster.emit_task(TaskProgressEvent::started(
                    task_id,
                    "export_series",
                    None,
                    None,
                    None,
                ));
            }

            // Run the export; on error, mark the record as failed before propagating
            let result = self
                .run_export(
                    task_id,
                    export_id,
                    user_id,
                    &export,
                    db,
                    event_broadcaster,
                    started_at,
                )
                .await;

            match result {
                Ok(task_result) => Ok(task_result),
                Err(e) => {
                    let error_msg = format!("{e:#}");
                    error!("Task {task_id}: Export {export_id} failed: {error_msg}");

                    // Mark export as failed in DB
                    if let Err(mark_err) =
                        SeriesExportRepository::mark_failed(db, export_id, &error_msg).await
                    {
                        error!("Task {task_id}: Failed to mark export as failed: {mark_err}");
                    }

                    // Clean up any partial file
                    let _ = self
                        .export_storage
                        .delete(user_id, export_id, &format)
                        .await;

                    // Emit failed event
                    if let Some(broadcaster) = event_broadcaster {
                        let _ = broadcaster.emit_task(TaskProgressEvent::failed(
                            task_id,
                            "export_series",
                            &error_msg,
                            started_at,
                            None,
                            None,
                            None,
                        ));
                    }

                    Err(e)
                }
            }
        })
    }
}

impl ExportSeriesHandler {
    /// Inner export logic, separated so errors can be caught and the export
    /// record marked as failed.
    #[allow(clippy::too_many_arguments)]
    async fn run_export(
        &self,
        task_id: Uuid,
        export_id: Uuid,
        user_id: Uuid,
        export: &crate::db::entities::series_exports::Model,
        db: &DatabaseConnection,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
        started_at: chrono::DateTime<Utc>,
    ) -> Result<TaskResult> {
        // Parse library_ids and fields from the export record
        let library_ids: Vec<Uuid> = serde_json::from_value(export.library_ids.clone())
            .context("Failed to parse library_ids from export record")?;

        let field_keys: Vec<String> = serde_json::from_value(export.fields.clone())
            .context("Failed to parse fields from export record")?;

        let fields: Vec<ExportField> = field_keys
            .iter()
            .filter_map(|k| ExportField::parse(k))
            .collect();

        // Ensure anchor fields are included
        let mut all_fields = fields.clone();
        for anchor in ExportField::ANCHORS {
            if !all_fields.contains(anchor) {
                all_fields.push(*anchor);
            }
        }

        // Resolve visible series IDs
        info!(
            "Task {task_id}: Resolving series for {} libraries",
            library_ids.len()
        );
        let series_ids =
            series_export_collector::resolve_series_ids(db, user_id, &library_ids).await?;

        let total = series_ids.len();
        info!("Task {task_id}: Found {total} series to export");

        // Emit progress: series resolved
        if let Some(broadcaster) = event_broadcaster {
            let _ = broadcaster.emit_task(TaskProgressEvent::progress(
                task_id,
                "export_series",
                0,
                total,
                Some(format!("Found {total} series to export")),
                None,
                None,
                None,
            ));
        }

        // Collect all rows, emitting progress along the way
        let mut rows = Vec::with_capacity(total);
        let mut progress_counter = 0usize;
        let broadcaster_ref = event_broadcaster;

        series_export_collector::collect_batched(db, user_id, &series_ids, &all_fields, |row| {
            rows.push(row);
            progress_counter += 1;

            // Emit progress every 50 rows
            if (progress_counter.is_multiple_of(50) || progress_counter == total)
                && let Some(broadcaster) = broadcaster_ref
            {
                let _ = broadcaster.emit_task(TaskProgressEvent::progress(
                    task_id,
                    "export_series",
                    progress_counter,
                    total,
                    Some(format!(
                        "Collecting series data ({progress_counter}/{total})"
                    )),
                    None,
                    None,
                    None,
                ));
            }
        })
        .await?;

        // Write the file atomically
        let format = &export.format;
        info!(
            "Task {task_id}: Writing {format} export ({} rows)",
            rows.len()
        );

        let (final_path, file_size) = self
            .export_storage
            .write_atomic(user_id, export_id, format, |tmp_path| {
                let rows = rows;
                let format = format.clone();
                let all_fields = all_fields.clone();
                async move {
                    match format.as_str() {
                        "csv" => series_export_writer::write_csv(tmp_path, all_fields, rows)
                            .await
                            .map(|_| ()),
                        _ => series_export_writer::write_json(tmp_path, rows)
                            .await
                            .map(|_| ()),
                    }
                }
            })
            .await?;

        let row_count = progress_counter as i32;
        let file_path_str = final_path.to_string_lossy().to_string();

        // Mark completed in DB
        SeriesExportRepository::mark_completed(
            db,
            export_id,
            &file_path_str,
            file_size as i64,
            row_count,
        )
        .await?;

        info!(
            "Task {task_id}: Export completed - {row_count} rows, {file_size} bytes at {}",
            final_path.display()
        );

        // Enforce per-user cap: evict oldest completed exports
        let max_per_user = self
            .settings_service
            .get_uint("exports.max_per_user", DEFAULT_MAX_PER_USER)
            .await
            .unwrap_or(DEFAULT_MAX_PER_USER);

        let to_evict =
            SeriesExportRepository::list_oldest_for_user(db, user_id, max_per_user).await?;

        for old_export in &to_evict {
            let _ = self
                .export_storage
                .delete(user_id, old_export.id, &old_export.format)
                .await;
            if let Err(e) = SeriesExportRepository::delete_by_id(db, old_export.id).await {
                warn!(
                    "Task {task_id}: Failed to evict old export {}: {e}",
                    old_export.id
                );
            }
        }

        if !to_evict.is_empty() {
            info!(
                "Task {task_id}: Evicted {} old exports (cap: {max_per_user})",
                to_evict.len()
            );
        }

        // Emit completed event
        if let Some(broadcaster) = event_broadcaster {
            let _ = broadcaster.emit_task(TaskProgressEvent::completed(
                task_id,
                "export_series",
                started_at,
                None,
                None,
                None,
            ));
        }

        Ok(TaskResult::success_with_data(
            format!("Exported {row_count} series to {format}"),
            json!({
                "export_id": export_id.to_string(),
                "format": format,
                "row_count": row_count,
                "file_size_bytes": file_size,
                "evicted_count": to_evict.len(),
            }),
        ))
    }
}
