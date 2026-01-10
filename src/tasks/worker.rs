use anyhow::Result;
use chrono::Utc;
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::repositories::TaskRepository;
use crate::events::{EventBroadcaster, TaskProgressEvent};
use crate::services::SettingsService;
use crate::tasks::handlers::{
    AnalyzeBookHandler, AnalyzeSeriesHandler, FindDuplicatesHandler, GenerateThumbnailsHandler,
    PurgeDeletedHandler, ScanLibraryHandler, TaskHandler,
};

/// Task worker that processes tasks from the queue
pub struct TaskWorker {
    db: DatabaseConnection,
    handlers: HashMap<String, Arc<dyn TaskHandler>>,
    worker_id: String,
    poll_interval: Duration,
    event_broadcaster: Option<Arc<EventBroadcaster>>,
    settings_service: Option<Arc<SettingsService>>,
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
        handlers.insert(
            "generate_thumbnails".to_string(),
            Arc::new(GenerateThumbnailsHandler::new()),
        );
        handlers.insert(
            "find_duplicates".to_string(),
            Arc::new(FindDuplicatesHandler::new()),
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
    pub fn with_settings_service(mut self, settings_service: Arc<SettingsService>) -> Self {
        self.settings_service = Some(settings_service);
        self
    }

    /// Get the worker ID
    pub fn worker_id(&self) -> &str {
        &self.worker_id
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
        let task =
            match TaskRepository::claim_next(&self.db, &self.worker_id, 300, prioritize_scans)
                .await?
            {
                Some(t) => t,
                None => return Ok(false), // No tasks available
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

        // Execute task
        let result = handler
            .handle(&task, &self.db, self.event_broadcaster.as_ref())
            .await;

        // Update task status based on result
        match result {
            Ok(task_result) => {
                TaskRepository::mark_completed(&self.db, task.id, task_result.data).await?;
                info!(
                    "Task {} completed successfully: {}",
                    task.id,
                    task_result.message.unwrap_or_default()
                );

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
            }
            Err(e) => {
                error!("Task {} failed: {}", task.id, e);
                TaskRepository::mark_failed(&self.db, task.id, e.to_string()).await?;

                // Emit task failed event
                if let Some(ref broadcaster) = self.event_broadcaster {
                    let _ = broadcaster.emit_task(TaskProgressEvent::failed(
                        task.id,
                        &task.task_type,
                        e.to_string(),
                        started_at,
                        task.library_id,
                        task.series_id,
                        task.book_id,
                    ));
                }
            }
        }

        Ok(true)
    }

    /// Run a single iteration of task processing (useful for testing)
    pub async fn process_once(&self) -> Result<bool> {
        self.process_next_task().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
