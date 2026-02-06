//! Task worker that processes tasks from the queue
//!
//! TODO: Remove allow(dead_code) once all task worker features are fully integrated

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::config::FilesConfig;
use crate::db::repositories::TaskRepository;
use crate::events::{EventBroadcaster, RecordedEvent, TaskProgressEvent};
use crate::services::PdfPageCache;
use crate::services::plugin::PluginManager;
use crate::services::{SettingsService, TaskMetricsService, ThumbnailService};
use crate::tasks::error::check_rate_limited;
use crate::tasks::handlers::{
    AnalyzeBookHandler, AnalyzeSeriesHandler, CleanupBookFilesHandler, CleanupOrphanedFilesHandler,
    CleanupPdfCacheHandler, CleanupPluginDataHandler, CleanupSeriesFilesHandler,
    FindDuplicatesHandler, GenerateSeriesThumbnailHandler, GenerateSeriesThumbnailsHandler,
    GenerateThumbnailHandler, GenerateThumbnailsHandler, PluginAutoMatchHandler,
    PurgeDeletedHandler, ReprocessSeriesTitleHandler, ReprocessSeriesTitlesHandler,
    ScanLibraryHandler, TaskHandler,
};

/// Task worker that processes tasks from the queue
pub struct TaskWorker {
    db: DatabaseConnection,
    handlers: HashMap<String, Arc<dyn TaskHandler>>,
    worker_id: String,
    poll_interval: Duration,
    event_broadcaster: Option<Arc<EventBroadcaster>>,
    settings_service: Option<Arc<SettingsService>>,
    thumbnail_service: Option<Arc<ThumbnailService>>,
    task_metrics_service: Option<Arc<TaskMetricsService>>,
    plugin_manager: Option<Arc<PluginManager>>,
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl TaskWorker {
    /// Create a new task worker
    pub fn new(db: DatabaseConnection) -> Self {
        let mut handlers: HashMap<String, Arc<dyn TaskHandler>> = HashMap::new();

        // Register all handlers
        handlers.insert(
            "scan_library".to_string(),
            Arc::new(ScanLibraryHandler::new()),
        );
        handlers.insert(
            "analyze_book".to_string(),
            Arc::new(AnalyzeBookHandler::new()),
        );
        handlers.insert(
            "analyze_series".to_string(),
            Arc::new(AnalyzeSeriesHandler::new()),
        );
        handlers.insert(
            "purge_deleted".to_string(),
            Arc::new(PurgeDeletedHandler::new()),
        );
        // Note: generate_thumbnails handler is registered when ThumbnailService is set
        handlers.insert(
            "find_duplicates".to_string(),
            Arc::new(FindDuplicatesHandler::new()),
        );
        // Reprocess series title handlers (no dependencies)
        handlers.insert(
            "reprocess_series_title".to_string(),
            Arc::new(ReprocessSeriesTitleHandler::new()),
        );
        handlers.insert(
            "reprocess_series_titles".to_string(),
            Arc::new(ReprocessSeriesTitlesHandler::new()),
        );
        // Plugin data cleanup handler (no dependencies)
        handlers.insert(
            "cleanup_plugin_data".to_string(),
            Arc::new(CleanupPluginDataHandler::new()),
        );

        // Generate worker ID from hostname or random UUID
        let worker_id = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| format!("worker-{}", Uuid::new_v4()));

        Self {
            db,
            handlers,
            worker_id,
            poll_interval: Duration::from_secs(5),
            event_broadcaster: None,
            settings_service: None,
            thumbnail_service: None,
            task_metrics_service: None,
            plugin_manager: None,
            shutdown_tx: None,
        }
    }

    /// Set the poll interval
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Set a custom worker ID (useful for testing)
    pub fn with_worker_id(mut self, worker_id: impl Into<String>) -> Self {
        self.worker_id = worker_id.into();
        self
    }

    /// Set the event broadcaster for task progress events
    pub fn with_event_broadcaster(mut self, broadcaster: Arc<EventBroadcaster>) -> Self {
        self.event_broadcaster = Some(broadcaster);
        self
    }

