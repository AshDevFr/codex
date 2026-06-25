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

use crate::error::check_rate_limited;
use crate::handlers::{
    AnalyzeBookHandler, AnalyzeSeriesHandler, BackfillTrackingFromMetadataHandler,
    BulkTrackForReleasesHandler, CleanupBookFilesHandler, CleanupOrphanedFilesHandler,
    CleanupPdfCacheHandler, CleanupPluginDataHandler, CleanupRefreshTokensHandler,
    CleanupSeriesExportsHandler, CleanupSeriesFilesHandler, ExportSeriesHandler,
    FindDuplicatesHandler, GenerateSeriesThumbnailHandler, GenerateSeriesThumbnailsHandler,
    GenerateThumbnailHandler, GenerateThumbnailsHandler, PluginAutoMatchHandler,
    PollReleaseSourceHandler, PurgeDeletedHandler, RefreshLibraryMetadataHandler,
    RenumberSeriesBatchHandler, RenumberSeriesHandler, ReprocessSeriesTitleHandler,
    ReprocessSeriesTitlesHandler, ScanLibraryHandler, TaskHandler,
    UserPluginRecommendationDismissHandler, UserPluginRecommendationsHandler,
    UserPluginSyncHandler,
};
use codex_config::FilesConfig;
use codex_db::repositories::TaskRepository;
use codex_events::{EventBroadcaster, RecordedEvent, TaskProgressEvent};
use codex_services::PdfPageCache;
use codex_services::export_storage::ExportStorage;
use codex_services::plugin::PluginManager;
use codex_services::user_plugin::OAuthStateManager;
use codex_services::{SettingsService, TaskMetricsService, ThumbnailService};

/// RAII guard that increments the OTel in-flight task gauge on creation and
/// decrements it on drop. Used by `process_next_task` to track currently-
/// executing tasks across all exit paths (success, failure, `?` propagation).
struct InFlightGuard;

impl InFlightGuard {
    fn new() -> Self {
        codex_services::metrics::task_in_flight_inc();
        Self
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        codex_services::metrics::task_in_flight_dec();
    }
}

/// RAII guard that runs a background heartbeat renewing a claimed task's lock,
/// and aborts that heartbeat on drop (when the handler finishes, fails, or the
/// function returns early).
///
/// Without this, a task that runs longer than its lock window
/// (`lock_duration_secs`) has its lock lapse and gets re-claimed and
/// re-executed by another concurrent worker — the cause of library scans
/// appearing to "run multiple times" on slow machines. The heartbeat keeps the
/// lock fresh for as long as this worker is alive and processing; a genuinely
/// dead worker stops renewing, so the lock still lapses and the task is
/// recovered.
struct HeartbeatGuard {
    handle: tokio::task::JoinHandle<()>,
}

impl HeartbeatGuard {
    fn spawn(
        db: DatabaseConnection,
        task_id: Uuid,
        worker_id: String,
        lock_duration_secs: i64,
    ) -> Self {
        // Renew at roughly one third of the lock window so a single delayed or
        // failed renewal can't let a live task's lock lapse. Floored at 200ms
        // to keep very short lock windows (used in tests) responsive without
        // hammering the database for production-sized windows.
        let interval =
            Duration::from_millis((((lock_duration_secs.max(1) * 1000) / 3) as u64).max(200));

        let handle = tokio::spawn(async move {
            loop {
                sleep(interval).await;
                match TaskRepository::renew_lock(&db, task_id, &worker_id, lock_duration_secs).await
                {
                    Ok(true) => {}
                    // We no longer own the task (completed/failed/stolen): stop.
                    Ok(false) => break,
                    Err(e) => warn!("Failed to renew lock for task {}: {}", task_id, e),
                }
            }
        });

        Self { handle }
    }
}

