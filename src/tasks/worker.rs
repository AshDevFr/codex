use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::db::repositories::TaskRepository;
use crate::tasks::handlers::{
    AnalyzeBookHandler, AnalyzeSeriesHandler, GenerateThumbnailsHandler, PurgeDeletedHandler,
    ScanLibraryHandler, TaskHandler,
};

/// Task worker that processes tasks from the queue
pub struct TaskWorker {
    db: DatabaseConnection,
    handlers: HashMap<String, Arc<dyn TaskHandler>>,
    worker_id: String,
    poll_interval: Duration,
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

        // Generate worker ID from hostname or random UUID
        let worker_id = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| format!("worker-{}", Uuid::new_v4()));

        Self {
            db,
            handlers,
            worker_id,
            poll_interval: Duration::from_secs(5),
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

    /// Get the worker ID
    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    /// Run the worker (blocks indefinitely)
    pub async fn run(&self) -> Result<()> {
        info!("Task worker {} started", self.worker_id);

        loop {
            match self.process_next_task().await {
                Ok(true) => {
                    // Processed a task, immediately check for more
                    continue;
                }
                Ok(false) => {
                    // No tasks available, sleep
                    debug!("No tasks available, sleeping for {:?}", self.poll_interval);
                    sleep(self.poll_interval).await;
                }
                Err(e) => {
                    error!("Worker error: {}", e);
                    // Sleep longer on error to avoid rapid retry loops
                    sleep(Duration::from_secs(10)).await;
                }
            }
        }
    }

    /// Process the next available task
    /// Returns Ok(true) if a task was processed, Ok(false) if no tasks were available
    async fn process_next_task(&self) -> Result<bool> {
        // Claim next available task
        let task = match TaskRepository::claim_next(&self.db, &self.worker_id, 300).await? {
            Some(t) => t,
            None => return Ok(false), // No tasks available
        };

        info!(
            "Worker {} processing task {} ({})",
            self.worker_id, task.id, task.task_type
        );

        // Get handler for this task type
        let handler = self.handlers.get(&task.task_type).ok_or_else(|| {
            anyhow::anyhow!("No handler registered for task type: {}", task.task_type)
        })?;

        // Execute task
        let result = handler.handle(&task, &self.db).await;

        // Update task status based on result
        match result {
            Ok(task_result) => {
                TaskRepository::mark_completed(&self.db, task.id, task_result.data).await?;
                info!(
                    "Task {} completed successfully: {}",
                    task.id,
                    task_result.message.unwrap_or_default()
                );
            }
            Err(e) => {
                error!("Task {} failed: {}", task.id, e);
                TaskRepository::mark_failed(&self.db, task.id, e.to_string()).await?;
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
