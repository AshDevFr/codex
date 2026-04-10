//! Handler for CleanupSeriesExports task
//!
//! Cleans up expired series exports: deletes DB records and files on disk.
//! Also sweeps orphaned files and stale .tmp files, and enforces the
//! global storage cap.

use anyhow::Result;
use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};

use crate::db::entities::tasks;
use crate::db::repositories::SeriesExportRepository;
use crate::events::EventBroadcaster;
use crate::services::SettingsService;
use crate::services::export_storage::ExportStorage;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Default global storage cap: 2 GiB
const DEFAULT_STORAGE_CAP_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub struct CleanupSeriesExportsHandler {
    export_storage: Arc<ExportStorage>,
    settings_service: Arc<SettingsService>,
}

impl CleanupSeriesExportsHandler {
    pub fn new(export_storage: Arc<ExportStorage>, settings_service: Arc<SettingsService>) -> Self {
        Self {
            export_storage,
            settings_service,
        }
    }
}

impl TaskHandler for CleanupSeriesExportsHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let task_id = task.id;
            info!("Task {task_id}: Starting series exports cleanup");

            let now = Utc::now();
            let mut expired_count = 0u64;
            let mut stale_tmp_count = 0u64;
            let mut cap_evicted_count = 0u64;

            // 1. Delete expired exports (completed + expires_at < now)
            let expired = SeriesExportRepository::list_expired(db, now).await?;
            for export in &expired {
                // Delete file
                let _ = self
                    .export_storage
                    .delete(export.user_id, export.id, &export.format)
                    .await;
                // Delete DB record
                if let Err(e) = SeriesExportRepository::delete_by_id(db, export.id).await {
                    warn!(
                        "Task {task_id}: Failed to delete expired export {}: {e}",
                        export.id
                    );
                } else {
                    expired_count += 1;
                }
            }

            if expired_count > 0 {
                info!("Task {task_id}: Deleted {expired_count} expired exports");
            }

            // 2. Sweep stale .tmp files (older than 1 hour)
            let stale_duration = std::time::Duration::from_secs(3600);
            match self
                .export_storage
                .list_stale_tmp_files(stale_duration)
                .await
            {
                Ok(stale_files) => {
                    for path in &stale_files {
                        if let Err(e) = tokio::fs::remove_file(path).await {
                            warn!(
                                "Task {task_id}: Failed to remove stale tmp file {}: {e}",
                                path.display()
                            );
                        } else {
                            stale_tmp_count += 1;
                        }
                    }
                    if stale_tmp_count > 0 {
                        info!("Task {task_id}: Removed {stale_tmp_count} stale .tmp files");
                    }
                }
                Err(e) => {
                    warn!("Task {task_id}: Failed to list stale tmp files: {e}");
                }
            }

            // 3. Enforce global storage cap
            let cap_bytes = self
                .settings_service
                .get_uint("exports.storage_cap_bytes", DEFAULT_STORAGE_CAP_BYTES)
                .await
                .unwrap_or(DEFAULT_STORAGE_CAP_BYTES);

            let total_size = SeriesExportRepository::total_size_bytes(db).await? as u64;
            if total_size > cap_bytes {
                info!(
                    "Task {task_id}: Storage cap exceeded ({total_size} > {cap_bytes}), evicting oldest exports"
                );

                // Get ALL completed exports ordered oldest first, evict until under cap
                // We use list_expired with a far-future date to get all completed, then sort
                let all_completed = {
                    use crate::db::entities::series_exports;
                    use crate::db::entities::series_exports::Entity as SeriesExport;
                    use sea_orm::*;

                    SeriesExport::find()
                        .filter(series_exports::Column::Status.eq("completed"))
                        .order_by_asc(series_exports::Column::CreatedAt)
                        .all(db)
                        .await?
                };

                let mut remaining_size = total_size;
                for export in &all_completed {
                    if remaining_size <= cap_bytes {
                        break;
                    }
                    let file_size = export.file_size_bytes.unwrap_or(0) as u64;
                    let _ = self
                        .export_storage
                        .delete(export.user_id, export.id, &export.format)
                        .await;
                    if let Err(e) = SeriesExportRepository::delete_by_id(db, export.id).await {
                        warn!(
                            "Task {task_id}: Failed to evict export {} for cap: {e}",
                            export.id
                        );
                    } else {
                        remaining_size = remaining_size.saturating_sub(file_size);
                        cap_evicted_count += 1;
                    }
                }

                if cap_evicted_count > 0 {
                    info!(
                        "Task {task_id}: Evicted {cap_evicted_count} exports to enforce storage cap"
                    );
                }
            }

            let total_cleaned = expired_count + stale_tmp_count + cap_evicted_count;
            let message = if total_cleaned == 0 {
                "No exports needed cleanup".to_string()
            } else {
                format!(
                    "Cleaned up {total_cleaned} items ({expired_count} expired, {stale_tmp_count} stale tmp, {cap_evicted_count} cap evictions)"
                )
            };

            info!("Task {task_id}: {message}");

            Ok(TaskResult::success_with_data(
                &message,
                json!({
                    "expired_deleted": expired_count,
                    "stale_tmp_deleted": stale_tmp_count,
                    "cap_evicted": cap_evicted_count,
                }),
            ))
        })
    }
}

/// Reconcile orphaned export records on startup.
///
/// Marks any exports in non-terminal status (pending/running) as failed,
/// since the server restarted and those tasks won't complete.
pub async fn reconcile_on_startup(db: &DatabaseConnection) -> Result<u64> {
    let orphans = SeriesExportRepository::list_non_terminal(db).await?;
    let count = orphans.len() as u64;

    for export in orphans {
        if let Err(e) =
            SeriesExportRepository::mark_failed(db, export.id, "interrupted by restart").await
        {
            warn!("Failed to reconcile orphaned export {}: {e}", export.id);
        }
    }

    if count > 0 {
        info!("Reconciled {count} orphaned series export(s) on startup");
    }

    Ok(count)
}