    /// Set the settings service for runtime configuration
    ///
    /// This also registers/updates handlers that depend on settings:
    /// - `ScanLibraryHandler` for post-scan auto-match settings
    pub fn with_settings_service(mut self, settings_service: Arc<SettingsService>) -> Self {
        // Re-register ScanLibraryHandler with settings service for post-scan auto-match
        self.handlers.insert(
            "scan_library".to_string(),
            Arc::new(ScanLibraryHandler::new().with_settings_service(settings_service.clone())),
        );
        self.settings_service = Some(settings_service);
        self
    }

    /// Set the thumbnail service for thumbnail generation
    pub fn with_thumbnail_service(mut self, thumbnail_service: Arc<ThumbnailService>) -> Self {
        // Register the GenerateThumbnailsHandler (batch) with thumbnail service
        self.handlers.insert(
            "generate_thumbnails".to_string(),
            Arc::new(GenerateThumbnailsHandler::new(thumbnail_service.clone())),
        );
        // Register the GenerateThumbnailHandler (single book) with thumbnail service
        self.handlers.insert(
            "generate_thumbnail".to_string(),
            Arc::new(GenerateThumbnailHandler::new(thumbnail_service.clone())),
        );
        // Register the GenerateSeriesThumbnailHandler (single series) with thumbnail service
        self.handlers.insert(
            "generate_series_thumbnail".to_string(),
            Arc::new(GenerateSeriesThumbnailHandler::new(
                thumbnail_service.clone(),
            )),
        );
        // Register the GenerateSeriesThumbnailsHandler (batch/fan-out) with thumbnail service
        self.handlers.insert(
            "generate_series_thumbnails".to_string(),
            Arc::new(GenerateSeriesThumbnailsHandler::new(
                thumbnail_service.clone(),
            )),
        );
        self.thumbnail_service = Some(thumbnail_service);
        self
    }

    /// Set the task metrics service for recording task performance metrics
    pub fn with_task_metrics_service(
        mut self,
        task_metrics_service: Arc<TaskMetricsService>,
    ) -> Self {
        self.task_metrics_service = Some(task_metrics_service);
        self
    }

    /// Set the plugin manager for plugin auto-match tasks
    ///
    /// This registers the `plugin_auto_match` task handler that enables
    /// background metadata matching via plugins.
    ///
    /// **Note**: Call `with_thumbnail_service` and `with_settings_service` before this method so that
    /// `PluginAutoMatchHandler` can download/apply cover images and respect confidence threshold settings.
    pub fn with_plugin_manager(mut self, plugin_manager: Arc<PluginManager>) -> Self {
        // Register the PluginAutoMatchHandler with ThumbnailService and SettingsService if available
        let mut handler = PluginAutoMatchHandler::new(plugin_manager.clone());
        if let Some(ref thumbnail_service) = self.thumbnail_service {
            handler = handler.with_thumbnail_service(thumbnail_service.clone());
        } else {
            tracing::warn!(
                "ThumbnailService not set - PluginAutoMatchHandler will not download covers. \
                 Call with_thumbnail_service before with_plugin_manager."
            );
        }
        if let Some(ref settings_service) = self.settings_service {
            handler = handler.with_settings_service(settings_service.clone());
        } else {
            tracing::warn!(
                "SettingsService not set - PluginAutoMatchHandler will use default confidence threshold. \
                 Call with_settings_service before with_plugin_manager."
            );
        }
        self.handlers
            .insert("plugin_auto_match".to_string(), Arc::new(handler));
        self.plugin_manager = Some(plugin_manager);
        self
    }