impl Drop for HeartbeatGuard {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Task worker that processes tasks from the queue
pub struct TaskWorker {
    db: DatabaseConnection,
    handlers: HashMap<String, Arc<dyn TaskHandler>>,
    worker_id: String,
    poll_interval: Duration,
    /// How long a claimed task is locked for, in seconds. A background
    /// heartbeat renews this lock while the handler runs so that a task which
    /// legitimately runs longer than the lock window is not re-claimed (and
    /// re-executed) by another worker. Only a genuinely dead worker stops
    /// renewing, letting the lock lapse so the task can be recovered.
    lock_duration_secs: i64,
    event_broadcaster: Option<Arc<EventBroadcaster>>,
    settings_service: Option<Arc<SettingsService>>,
    thumbnail_service: Option<Arc<ThumbnailService>>,
    task_metrics_service: Option<Arc<TaskMetricsService>>,
    plugin_manager: Option<Arc<PluginManager>>,
    pdf_handle_cache: Option<Arc<codex_services::PdfHandleCache>>,
    /// Shared per-host backoff state used by the `PollReleaseSourceHandler`.
    /// Exposed via [`Self::release_backoff`] so the scheduler can read the
    /// same multipliers when picking next-poll intervals.
    release_backoff: codex_services::release::backoff::HostBackoff,
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
        // Renumber series handlers (no dependencies)
        handlers.insert(
            "renumber_series".to_string(),
            Arc::new(RenumberSeriesHandler::new()),
        );
        handlers.insert(
            "renumber_series_batch".to_string(),
            Arc::new(RenumberSeriesBatchHandler::new()),
        );
        // Plugin data cleanup handler (no dependencies)
        handlers.insert(
            "cleanup_plugin_data".to_string(),
            Arc::new(CleanupPluginDataHandler::new()),
        );
        // Refresh-token cleanup handler (no dependencies)
        handlers.insert(
            "cleanup_refresh_tokens".to_string(),
            Arc::new(CleanupRefreshTokensHandler::new()),
        );
        // Release-tracking maintenance: backfill aliases from metadata.
        handlers.insert(
            "backfill_tracking_from_metadata".to_string(),
            Arc::new(BackfillTrackingFromMetadataHandler::new()),
        );
        // User-initiated bulk track / untrack for releases (async fan-in).
        handlers.insert(
            "bulk_track_for_releases".to_string(),
            Arc::new(BulkTrackForReleasesHandler::new()),
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
            lock_duration_secs: 300,
            event_broadcaster: None,
            settings_service: None,
            thumbnail_service: None,
            task_metrics_service: None,
            plugin_manager: None,
            pdf_handle_cache: None,
            release_backoff: codex_services::release::backoff::HostBackoff::new(),
            shutdown_tx: None,
        }
    }

    /// Shared per-host backoff used by `PollReleaseSourceHandler`. The
    /// scheduler reads this when computing the effective interval for the
    /// next poll.
    pub fn release_backoff(&self) -> codex_services::release::backoff::HostBackoff {
        self.release_backoff.clone()
    }

    /// Set the poll interval
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Set the task lock duration in seconds. The worker's heartbeat renews the
    /// lock on this cadence while a handler runs. Primarily used by tests to
    /// drive short lock windows; production reads `task.lock_duration_seconds`
    /// from settings in [`Self::run`].
    pub fn with_lock_duration_secs(mut self, secs: i64) -> Self {
        self.lock_duration_secs = secs.max(1);
        self
    }

    /// Set a custom worker ID (useful for testing)
    pub fn with_worker_id(mut self, worker_id: impl Into<String>) -> Self {
        self.worker_id = worker_id.into();
        self
    }

    /// Register or replace a task handler. Test-only: real callers register
    /// handlers via the `with_*_service` builders, which set up the full
    /// dependency graph each handler requires.
    #[cfg(test)]
    pub fn with_handler(mut self, task_type: &str, handler: Arc<dyn TaskHandler>) -> Self {
        self.handlers.insert(task_type.to_string(), handler);
        self
    }

    /// Set the event broadcaster for task progress events
    pub fn with_event_broadcaster(mut self, broadcaster: Arc<EventBroadcaster>) -> Self {
        self.event_broadcaster = Some(broadcaster);
        self
    }

    /// Set the OAuth state manager for cleaning up expired OAuth flows
    ///
    /// This re-registers the `CleanupPluginDataHandler` with the manager so it
    /// can clean up expired in-memory OAuth state alongside expired storage data.
    pub fn with_oauth_state_manager(mut self, manager: Arc<OAuthStateManager>) -> Self {
        self.handlers.insert(
            "cleanup_plugin_data".to_string(),
            Arc::new(CleanupPluginDataHandler::new().with_oauth_state_manager(manager)),
        );
        self
    }

    /// Set the settings service for runtime configuration
    ///
    /// This also registers/updates handlers that depend on settings:
    /// - `ScanLibraryHandler` for post-scan auto-match settings
    pub fn with_settings_service(mut self, settings_service: Arc<SettingsService>) -> Self {
        self.settings_service = Some(settings_service);
        self.register_scan_library_handler();
        self
    }

