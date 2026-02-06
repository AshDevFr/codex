use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::db::entities::tasks;
use crate::db::repositories::{
    BookRepository, LibraryRepository, PluginsRepository, SeriesRepository, TaskRepository,
};
use crate::events::EventBroadcaster;
use crate::scanner::{ScanMode, ScanningConfig, scan_library};
use crate::services::plugin::protocol::PluginScope;
use crate::services::settings::SettingsService;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::{TaskResult, TaskType};

/// Settings key for enabling post-scan auto-match
const SETTING_POST_SCAN_AUTO_MATCH_ENABLED: &str = "plugins.post_scan_auto_match_enabled";
/// Default value for post-scan auto-match (disabled for safety)
const DEFAULT_POST_SCAN_AUTO_MATCH_ENABLED: bool = false;

pub struct ScanLibraryHandler {
    settings_service: Option<Arc<SettingsService>>,
}

impl Default for ScanLibraryHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanLibraryHandler {
    pub fn new() -> Self {
        Self {
            settings_service: None,
        }
    }

    /// Enable post-scan auto-match by providing a settings service
    pub fn with_settings_service(mut self, settings_service: Arc<SettingsService>) -> Self {
        self.settings_service = Some(settings_service);
        self
    }

    /// Queue plugin auto-match tasks for all series in the library
    ///
    /// This is called after a library scan completes. It:
    /// 1. Checks if the feature is enabled via settings
    /// 2. Finds all plugins with `library:scan` scope that apply to this library
    /// 3. Gets all series in the library
    /// 4. Enqueues auto-match tasks for each series/plugin combination
    ///
    /// Returns the number of tasks queued (0 if feature is disabled or no applicable plugins).
    async fn queue_post_scan_auto_match(
        &self,
        db: &DatabaseConnection,
        task_id: uuid::Uuid,
        library_id: uuid::Uuid,
    ) -> usize {
        // Check if feature is enabled via settings
        let is_enabled = if let Some(ref settings) = self.settings_service {
            settings
                .get_bool(
                    SETTING_POST_SCAN_AUTO_MATCH_ENABLED,
                    DEFAULT_POST_SCAN_AUTO_MATCH_ENABLED,
                )
                .await
                .unwrap_or(DEFAULT_POST_SCAN_AUTO_MATCH_ENABLED)
        } else {
            debug!(
                "Task {}: SettingsService not available, post-scan auto-match disabled",
                task_id
            );
            return 0;
        };

        if !is_enabled {
            debug!(
                "Task {}: Post-scan auto-match is disabled via settings",
                task_id
            );
            return 0;
        }

        // Find plugins with library:scan scope that apply to this library
        let plugins = match PluginsRepository::get_enabled_by_scope_and_library(
            db,
            &PluginScope::LibraryScan,
            library_id,
        )
        .await
        {
            Ok(plugins) => plugins,
            Err(e) => {
                warn!(
                    "Task {}: Failed to query plugins for post-scan auto-match: {}",
                    task_id, e
                );
                return 0;
            }
        };

        if plugins.is_empty() {
            debug!(
                "Task {}: No plugins with library:scan scope found for library {}",
                task_id, library_id
            );
            return 0;
        }

        info!(
            "Task {}: Found {} plugin(s) with library:scan scope for library {}",
            task_id,
            plugins.len(),
            library_id
        );

        // Get all series in the library
        let series_list = match SeriesRepository::list_by_library(db, library_id).await {
            Ok(series) => series,
            Err(e) => {
                warn!(
                    "Task {}: Failed to list series for post-scan auto-match: {}",
                    task_id, e
                );
                return 0;
            }
        };

        if series_list.is_empty() {
            debug!(
                "Task {}: No series found in library {} for post-scan auto-match",
                task_id, library_id
            );
            return 0;
        }

        info!(
            "Task {}: Queueing auto-match tasks for {} series with {} plugin(s)",
            task_id,
            series_list.len(),
            plugins.len()
        );

        // Enqueue auto-match tasks for each series/plugin combination
        let mut tasks_queued = 0;
        for series in &series_list {
            for plugin in &plugins {
                match TaskRepository::enqueue(
                    db,
                    TaskType::PluginAutoMatch {
                        series_id: series.id,
                        plugin_id: plugin.id,
                        source_scope: Some("library:scan".to_string()),
                    },
                    0,    // priority (normal)
                    None, // schedule now
                )
                .await
                {
                    Ok(_) => tasks_queued += 1,
                    Err(e) => {
                        // Log but don't fail - other tasks may succeed
                        warn!(
                            "Task {}: Failed to enqueue auto-match task for series {} with plugin {}: {}",
                            task_id, series.id, plugin.id, e
                        );
                    }
                }
            }
        }

        if tasks_queued > 0 {
            info!(
                "Task {}: Queued {} auto-match tasks for post-scan processing",
                task_id, tasks_queued
            );
        }

        tasks_queued
    }
}

impl TaskHandler for ScanLibraryHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
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

            // Execute scan (without progress channel for now, pass event_broadcaster)
            // Note: Analysis tasks are now queued during the scan itself (streaming),
            // so workers can start processing immediately rather than waiting for scan to complete.
            match scan_library(db, library_id, scan_mode, None, event_broadcaster).await {
                Ok(result) => {
                    info!(
                        "Task {}: Library scan completed - {} files processed, {} series, {} books, {} analysis tasks queued",
                        task.id,
                        result.files_processed,
                        result.series_created,
                        result.books_created,
                        result.tasks_queued
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
                                            db,
                                            library_id,
                                            event_broadcaster,
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

                    // Post-scan auto-match: Queue plugin auto-match tasks for series
                    // if the feature is enabled and there are plugins with library:scan scope
                    let auto_match_tasks_queued = self
                        .queue_post_scan_auto_match(db, task.id, library_id)
                        .await;

                    Ok(TaskResult::success_with_data(
                        format!(
                            "Scanned {} files ({} series, {} books), queued {} analysis tasks{}{}",
                            result.files_processed,
                            result.series_created,
                            result.books_created,
                            result.tasks_queued,
                            if purged_count > 0 {
                                format!(", purged {} deleted books", purged_count)
                            } else {
                                String::new()
                            },
                            if auto_match_tasks_queued > 0 {
                                format!(", queued {} auto-match tasks", auto_match_tasks_queued)
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
                            "tasks_queued": result.tasks_queued,
                            "books_purged": purged_count,
                            "auto_match_tasks_queued": auto_match_tasks_queued,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setting_constants() {
        assert_eq!(
            SETTING_POST_SCAN_AUTO_MATCH_ENABLED,
            "plugins.post_scan_auto_match_enabled"
        );
        const { assert!(!DEFAULT_POST_SCAN_AUTO_MATCH_ENABLED) };
    }

    #[test]
    fn test_handler_creation() {
        let handler = ScanLibraryHandler::new();
        assert!(handler.settings_service.is_none());
    }

    #[test]
    fn test_handler_default() {
        let handler = ScanLibraryHandler::default();
        assert!(handler.settings_service.is_none());
    }
}