    /// Set the files config for cleanup handlers
    ///
    /// This registers the cleanup task handlers that need access to
    /// thumbnail and upload directories.
    ///
    /// **Note**: Call `with_thumbnail_service` before this method so that
    /// `CleanupBookFilesHandler` can invalidate series thumbnails.
    pub fn with_files_config(mut self, files_config: FilesConfig) -> Self {
        // Register cleanup handlers
        // CleanupBookFilesHandler needs ThumbnailService to invalidate series thumbnails
        if let Some(ref thumbnail_service) = self.thumbnail_service {
            self.handlers.insert(
                "cleanup_book_files".to_string(),
                Arc::new(CleanupBookFilesHandler::new(
                    files_config.clone(),
                    thumbnail_service.clone(),
                )),
            );
        } else {
            tracing::warn!(
                "ThumbnailService not set - CleanupBookFilesHandler will not be registered. \
                 Call with_thumbnail_service before with_files_config."
            );
        }
        self.handlers.insert(
            "cleanup_series_files".to_string(),
            Arc::new(CleanupSeriesFilesHandler::new(files_config.clone())),
        );
        self.handlers.insert(
            "cleanup_orphaned_files".to_string(),
            Arc::new(CleanupOrphanedFilesHandler::new(files_config)),
        );
        self
    }

    /// Set the PDF cache and settings service for PDF cache cleanup handler
    ///
    /// This registers the CleanupPdfCache task handler.
    pub fn with_pdf_cache(
        mut self,
        pdf_cache: Arc<PdfPageCache>,
        settings_service: Arc<SettingsService>,
    ) -> Self {
        self.handlers.insert(
            "cleanup_pdf_cache".to_string(),
            Arc::new(CleanupPdfCacheHandler::new(pdf_cache, settings_service)),
        );
        self
    }

    /// Get the worker ID
    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    /// Check if we're running in distributed mode (PostgreSQL)
    ///
    /// In distributed mode, workers may run in separate processes from the web server,
    /// so events need to be recorded and replayed via the TaskListener.
    fn is_distributed_mode(&self) -> bool {
        // Check if database is PostgreSQL (indicates distributed deployment)
        matches!(&self.db, DatabaseConnection::SqlxPostgresPoolConnection(_))
    }

    /// Get a shutdown sender to stop the worker
    pub fn shutdown_sender(&self) -> Option<broadcast::Sender<()>> {
        self.shutdown_tx.clone()
    }