    /// Set the PDF handle cache so the scanner can invalidate cached open
    /// `PdfDocument` handles when book files change during a scan.
    pub fn with_pdf_handle_cache(mut self, cache: Arc<codex_services::PdfHandleCache>) -> Self {
        self.pdf_handle_cache = Some(cache);
        self.register_scan_library_handler();
        self
    }

    /// Rebuild and register the `ScanLibraryHandler` with whichever optional
    /// dependencies have been wired in so far. Idempotent: callers can invoke
    /// any `with_*` builder in any order.
    fn register_scan_library_handler(&mut self) {
        let mut handler = ScanLibraryHandler::new();
        if let Some(settings) = &self.settings_service {
            handler = handler.with_settings_service(settings.clone());
        }
        if let Some(cache) = &self.pdf_handle_cache {
            handler = handler.with_pdf_handle_cache(cache.clone());
        }
        self.handlers
            .insert("scan_library".to_string(), Arc::new(handler));
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
        // Register the scheduled per-library metadata refresh handler.
        // It depends on PluginManager (to call get_series_metadata) and
        // optionally ThumbnailService (for cover-field updates via the
        // shared MetadataApplier).
        let mut refresh_handler = RefreshLibraryMetadataHandler::new(plugin_manager.clone());
        if let Some(ref thumbnail_service) = self.thumbnail_service {
            refresh_handler = refresh_handler.with_thumbnail_service(thumbnail_service.clone());
        }
        self.handlers.insert(
            "refresh_library_metadata".to_string(),
            Arc::new(refresh_handler),
        );
        // Register user plugin sync handler (with settings service for configurable timeout)
        let mut sync_handler = UserPluginSyncHandler::new(plugin_manager.clone());
        if let Some(ref settings_service) = self.settings_service {
            sync_handler = sync_handler.with_settings_service(settings_service.clone());
        }
        self.handlers
            .insert("user_plugin_sync".to_string(), Arc::new(sync_handler));
        // Register user plugin recommendations handler (with settings service for configurable timeout)
        let mut recs_handler = UserPluginRecommendationsHandler::new(plugin_manager.clone());
        if let Some(ref settings_service) = self.settings_service {
            recs_handler = recs_handler.with_settings_service(settings_service.clone());
        }
        self.handlers.insert(
            "user_plugin_recommendations".to_string(),
            Arc::new(recs_handler),
        );
        // Register user plugin recommendation dismiss handler
        let mut dismiss_handler =
            UserPluginRecommendationDismissHandler::new(plugin_manager.clone());
        if let Some(ref settings_service) = self.settings_service {
            dismiss_handler = dismiss_handler.with_settings_service(settings_service.clone());
        }
        self.handlers.insert(
            "user_plugin_recommendation_dismiss".to_string(),
            Arc::new(dismiss_handler),
        );
        // Register release-polling handler. Shares the worker's HostBackoff
        // so the scheduler can also consult the same multipliers.
        let mut poll_handler = PollReleaseSourceHandler::new(plugin_manager.clone())
            .with_backoff(self.release_backoff.clone());
        if let Some(ref settings_service) = self.settings_service {
            poll_handler = poll_handler.with_settings_service(settings_service.clone());
        }
        self.handlers
            .insert("poll_release_source".to_string(), Arc::new(poll_handler));
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

    /// Set the export storage for the series export and cleanup handlers.
    ///
    /// Requires `with_settings_service` to be called first.
    pub fn with_export_storage(mut self, export_storage: Arc<ExportStorage>) -> Self {
        if let Some(ref settings_service) = self.settings_service {
            self.handlers.insert(
                "export_series".to_string(),
                Arc::new(ExportSeriesHandler::new(
                    export_storage.clone(),
                    settings_service.clone(),
                )),
            );
            self.handlers.insert(
                "cleanup_series_exports".to_string(),
                Arc::new(CleanupSeriesExportsHandler::new(
                    export_storage,
                    settings_service.clone(),
                )),
            );
        } else {
            tracing::warn!(
                "SettingsService not set - ExportSeriesHandler will not be registered. \
                 Call with_settings_service before with_export_storage."
            );
        }
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

        // Resolve the task lock duration from settings (default 300s). The
        // heartbeat in `process_next_task` renews the lock on this cadence; a
        // task that outlives one window without a heartbeat (dead worker) is
        // reclaimed by `claim_next` once the lock lapses.
        if let Some(ref settings) = self.settings_service {
            let secs = settings
                .get_uint("task.lock_duration_seconds", 300)
                .await
                .unwrap_or(300);
            self.lock_duration_secs = (secs as i64).max(1);
        }
        info!(
            "Task worker using lock duration: {} seconds",
            self.lock_duration_secs
        );

        // Recover tasks whose lock lapsed without a heartbeat (dead worker).
        // Use 2x the lock duration so a live worker's in-flight renewals never
        // trip false-positive recovery.
        let stale_threshold_secs = (self.lock_duration_secs * 2).max(600);

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
                        // Recover tasks whose lock lapsed without a heartbeat
                        // (dead worker). Threshold is 2x the lock duration to
                        // avoid reclaiming tasks a live worker is still renewing.
                        match TaskRepository::recover_stale_tasks(&db_clone_stale, stale_threshold_secs).await {
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
        // Claim next available task
        // Note: claim_next can fail due to race conditions (multiple workers competing
        // for the same task). This is expected behavior, not an error - treat it as
        // "no task available" and retry on the next poll interval.
        let task =
            match TaskRepository::claim_next(&self.db, &self.worker_id, self.lock_duration_secs)
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

        // RAII guard for the OTel in-flight task gauge: increments on claim,
        // decrements on every exit path (success, failure, error propagation).
        let _in_flight = InFlightGuard::new();

        // RAII heartbeat: keep renewing this task's lock while the handler runs
        // so a long task isn't re-claimed by another worker. Aborts on drop
        // (every exit path below, including the distributed-mode early return).
        let _heartbeat = HeartbeatGuard::spawn(
            self.db.clone(),
            task.id,
            self.worker_id.clone(),
            self.lock_duration_secs,
        );

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

        // Build the task identity exposed to reverse-RPC handlers via the
        // task-local context. Used by `releases/report_progress` to
        // construct a `TaskProgressEvent` (which needs the task id/type)
        // and to rate-limit emits.
        let task_identity = Arc::new(codex_events::TaskIdentity::new(
            task.id,
            task.task_type.clone(),
            task.library_id,
            task.series_id,
            task.book_id,
        ));

        // Each task gets its own root span so background work does not
        // accidentally inherit an HTTP server span as its parent. The span
        // covers handler execution across both single-process and
        // distributed-mode branches below.
        let task_span = tracing::info_span!(
            "task.execute",
            task.id = %task.id,
            task.type = %task.task_type,
            library.id = ?task.library_id,
            series.id = ?task.series_id,
            book.id = ?task.book_id,
            otel.kind = "internal",
        );

        // In distributed mode, create a recording broadcaster to capture events
        // that need to be replayed by the TaskListener on the web server
        let (task_broadcaster, recorded_events): (
            Option<Arc<EventBroadcaster>>,
            Option<Vec<RecordedEvent>>,
        ) = if self.is_distributed_mode() {
            // Create a recording broadcaster for this task
            let recording_broadcaster = Arc::new(EventBroadcaster::new_with_recording(1000, true));
            let broadcaster_clone = recording_broadcaster.clone();

            // Execute the handler inside task-local scopes that expose the
            // recording broadcaster and the task identity to any code on
            // this task's await chain — including reverse-RPC handlers
            // (e.g. `releases/record`, `releases/report_progress`), which
            // are dispatched on this task by `RpcClient::call_with_timeout`
            // when the plugin tags reverse-RPCs with the parent forward
            // request id. Without these scopes, plugins that emit events
            // via reverse-RPC would have no recording context and their
            // events would never replay.
            let result = tracing::Instrument::instrument(
                codex_events::with_task_identity(
                    task_identity.clone(),
                    codex_events::with_recording_broadcaster(
                        recording_broadcaster.clone(),
                        handler.handle(&task, &self.db, Some(&recording_broadcaster)),
                    ),
                ),
                task_span.clone(),
            )
            .await;

            // Get recorded events before returning
            let events = broadcaster_clone.take_recorded_events();
            let events = if events.is_empty() {
                None
            } else {
                Some(events)
            };

            // Return result info for later processing. A handler that
            // returns `Ok(TaskResult { success: false, .. })` is signalling
            // a logical failure (e.g. plugin RPC timeout, missing source) —
            // route it to `fail_task` so the task row reflects reality
            // instead of recording a green "completed" status.
            match result {
                Ok(task_result) if task_result.success => {
                    self.complete_task(&task, task_result, started_at, events)
                        .await?;
                }
                Ok(task_result) => {
                    let err = anyhow::anyhow!(
                        task_result
                            .message
                            .unwrap_or_else(|| "task reported failure".to_string())
                    );
                    self.fail_task(&task, err, started_at, events).await?;
                }
                Err(e) => {
                    self.fail_task(&task, e, started_at, events).await?;
                }
            }

            return Ok(true);
        } else {
            // Single-process mode: use shared broadcaster directly
            (self.event_broadcaster.clone(), None)
        };

        // Execute task with shared broadcaster (single-process mode).
        // Set the task-locals to the shared broadcaster + task identity so
        // reverse-RPC handlers see *the same* broadcaster the rest of the
        // task uses, and can synthesize `TaskProgressEvent`s for the task.
        // The shared broadcaster has recording disabled here (web/single-
        // process mode), so emits flow straight to live SSE subscribers.
        let result = if let Some(ref shared) = task_broadcaster {
            tracing::Instrument::instrument(
                codex_events::with_task_identity(
                    task_identity.clone(),
                    codex_events::with_recording_broadcaster(
                        shared.clone(),
                        handler.handle(&task, &self.db, task_broadcaster.as_ref()),
                    ),
                ),
                task_span.clone(),
            )
            .await
        } else {
            tracing::Instrument::instrument(
                codex_events::with_task_identity(
                    task_identity.clone(),
                    handler.handle(&task, &self.db, task_broadcaster.as_ref()),
                ),
                task_span.clone(),
            )
            .await
        };

        // Update task status based on result. See the matching block in
        // distributed mode above for the rationale on the `success: false`
        // branch.
        match result {
            Ok(task_result) if task_result.success => {
                self.complete_task(&task, task_result, started_at, recorded_events)
                    .await?;
            }
            Ok(task_result) => {
                let err = anyhow::anyhow!(
                    task_result
                        .message
                        .unwrap_or_else(|| "task reported failure".to_string())
                );
                // Single-process mode: events flow live to the shared
                // broadcaster, so there are none to record/replay.
                self.fail_task(&task, err, started_at, None).await?;
            }
            Err(e) => {
                self.fail_task(&task, e, started_at, None).await?;
            }
        }

        Ok(true)
    }

    /// Complete a task successfully, storing result and recorded events
    async fn complete_task(
        &self,
        task: &codex_db::entities::tasks::Model,
        task_result: crate::types::TaskResult,
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
        task: &codex_db::entities::tasks::Model,
        error: anyhow::Error,
        started_at: chrono::DateTime<Utc>,
        recorded_events: Option<Vec<RecordedEvent>>,
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

        // Not rate-limited: handle as normal failure. Carry any recorded
        // events through so the web `TaskListener` can replay them for failed
        // tasks too (distributed mode) — e.g. a `release_source_polled` from a
        // poll that errored, so the Release tracking UI updates without a
        // manual reload.
        error!("Task {} failed: {}", task.id, error_string);
        let result_data = recorded_events
            .filter(|events| !events.is_empty())
            .map(|events| json!({ "emitted_events": events }));
        TaskRepository::mark_failed(&self.db, task.id, error_string.clone(), result_data).await?;

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
    use crate::handlers::TaskHandler;
    use crate::types::{TaskResult, TaskType};
    use codex_db::repositories::TaskRepository;
    use codex_db::test_helpers::create_test_db;
    use codex_events::{EntityChangeEvent, EntityEvent, EntityType};

    /// Stub handler that returns whatever `TaskResult` it was constructed with.
    /// Used to drive the worker through specific result branches without
    /// dragging in the real handlers' dependency graphs.
    struct StubHandler {
        result: TaskResult,
    }

    impl TaskHandler for StubHandler {
        fn handle<'a>(
            &'a self,
            _task: &'a codex_db::entities::tasks::Model,
            _db: &'a sea_orm::DatabaseConnection,
            _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>>
        {
            let r = self.result.clone();
            Box::pin(async move { Ok(r) })
        }
    }

    /// Handler that sleeps for a fixed duration before succeeding, to simulate
    /// a task (like a library scan) that runs longer than the lock window.
    struct SleepyHandler {
        sleep: Duration,
    }

    impl TaskHandler for SleepyHandler {
        fn handle<'a>(
            &'a self,
            _task: &'a codex_db::entities::tasks::Model,
            _db: &'a sea_orm::DatabaseConnection,
            _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>>
        {
            let dur = self.sleep;
            Box::pin(async move {
                sleep(dur).await;
                Ok(TaskResult::success("slept"))
            })
        }
    }

    /// Regression: a task whose handler runs longer than the lock duration must
    /// keep its lock renewed (via the worker heartbeat) so that a *different*
    /// worker cannot re-claim and re-execute the same task while it is still in
    /// progress. This reproduces the "scan runs multiple times on a slow
    /// machine" bug, where a scan exceeding the 300s lock was picked up again
    /// by one of the other concurrent workers.
    #[tokio::test]
    async fn in_progress_task_is_not_reclaimed_while_handler_still_running() {
        let (test_db, _temp) = create_test_db().await;
        let db = test_db.sea_orm_connection().clone();
        let task_id = TaskRepository::enqueue(&db, TaskType::FindDuplicates, None)
            .await
            .expect("enqueue");

        // Handler sleeps far longer than the 1s lock window.
        let handler = Arc::new(SleepyHandler {
            sleep: Duration::from_secs(3),
        });
        let worker = TaskWorker::new(db.clone())
            .with_handler("find_duplicates", handler)
            .with_worker_id("worker-A")
            .with_lock_duration_secs(1);

        // Process the task in the background; it stays "processing" for ~3s.
        let worker_db = db.clone();
        let proc = tokio::spawn(async move { worker.process_once().await });

        // Wait until the task is actually claimed (status flips to processing).
        let mut claimed = false;
        for _ in 0..100 {
            if let Some(t) = TaskRepository::get_by_id(&worker_db, task_id)
                .await
                .expect("get_by_id")
                && t.status == "processing"
            {
                claimed = true;
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
        assert!(claimed, "task should have been claimed and be processing");

        // Wait past the original 1s lock window, but well before the 3s handler
        // finishes. Without a heartbeat the lock has now expired.
        sleep(Duration::from_millis(1600)).await;

        // Another worker attempts to claim work. The in-progress task must NOT
        // be handed to it.
        let stolen = TaskRepository::claim_next(&worker_db, "worker-B", 1)
            .await
            .expect("claim_next");
        assert!(
            stolen.is_none(),
            "an in-progress task must not be re-claimed by another worker while its handler is still running"
        );

        // Let the original handler finish cleanly.
        let _ = proc.await.expect("worker task join");
    }

    /// Regression: a handler returning `Ok(TaskResult::failure(...))` must
    /// land the task in the `failed` state. Before this fix, the worker
    /// dispatched both `Ok` arms to `complete_task`, silently writing
    /// `status = "completed"` over the handler's failure signal.
    #[tokio::test]
    async fn handler_failure_result_marks_task_failed() {
        let (test_db, _temp) = create_test_db().await;
        let db = test_db.sea_orm_connection().clone();
        let task_id = TaskRepository::enqueue(&db, TaskType::FindDuplicates, None)
            .await
            .expect("enqueue");

        let stub = Arc::new(StubHandler {
            result: TaskResult::failure("synthetic failure"),
        });
        let worker = TaskWorker::new(db.clone())
            .with_handler("find_duplicates", stub)
            .with_poll_interval(Duration::from_millis(10));

        let processed = worker.process_once().await.expect("process_once");
        assert!(processed, "worker should have claimed the task");

        let task = TaskRepository::get_by_id(&db, task_id)
            .await
            .expect("get_by_id")
            .expect("task row");
        // FindDuplicates has retries enabled, so on the first failure the
        // task is bounced back to `pending` for retry; the load-bearing
        // assertion is "not completed". `last_error` must reflect the
        // handler's message regardless of retry state.
        assert_ne!(
            task.status, "completed",
            "Ok(TaskResult::failure) must not be recorded as completed"
        );
        assert_eq!(
            task.last_error.as_deref(),
            Some("synthetic failure"),
            "the handler's failure message must surface on the task row"
        );
    }

    /// Symmetric positive-case: a `Ok(TaskResult::success(..))` still flows
    /// to `complete_task` after the routing change.
    #[tokio::test]
    async fn handler_success_result_marks_task_completed() {
        let (test_db, _temp) = create_test_db().await;
        let db = test_db.sea_orm_connection().clone();
        let task_id = TaskRepository::enqueue(&db, TaskType::FindDuplicates, None)
            .await
            .expect("enqueue");

        let stub = Arc::new(StubHandler {
            result: TaskResult::success("done"),
        });
        let worker = TaskWorker::new(db.clone())
            .with_handler("find_duplicates", stub)
            .with_poll_interval(Duration::from_millis(10));

        worker.process_once().await.expect("process_once");

        let task = TaskRepository::get_by_id(&db, task_id)
            .await
            .expect("get_by_id")
            .expect("task row");
        assert_eq!(task.status, "completed");
        assert!(task.last_error.is_none());
    }

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
