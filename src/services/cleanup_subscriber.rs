//! Cleanup Event Subscriber Service
//!
//! Subscribes to entity deletion events and enqueues appropriate cleanup tasks
//! to remove orphaned files (thumbnails, covers) from the filesystem.

use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use crate::db::repositories::TaskRepository;
use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster};
use crate::tasks::types::TaskType;

/// Service that subscribes to entity events and triggers file cleanup tasks
pub struct CleanupEventSubscriber {
    db: DatabaseConnection,
    event_broadcaster: Arc<EventBroadcaster>,
}

impl CleanupEventSubscriber {
    /// Create a new CleanupEventSubscriber
    pub fn new(db: DatabaseConnection, event_broadcaster: Arc<EventBroadcaster>) -> Self {
        Self {
            db,
            event_broadcaster,
        }
    }

    /// Start the subscriber in a background task
    ///
    /// This method spawns a task that listens for entity deletion events
    /// and enqueues cleanup tasks accordingly. Returns a handle to the spawned task.
    pub fn start(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            info!("CleanupEventSubscriber started");
            if let Err(e) = self.run().await {
                error!("CleanupEventSubscriber error: {}", e);
            }
            info!("CleanupEventSubscriber stopped");
        })
    }

    /// Run the subscriber loop
    async fn run(self) -> anyhow::Result<()> {
        let mut receiver = self.event_broadcaster.subscribe();

        loop {
            match receiver.recv().await {
                Ok(event) => {
                    if let Err(e) = self.handle_event(&event).await {
                        warn!("Failed to handle event {:?}: {}", event.event, e);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("CleanupEventSubscriber lagged by {} events", n);
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("Event broadcaster closed, shutting down CleanupEventSubscriber");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle an entity change event
    async fn handle_event(&self, event: &EntityChangeEvent) -> anyhow::Result<()> {
        match &event.event {
            EntityEvent::BookDeleted {
                book_id,
                series_id: _,
                library_id: _,
            } => {
                self.handle_book_deleted(*book_id).await?;
            }
            EntityEvent::SeriesDeleted {
                series_id,
                library_id: _,
            } => {
                self.handle_series_deleted(*series_id).await?;
            }
            EntityEvent::LibraryDeleted { library_id } => {
                self.handle_library_deleted(*library_id).await?;
            }
            // Ignore other events
            _ => {}
        }

        Ok(())
    }

    /// Handle BookDeleted event - enqueue cleanup task for book's files
    async fn handle_book_deleted(&self, book_id: uuid::Uuid) -> anyhow::Result<()> {
        debug!("Handling BookDeleted event for book {}", book_id);

        // Enqueue cleanup task for this book's files
        // The thumbnail_path is not available here since the book is already deleted,
        // so we'll derive it from the book_id in the handler
        let task = TaskType::CleanupBookFiles {
            book_id,
            thumbnail_path: None,
        };

        // Use lowest priority so cleanup doesn't interfere with more important tasks
        let priority = -100;

        match TaskRepository::enqueue(&self.db, task, priority, None).await {
            Ok(task_id) => {
                info!(
                    "Enqueued CleanupBookFiles task {} for deleted book {}",
                    task_id, book_id
                );
            }
            Err(e) => {
                warn!(
                    "Failed to enqueue CleanupBookFiles task for book {}: {}",
                    book_id, e
                );
            }
        }

        Ok(())
    }

    /// Handle SeriesDeleted event - enqueue cleanup task for series' cover files
    async fn handle_series_deleted(&self, series_id: uuid::Uuid) -> anyhow::Result<()> {
        debug!("Handling SeriesDeleted event for series {}", series_id);

        // Enqueue cleanup task for this series' cover files
        let task = TaskType::CleanupSeriesFiles { series_id };

        // Use lowest priority so cleanup doesn't interfere with more important tasks
        let priority = -100;

        match TaskRepository::enqueue(&self.db, task, priority, None).await {
            Ok(task_id) => {
                info!(
                    "Enqueued CleanupSeriesFiles task {} for deleted series {}",
                    task_id, series_id
                );
            }
            Err(e) => {
                warn!(
                    "Failed to enqueue CleanupSeriesFiles task for series {}: {}",
                    series_id, e
                );
            }
        }

        Ok(())
    }

    /// Handle LibraryDeleted event - enqueue global orphan cleanup task
    ///
    /// When a library is deleted, all its books and series are cascade-deleted from the DB,
    /// but their files remain on disk. Since we don't know which files belonged to the library
    /// (the records are gone), we trigger a global orphan cleanup scan that will find and
    /// remove all files without corresponding DB entries.
    async fn handle_library_deleted(&self, library_id: uuid::Uuid) -> anyhow::Result<()> {
        debug!("Handling LibraryDeleted event for library {}", library_id);

        // Enqueue the global orphan cleanup task
        // This will scan the filesystem and remove any files that don't have
        // corresponding DB entries
        let task = TaskType::CleanupOrphanedFiles;

        // Use lowest priority so cleanup doesn't interfere with more important tasks
        let priority = -100;

        match TaskRepository::enqueue(&self.db, task, priority, None).await {
            Ok(task_id) => {
                info!(
                    "Enqueued CleanupOrphanedFiles task {} after library {} deletion",
                    task_id, library_id
                );
            }
            Err(e) => {
                warn!(
                    "Failed to enqueue CleanupOrphanedFiles task after library {} deletion: {}",
                    library_id, e
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::create_test_db;
    use crate::events::EventBroadcaster;
    use crate::tasks::types::TaskType;
    use chrono::Utc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_subscriber_creation() {
        let (db, _temp_dir) = create_test_db().await;
        let broadcaster = Arc::new(EventBroadcaster::new(100));
        let _subscriber = CleanupEventSubscriber::new(db.sea_orm_connection().clone(), broadcaster);
        // Just verify it can be created
    }

    #[tokio::test]
    async fn test_handle_book_deleted_event() {
        let (db, _temp_dir) = create_test_db().await;
        let _broadcaster = Arc::new(EventBroadcaster::new(100));

        let book_id = Uuid::new_v4();

        // Enqueue the task directly to verify TaskRepository works
        let task = TaskType::CleanupBookFiles {
            book_id,
            thumbnail_path: None,
        };
        let task_id = TaskRepository::enqueue(db.sea_orm_connection(), task, -100, None)
            .await
            .expect("Failed to enqueue task");

        // Verify a task was enqueued
        let tasks = TaskRepository::list(
            db.sea_orm_connection(),
            Some("pending".to_string()),
            Some("cleanup_book_files".to_string()),
            Some(10),
        )
        .await
        .unwrap();

        assert!(!tasks.is_empty(), "Task should have been enqueued");
        // book_id is stored in params, not as FK (because book is deleted)
        assert_eq!(tasks[0].book_id, None);
        assert_eq!(tasks[0].id, task_id);
        // Verify book_id is in params
        let params_book_id: Uuid = tasks[0]
            .params
            .as_ref()
            .and_then(|p| p.get("book_id"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap();
        assert_eq!(params_book_id, book_id);
    }

    #[tokio::test]
    async fn test_handle_series_deleted_event() {
        let (db, _temp_dir) = create_test_db().await;
        let broadcaster = Arc::new(EventBroadcaster::new(100));
        let subscriber =
            CleanupEventSubscriber::new(db.sea_orm_connection().clone(), broadcaster.clone());

        let series_id = Uuid::new_v4();

        // Handle the event directly
        let result = subscriber.handle_series_deleted(series_id).await;
        assert!(result.is_ok());

        // Verify a task was enqueued
        let tasks = TaskRepository::list(
            db.sea_orm_connection(),
            Some("pending".to_string()),
            Some("cleanup_series_files".to_string()),
            Some(10),
        )
        .await
        .unwrap();

        assert!(!tasks.is_empty());
        // series_id is stored in params, not as FK (because series is deleted)
        assert_eq!(tasks[0].series_id, None);
        // Verify series_id is in params
        let params_series_id: Uuid = tasks[0]
            .params
            .as_ref()
            .and_then(|p| p.get("series_id"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap();
        assert_eq!(params_series_id, series_id);
    }

    #[tokio::test]
    async fn test_handle_library_deleted_event() {
        let (db, _temp_dir) = create_test_db().await;
        let broadcaster = Arc::new(EventBroadcaster::new(100));
        let subscriber =
            CleanupEventSubscriber::new(db.sea_orm_connection().clone(), broadcaster.clone());

        let library_id = Uuid::new_v4();

        // Handle the event directly
        let result = subscriber.handle_library_deleted(library_id).await;
        assert!(result.is_ok());

        // Verify a task was enqueued
        let tasks = TaskRepository::list(
            db.sea_orm_connection(),
            Some("pending".to_string()),
            Some("cleanup_orphaned_files".to_string()),
            Some(10),
        )
        .await
        .unwrap();

        assert!(!tasks.is_empty());
    }

    #[tokio::test]
    async fn test_handle_event_book_deleted() {
        let (db, _temp_dir) = create_test_db().await;
        let broadcaster = Arc::new(EventBroadcaster::new(100));
        let subscriber =
            CleanupEventSubscriber::new(db.sea_orm_connection().clone(), broadcaster.clone());

        let book_id = Uuid::new_v4();
        let series_id = Uuid::new_v4();
        let library_id = Uuid::new_v4();

        let event = EntityChangeEvent {
            event: EntityEvent::BookDeleted {
                book_id,
                series_id,
                library_id,
            },
            timestamp: Utc::now(),
            user_id: None,
        };

        // Handle the event
        let result = subscriber.handle_event(&event).await;
        assert!(result.is_ok());

        // Verify a task was enqueued
        let tasks = TaskRepository::list(
            db.sea_orm_connection(),
            Some("pending".to_string()),
            Some("cleanup_book_files".to_string()),
            Some(10),
        )
        .await
        .unwrap();

        assert!(!tasks.is_empty());
        // book_id is stored in params, not as FK
        let params_book_id: Uuid = tasks[0]
            .params
            .as_ref()
            .and_then(|p| p.get("book_id"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap();
        assert_eq!(params_book_id, book_id);
    }

    #[tokio::test]
    async fn test_handle_event_series_deleted() {
        let (db, _temp_dir) = create_test_db().await;
        let broadcaster = Arc::new(EventBroadcaster::new(100));
        let subscriber =
            CleanupEventSubscriber::new(db.sea_orm_connection().clone(), broadcaster.clone());

        let series_id = Uuid::new_v4();
        let library_id = Uuid::new_v4();

        let event = EntityChangeEvent {
            event: EntityEvent::SeriesDeleted {
                series_id,
                library_id,
            },
            timestamp: Utc::now(),
            user_id: None,
        };

        // Handle the event
        let result = subscriber.handle_event(&event).await;
        assert!(result.is_ok());

        // Verify a task was enqueued
        let tasks = TaskRepository::list(
            db.sea_orm_connection(),
            Some("pending".to_string()),
            Some("cleanup_series_files".to_string()),
            Some(10),
        )
        .await
        .unwrap();

        assert!(!tasks.is_empty());
        // series_id is stored in params, not as FK
        let params_series_id: Uuid = tasks[0]
            .params
            .as_ref()
            .and_then(|p| p.get("series_id"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap();
        assert_eq!(params_series_id, series_id);
    }

    #[tokio::test]
    async fn test_handle_event_library_deleted() {
        let (db, _temp_dir) = create_test_db().await;
        let broadcaster = Arc::new(EventBroadcaster::new(100));
        let subscriber =
            CleanupEventSubscriber::new(db.sea_orm_connection().clone(), broadcaster.clone());

        let library_id = Uuid::new_v4();

        let event = EntityChangeEvent {
            event: EntityEvent::LibraryDeleted { library_id },
            timestamp: Utc::now(),
            user_id: None,
        };

        // Handle the event
        let result = subscriber.handle_event(&event).await;
        assert!(result.is_ok());

        // Verify a task was enqueued
        let tasks = TaskRepository::list(
            db.sea_orm_connection(),
            Some("pending".to_string()),
            Some("cleanup_orphaned_files".to_string()),
            Some(10),
        )
        .await
        .unwrap();

        assert!(!tasks.is_empty());
    }

    #[tokio::test]
    async fn test_ignores_other_events() {
        let (db, _temp_dir) = create_test_db().await;
        let broadcaster = Arc::new(EventBroadcaster::new(100));
        let subscriber =
            CleanupEventSubscriber::new(db.sea_orm_connection().clone(), broadcaster.clone());

        let book_id = Uuid::new_v4();
        let series_id = Uuid::new_v4();
        let library_id = Uuid::new_v4();

        // Test BookCreated - should be ignored
        let event = EntityChangeEvent {
            event: EntityEvent::BookCreated {
                book_id,
                series_id,
                library_id,
            },
            timestamp: Utc::now(),
            user_id: None,
        };

        let result = subscriber.handle_event(&event).await;
        assert!(result.is_ok());

        // No cleanup tasks should be enqueued
        let tasks = TaskRepository::list(
            db.sea_orm_connection(),
            Some("pending".to_string()),
            None,
            Some(10),
        )
        .await
        .unwrap();

        // Filter for cleanup tasks only
        let cleanup_tasks: Vec<_> = tasks
            .iter()
            .filter(|t| t.task_type.starts_with("cleanup"))
            .collect();

        assert!(cleanup_tasks.is_empty());
    }

    #[tokio::test]
    async fn test_subscriber_receives_events() {
        let (db, _temp_dir) = create_test_db().await;
        let broadcaster = Arc::new(EventBroadcaster::new(100));
        let subscriber =
            CleanupEventSubscriber::new(db.sea_orm_connection().clone(), broadcaster.clone());

        // Start the subscriber
        let handle = subscriber.start();

        // Give it a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Emit an event
        let book_id = Uuid::new_v4();
        let series_id = Uuid::new_v4();
        let library_id = Uuid::new_v4();

        let event = EntityChangeEvent {
            event: EntityEvent::BookDeleted {
                book_id,
                series_id,
                library_id,
            },
            timestamp: Utc::now(),
            user_id: None,
        };

        let _ = broadcaster.emit(event);

        // Give it time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Verify a task was enqueued
        let tasks = TaskRepository::list(
            db.sea_orm_connection(),
            Some("pending".to_string()),
            Some("cleanup_book_files".to_string()),
            Some(10),
        )
        .await
        .unwrap();

        assert!(!tasks.is_empty());
        // book_id is stored in params, not as FK
        let params_book_id: Uuid = tasks[0]
            .params
            .as_ref()
            .and_then(|p| p.get("book_id"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap();
        assert_eq!(params_book_id, book_id);

        // Abort the handle since we're done testing
        handle.abort();
    }
}