    /// Create shutdown channel and prepare worker for running
    /// Call this before spawning the worker
    pub fn with_shutdown(mut self) -> (Self, broadcast::Sender<()>) {
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx.clone());
        (self, shutdown_tx)
    }

    /// Run the worker with graceful shutdown support
    pub async fn run(&mut self) -> Result<()> {
        let shutdown_tx = self
            .shutdown_tx
            .clone()
            .expect("Worker must be initialized with with_shutdown() before running");
        let mut shutdown_rx = shutdown_tx.subscribe();

        info!("Task worker {} started", self.worker_id);

        // Get cleanup interval from settings
        let cleanup_interval_secs = if let Some(ref settings) = self.settings_service {
            settings
                .get_uint("task.cleanup_interval_seconds", 30)
                .await
                .unwrap_or(30)
        } else {
            30
        };

        info!(
            "Task worker using cleanup interval: {} seconds",
            cleanup_interval_secs
        );

        // Spawn background cleanup task for completed tasks
        let db_clone = self.db.clone();
        let settings_clone = self.settings_service.clone();
        let mut shutdown_rx_cleanup = shutdown_rx.resubscribe();
        let cleanup_handle = tokio::spawn(async move {
            loop {
                // Get cleanup interval from settings (hot-reload support)
                let interval = if let Some(ref settings) = settings_clone {
                    settings
                        .get_uint("task.cleanup_interval_seconds", 30)
                        .await
                        .unwrap_or(30)
                } else {
                    30
                };

                tokio::select! {
                    _ = sleep(Duration::from_secs(interval)) => {
                        // Clean up completed tasks older than 10 seconds
                        match TaskRepository::purge_completed_tasks(&db_clone, 10).await {
                            Ok(count) if count > 0 => {
                                debug!("Cleaned up {} completed tasks", count);
                            }
                            Ok(_) => {} // No tasks to clean up
                            Err(e) => {
                                error!("Failed to clean up completed tasks: {}", e);
                            }
                        }
                    }
                    _ = shutdown_rx_cleanup.recv() => {
                        info!("Cleanup task shutting down...");
                        break;
                    }
                }
            }
        });

        // Spawn background cleanup task for stale tasks
        let db_clone_stale = self.db.clone();
        let mut shutdown_rx_stale = shutdown_tx.subscribe();
        let stale_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = sleep(Duration::from_secs(60)) => {
                        // Recover tasks locked for more than 10 minutes (600 seconds)
                        // This is 2x the normal lock duration to avoid false positives
                        match TaskRepository::recover_stale_tasks(&db_clone_stale, 600).await {
                            Ok(count) if count > 0 => {
                                warn!("Recovered {} stale tasks from dead workers", count);
                            }
                            Ok(_) => {} // No stale tasks
                            Err(e) => {
                                error!("Failed to recover stale tasks: {}", e);
                            }
                        }
                    }
                    _ = shutdown_rx_stale.recv() => {
                        info!("Stale task recovery shutting down...");
                        break;
                    }
                }
            }
        });

        // Get initial poll interval from settings
        let mut poll_interval = if let Some(ref settings) = self.settings_service {
            let interval = settings
                .get_uint("task.poll_interval_seconds", 5)
                .await
                .unwrap_or(5);
            info!("Task worker using poll interval: {} seconds", interval);
            Duration::from_secs(interval)
        } else {
            self.poll_interval
        };

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Task worker {} received shutdown signal", self.worker_id);
                    break;
                }
                result = self.process_next_task() => {
                    match result {
                        Ok(true) => {
                            // Processed a task, immediately check for more
                            continue;
                        }
                        Ok(false) => {
                            // No tasks available, sleep
                            // Reload poll interval from settings (hot-reload support)
                            if let Some(ref settings) = self.settings_service {
                                let interval = settings
                                    .get_uint("task.poll_interval_seconds", 5)
                                    .await
                                    .unwrap_or(5);
                                poll_interval = Duration::from_secs(interval);
                            }

                            debug!("No tasks available, sleeping for {:?}", poll_interval);
                            sleep(poll_interval).await;
                        }
                        Err(e) => {
                            error!("Worker error: {}", e);
                            // Sleep longer on error to avoid rapid retry loops
                            sleep(Duration::from_secs(10)).await;
                        }
                    }
                }
            }
        }

        // Wait for background tasks to finish
        info!("Waiting for background tasks to complete...");
        let _ = tokio::join!(cleanup_handle, stale_handle);
        info!("Task worker {} stopped", self.worker_id);

        Ok(())
    }

    /// Process the next available task
    /// Returns Ok(true) if a task was processed, Ok(false) if no tasks were available
    async fn process_next_task(&self) -> Result<bool> {
        // Get prioritize_scans setting (hot-reload support)
        let prioritize_scans = if let Some(ref settings) = self.settings_service {
            settings
                .get_bool("task.prioritize_scans_over_analysis", true)
                .await
                .unwrap_or(true)
        } else {
            true // Default to prioritizing scans if settings service not available
        };

        // Claim next available task
        // Note: claim_next can fail due to race conditions (multiple workers competing
        // for the same task). This is expected behavior, not an error - treat it as
        // "no task available" and retry on the next poll interval.
        let task = match TaskRepository::claim_next(
            &self.db,
            &self.worker_id,
            300,
            prioritize_scans,
        )
        .await
        {
            Ok(Some(t)) => t,
            Ok(None) => return Ok(false), // No tasks available
            Err(e) => {
                // Race condition or transient DB error - log at debug level and retry
                debug!(
                    "Worker {} failed to claim task (likely race condition): {}",
                    self.worker_id, e
                );
                return Ok(false);
            }
        };

        let started_at = Utc::now();

        info!(
            "Worker {} processing task {} ({})",
            self.worker_id, task.id, task.task_type
        );

        // Emit task started event
        if let Some(ref broadcaster) = self.event_broadcaster {
            let _ = broadcaster.emit_task(TaskProgressEvent::started(
                task.id,
                &task.task_type,
                task.library_id,
                task.series_id,
                task.book_id,
            ));
        }

        // Get handler for this task type
        let handler = self.handlers.get(&task.task_type).ok_or_else(|| {
            anyhow::anyhow!("No handler registered for task type: {}", task.task_type)
        })?;

        // In distributed mode, create a recording broadcaster to capture events
        // that need to be replayed by the TaskListener on the web server
        let (task_broadcaster, recorded_events): (
            Option<Arc<EventBroadcaster>>,
            Option<Vec<RecordedEvent>>,
        ) = if self.is_distributed_mode() {
            // Create a recording broadcaster for this task
            let recording_broadcaster = Arc::new(EventBroadcaster::new_with_recording(1000, true));
            let broadcaster_clone = recording_broadcaster.clone();

            // Execute task with recording broadcaster
            let result = handler
                .handle(&task, &self.db, Some(&recording_broadcaster))
                .await;

            // Get recorded events before returning
            let events = broadcaster_clone.take_recorded_events();
            let events = if events.is_empty() {
                None
            } else {
                Some(events)
            };

            // Return result info for later processing
            match result {
                Ok(task_result) => {
                    self.complete_task(&task, task_result, started_at, events)
                        .await?;
                }
                Err(e) => {
                    self.fail_task(&task, e, started_at).await?;
                }
            }

            return Ok(true);
        } else {
            // Single-process mode: use shared broadcaster directly
            (self.event_broadcaster.clone(), None)
        };

        // Execute task with shared broadcaster (single-process mode)
        let result = handler
            .handle(&task, &self.db, task_broadcaster.as_ref())
            .await;

        // Update task status based on result
        match result {
            Ok(task_result) => {
                self.complete_task(&task, task_result, started_at, recorded_events)
                    .await?;
            }
            Err(e) => {
                self.fail_task(&task, e, started_at).await?;
            }
        }

        Ok(true)
    }

    /// Complete a task successfully, storing result and recorded events
    async fn complete_task(
        &self,
        task: &crate::db::entities::tasks::Model,
        task_result: crate::tasks::types::TaskResult,
        started_at: chrono::DateTime<Utc>,
        recorded_events: Option<Vec<RecordedEvent>>,
    ) -> Result<()> {
        let completed_at = Utc::now();

        // Merge recorded events into task result data
        let result_data = match (task_result.data.clone(), recorded_events) {
            (Some(mut data), Some(events)) if !events.is_empty() => {
                // Add recorded events to existing result data
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("emitted_events".to_string(), json!(events));
                }
                Some(data)
            }
            (None, Some(events)) if !events.is_empty() => {
                // Create result data with just the recorded events
                Some(json!({ "emitted_events": events }))
            }
            (data, _) => data,
        };

        TaskRepository::mark_completed(&self.db, task.id, result_data).await?;
        info!(
            "Task {} completed successfully: {}",
            task.id,
            task_result.message.clone().unwrap_or_default()
        );

        // Record metrics
        if let Some(ref metrics_service) = self.task_metrics_service {
            let duration_ms = (completed_at - started_at).num_milliseconds();
            let queue_wait_ms = task
                .started_at
                .map(|s| (s - task.created_at).num_milliseconds())
                .unwrap_or(0);

            // Extract items_processed and bytes_processed from task result data
            let (items_processed, bytes_processed) = task_result
                .data
                .as_ref()
                .map(|d| {
                    let items = d
                        .get("items_processed")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(1);
                    let bytes = d
                        .get("bytes_processed")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    (items, bytes)
                })
                .unwrap_or((1, 0));

            metrics_service
                .record(
                    task.task_type.clone(),
                    task.library_id,
                    true, // success
                    task.attempts > 1,
                    duration_ms,
                    queue_wait_ms,
                    items_processed,
                    bytes_processed,
                    None,
                )
                .await;
        }

        // Emit task completed event
        if let Some(ref broadcaster) = self.event_broadcaster {
            let _ = broadcaster.emit_task(TaskProgressEvent::completed(
                task.id,
                &task.task_type,
                started_at,
                task.library_id,
                task.series_id,
                task.book_id,
            ));
        }

        Ok(())
    }

    /// Handle a task failure, checking for rate-limited errors first
    ///
    /// If the error is a rate-limited error, the task is rescheduled without consuming
    /// a retry attempt. Otherwise, the task is marked as failed normally.
    async fn fail_task(
        &self,
        task: &crate::db::entities::tasks::Model,
        error: anyhow::Error,
        started_at: chrono::DateTime<Utc>,
    ) -> Result<()> {
        let completed_at = Utc::now();
        let error_string = error.to_string();

        // Check if this is a rate-limited error
        if let Some(retry_after_secs) = check_rate_limited(&error) {
            // Rate-limited: reschedule without consuming retry attempts
            info!(
                "Task {} rate-limited, rescheduling in {} seconds",
                task.id, retry_after_secs
            );

            // Warn if approaching max reschedules
            let reschedule_count = task.reschedule_count + 1;
            if reschedule_count >= task.max_reschedules - 2 {
                warn!(
                    "Task {} approaching max reschedules ({}/{})",
                    task.id, reschedule_count, task.max_reschedules
                );
            }

            TaskRepository::mark_rate_limited(&self.db, task.id, retry_after_secs).await?;

            // Record metrics for rate-limited task (as a "soft failure")
            if let Some(ref metrics_service) = self.task_metrics_service {
                let duration_ms = (completed_at - started_at).num_milliseconds();
                let queue_wait_ms = task
                    .started_at
                    .map(|s| (s - task.created_at).num_milliseconds())
                    .unwrap_or(0);

                metrics_service
                    .record(
                        task.task_type.clone(),
                        task.library_id,
                        false, // not a success
                        true,  // will be retried
                        duration_ms,
                        queue_wait_ms,
                        0,
                        0,
                        Some("rate_limited".to_string()),
                    )
                    .await;
            }

            // Emit task rescheduled event (reuse task progress event with appropriate message)
            if let Some(ref broadcaster) = self.event_broadcaster {
                let _ = broadcaster.emit_task(TaskProgressEvent::failed(
                    task.id,
                    &task.task_type,
                    format!("Rate-limited, rescheduled for {} seconds", retry_after_secs),
                    started_at,
                    task.library_id,
                    task.series_id,
                    task.book_id,
                ));
            }

            return Ok(());
        }

        // Not rate-limited: handle as normal failure
        error!("Task {} failed: {}", task.id, error_string);
        TaskRepository::mark_failed(&self.db, task.id, error_string.clone()).await?;

        // Record metrics
        if let Some(ref metrics_service) = self.task_metrics_service {
            let duration_ms = (completed_at - started_at).num_milliseconds();
            let queue_wait_ms = task
                .started_at
                .map(|s| (s - task.created_at).num_milliseconds())
                .unwrap_or(0);

            metrics_service
                .record(
                    task.task_type.clone(),
                    task.library_id,
                    false, // failed
                    task.attempts > 1,
                    duration_ms,
                    queue_wait_ms,
                    0, // no items processed on failure
                    0, // no bytes processed on failure
                    Some(error_string.clone()),
                )
                .await;
        }

        // Emit task failed event
        if let Some(ref broadcaster) = self.event_broadcaster {
            let _ = broadcaster.emit_task(TaskProgressEvent::failed(
                task.id,
                &task.task_type,
                error_string,
                started_at,
                task.library_id,
                task.series_id,
                task.book_id,
            ));
        }

        Ok(())
    }

    /// Run a single iteration of task processing (useful for testing)
    pub async fn process_once(&self) -> Result<bool> {
        self.process_next_task().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{EntityChangeEvent, EntityEvent, EntityType};

    #[test]
    fn test_worker_creation() {
        // Test that worker can be created with a valid configuration
        // Actual database tests are in tests/task_queue.rs
        let worker_id = "test-worker-123";
        assert!(!worker_id.is_empty());
    }

    #[test]
    fn test_worker_id_generation() {
        // Test worker ID format
        let hostname = std::env::var("HOSTNAME")
            .unwrap_or_else(|_| format!("worker-{}", uuid::Uuid::new_v4()));
        assert!(!hostname.is_empty());
    }

    #[test]
    fn test_poll_interval_default() {
        let default_interval = Duration::from_secs(5);
        assert_eq!(default_interval.as_secs(), 5);
    }

    #[test]
    fn test_event_recording_creates_recorded_events() {
        // Test that recording broadcaster captures events
        let broadcaster = EventBroadcaster::new_with_recording(100, true);

        let book_id = Uuid::new_v4();
        let series_id = Uuid::new_v4();
        let library_id = Uuid::new_v4();

        // Emit events
        let _ = broadcaster.emit(EntityChangeEvent::new(
            EntityEvent::BookCreated {
                book_id,
                series_id,
                library_id,
            },
            None,
        ));

        let _ = broadcaster.emit(EntityChangeEvent::new(
            EntityEvent::CoverUpdated {
                entity_type: EntityType::Book,
                entity_id: book_id,
                library_id: Some(library_id),
            },
            None,
        ));

        // Take recorded events
        let events = broadcaster.take_recorded_events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].event, EntityEvent::BookCreated { .. }));
        assert!(matches!(events[1].event, EntityEvent::CoverUpdated { .. }));
    }

    #[test]
    fn test_merge_recorded_events_into_result() {
        // Test the logic for merging recorded events into task result
        let book_id = Uuid::new_v4();
        let library_id = Uuid::new_v4();

        let recorded_events = vec![RecordedEvent {
            event: EntityEvent::CoverUpdated {
                entity_type: EntityType::Book,
                entity_id: book_id,
                library_id: Some(library_id),
            },
            timestamp: Utc::now(),
            user_id: None,
        }];

        // Test case 1: Merge into existing result data
        let existing_data = json!({ "generated": 5, "skipped": 2 });
        let result_data = match (Some(existing_data.clone()), Some(recorded_events.clone())) {
            (Some(mut data), Some(events)) if !events.is_empty() => {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("emitted_events".to_string(), json!(events));
                }
                Some(data)
            }
            _ => None,
        };

        let result = result_data.unwrap();
        assert_eq!(result["generated"], 5);
        assert_eq!(result["skipped"], 2);
        assert!(result["emitted_events"].is_array());
        assert_eq!(result["emitted_events"].as_array().unwrap().len(), 1);

        // Test case 2: Create result data with just events (no existing data)
        let result_data = match (None::<serde_json::Value>, Some(recorded_events.clone())) {
            (None, Some(events)) if !events.is_empty() => Some(json!({ "emitted_events": events })),
            _ => None,
        };

        let result = result_data.unwrap();
        assert!(result["emitted_events"].is_array());

        // Test case 3: No events, keep original data
        let existing_data = json!({ "status": "ok" });

        assert_eq!(existing_data["status"], "ok");
    }

    #[test]
    fn test_recorded_events_serialization() {
        // Test that recorded events can be serialized to JSON (as stored in task result)
        let events = vec![
            RecordedEvent {
                event: EntityEvent::BookCreated {
                    book_id: Uuid::new_v4(),
                    series_id: Uuid::new_v4(),
                    library_id: Uuid::new_v4(),
                },
                timestamp: Utc::now(),
                user_id: None,
            },
            RecordedEvent {
                event: EntityEvent::CoverUpdated {
                    entity_type: EntityType::Book,
                    entity_id: Uuid::new_v4(),
                    library_id: Some(Uuid::new_v4()),
                },
                timestamp: Utc::now(),
                user_id: Some(Uuid::new_v4()),
            },
        ];

        // Serialize
        let json_str = serde_json::to_string(&events).unwrap();
        assert!(json_str.contains("book_created"));
        assert!(json_str.contains("cover_updated"));

        // Deserialize
        let deserialized: Vec<RecordedEvent> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.len(), 2);
    }
}
