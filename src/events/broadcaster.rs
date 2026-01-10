use super::types::{EntityChangeEvent, EntityEvent, TaskProgressEvent};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tracing::{debug, warn};

/// Recorded event for cross-process replay in distributed deployments
///
/// When tasks run in a separate worker process, they cannot directly emit events
/// to the web server's broadcaster. Instead, events are recorded during task
/// execution and stored in the task result. The TaskListener then replays these
/// events when the task completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedEvent {
    /// The entity event that was emitted
    pub event: EntityEvent,
    /// When the event was originally emitted
    pub timestamp: DateTime<Utc>,
    /// User who triggered the change (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<uuid::Uuid>,
}

/// Event broadcaster for entity change notifications
///
/// In single-process mode, events are broadcast directly to subscribers.
/// In distributed mode (with recording enabled), events are also recorded
/// for later replay by the TaskListener.
#[derive(Debug, Clone)]
pub struct EventBroadcaster {
    entity_sender: broadcast::Sender<EntityChangeEvent>,
    task_sender: broadcast::Sender<TaskProgressEvent>,
    /// Optional event recording for cross-process bridging in distributed deployments
    recorded_events: Option<Arc<RwLock<Vec<RecordedEvent>>>>,
}

impl EventBroadcaster {
    /// Create a new event broadcaster with the specified channel capacity
    ///
    /// This creates a broadcaster without event recording, suitable for
    /// single-process deployments or the web server process.
    pub fn new(capacity: usize) -> Self {
        Self::new_with_recording(capacity, false)
    }

    /// Create a new event broadcaster with optional event recording
    ///
    /// When `record_events` is true, all emitted entity events are recorded
    /// for later retrieval. This is used in distributed worker processes
    /// to capture events that need to be replayed on the web server.
    pub fn new_with_recording(capacity: usize, record_events: bool) -> Self {
        let (entity_sender, _) = broadcast::channel(capacity);
        let (task_sender, _) = broadcast::channel(capacity);
        debug!(
            "Created event broadcaster with capacity {} (recording: {})",
            capacity, record_events
        );
        Self {
            entity_sender,
            task_sender,
            recorded_events: if record_events {
                Some(Arc::new(RwLock::new(Vec::new())))
            } else {
                None
            },
        }
    }

    /// Subscribe to entity change events
    pub fn subscribe(&self) -> broadcast::Receiver<EntityChangeEvent> {
        self.entity_sender.subscribe()
    }

    /// Subscribe to task progress events
    pub fn subscribe_tasks(&self) -> broadcast::Receiver<TaskProgressEvent> {
        self.task_sender.subscribe()
    }

    /// Emit an entity change event to all subscribers
    ///
    /// If event recording is enabled, the event is also recorded for later
    /// retrieval via `take_recorded_events()`.
    ///
    /// Returns the number of receivers that received the event.
    pub fn emit(
        &self,
        event: EntityChangeEvent,
    ) -> Result<usize, broadcast::error::SendError<EntityChangeEvent>> {
        // Record event if recording is enabled
        if let Some(ref recorded) = self.recorded_events {
            if let Ok(mut events) = recorded.write() {
                events.push(RecordedEvent {
                    event: event.event.clone(),
                    timestamp: event.timestamp,
                    user_id: event.user_id,
                });
            }
        }

        // Broadcast to local subscribers
        match self.entity_sender.send(event.clone()) {
            Ok(count) => {
                debug!(
                    "Broadcast entity event to {} subscribers: {:?}",
                    count, event.event
                );
                Ok(count)
            }
            Err(e) => {
                // No subscribers is not an error for recording purposes
                debug!("No subscribers for entity event: {:?}", event.event);
                Err(e)
            }
        }
    }

    /// Emit a task progress event to all subscribers
    /// Returns the number of receivers that received the event
    pub fn emit_task(
        &self,
        event: TaskProgressEvent,
    ) -> Result<usize, broadcast::error::SendError<TaskProgressEvent>> {
        match self.task_sender.send(event.clone()) {
            Ok(count) => {
                debug!(
                    "Broadcast task event to {} subscribers: task_id={}, type={}, status={:?}",
                    count, event.task_id, event.task_type, event.status
                );
                Ok(count)
            }
            Err(e) => {
                warn!("Failed to broadcast task event: {:?}", e);
                Err(e)
            }
        }
    }

    /// Get the number of active entity event subscribers
    pub fn subscriber_count(&self) -> usize {
        self.entity_sender.receiver_count()
    }

    /// Get the number of active task event subscribers
    pub fn task_subscriber_count(&self) -> usize {
        self.task_sender.receiver_count()
    }

    /// Check if event recording is enabled
    pub fn is_recording(&self) -> bool {
        self.recorded_events.is_some()
    }

    /// Take all recorded events, clearing the internal buffer
    ///
    /// This is used after task execution to retrieve events that should
    /// be stored in the task result for later replay.
    ///
    /// Returns an empty vector if recording is not enabled.
    pub fn take_recorded_events(&self) -> Vec<RecordedEvent> {
        if let Some(ref recorded) = self.recorded_events {
            if let Ok(mut events) = recorded.write() {
                std::mem::take(&mut *events)
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }

    /// Get the number of recorded events without clearing them
    pub fn recorded_event_count(&self) -> usize {
        if let Some(ref recorded) = self.recorded_events {
            recorded.read().map(|e| e.len()).unwrap_or(0)
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::{EntityEvent, EntityType};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_broadcaster_creation() {
        let broadcaster = EventBroadcaster::new(100);
        assert_eq!(broadcaster.subscriber_count(), 0);
        assert!(!broadcaster.is_recording());
    }

    #[tokio::test]
    async fn test_broadcaster_with_recording() {
        let broadcaster = EventBroadcaster::new_with_recording(100, true);
        assert!(broadcaster.is_recording());
        assert_eq!(broadcaster.recorded_event_count(), 0);
    }

    #[tokio::test]
    async fn test_subscribe_and_emit() {
        let broadcaster = EventBroadcaster::new(100);
        let mut receiver = broadcaster.subscribe();

        let event = EntityChangeEvent::new(
            EntityEvent::BookCreated {
                book_id: Uuid::new_v4(),
                series_id: Uuid::new_v4(),
                library_id: Uuid::new_v4(),
            },
            None,
        );

        broadcaster.emit(event.clone()).unwrap();

        let received = receiver.recv().await.unwrap();
        assert_eq!(received.library_id(), event.library_id());
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let broadcaster = EventBroadcaster::new(100);
        let mut receiver1 = broadcaster.subscribe();
        let mut receiver2 = broadcaster.subscribe();

        assert_eq!(broadcaster.subscriber_count(), 2);

        let event = EntityChangeEvent::new(
            EntityEvent::SeriesCreated {
                series_id: Uuid::new_v4(),
                library_id: Uuid::new_v4(),
            },
            None,
        );

        let count = broadcaster.emit(event.clone()).unwrap();
        assert_eq!(count, 2);

        let received1 = receiver1.recv().await.unwrap();
        let received2 = receiver2.recv().await.unwrap();

        assert_eq!(received1.library_id(), event.library_id());
        assert_eq!(received2.library_id(), event.library_id());
    }

    #[tokio::test]
    async fn test_event_recording() {
        let broadcaster = EventBroadcaster::new_with_recording(100, true);

        let book_id = Uuid::new_v4();
        let series_id = Uuid::new_v4();
        let library_id = Uuid::new_v4();

        // Emit event (no subscribers, but should still record)
        let event = EntityChangeEvent::new(
            EntityEvent::BookCreated {
                book_id,
                series_id,
                library_id,
            },
            None,
        );

        // emit() returns Err when no subscribers, but event should still be recorded
        let _ = broadcaster.emit(event);

        assert_eq!(broadcaster.recorded_event_count(), 1);

        let recorded = broadcaster.take_recorded_events();
        assert_eq!(recorded.len(), 1);
        assert!(matches!(recorded[0].event, EntityEvent::BookCreated { .. }));

        // After take, count should be 0
        assert_eq!(broadcaster.recorded_event_count(), 0);
    }

    #[tokio::test]
    async fn test_event_recording_multiple_events() {
        let broadcaster = EventBroadcaster::new_with_recording(100, true);

        let library_id = Uuid::new_v4();

        // Emit multiple events
        let _ = broadcaster.emit(EntityChangeEvent::new(
            EntityEvent::BookCreated {
                book_id: Uuid::new_v4(),
                series_id: Uuid::new_v4(),
                library_id,
            },
            None,
        ));

        let _ = broadcaster.emit(EntityChangeEvent::new(
            EntityEvent::CoverUpdated {
                entity_type: EntityType::Book,
                entity_id: Uuid::new_v4(),
                library_id: Some(library_id),
            },
            None,
        ));

        let _ = broadcaster.emit(EntityChangeEvent::new(
            EntityEvent::SeriesCreated {
                series_id: Uuid::new_v4(),
                library_id,
            },
            None,
        ));

        assert_eq!(broadcaster.recorded_event_count(), 3);

        let recorded = broadcaster.take_recorded_events();
        assert_eq!(recorded.len(), 3);

        // Verify event types in order
        assert!(matches!(recorded[0].event, EntityEvent::BookCreated { .. }));
        assert!(matches!(
            recorded[1].event,
            EntityEvent::CoverUpdated { .. }
        ));
        assert!(matches!(
            recorded[2].event,
            EntityEvent::SeriesCreated { .. }
        ));
    }

    #[tokio::test]
    async fn test_no_recording_without_flag() {
        let broadcaster = EventBroadcaster::new(100);

        let _ = broadcaster.emit(EntityChangeEvent::new(
            EntityEvent::BookCreated {
                book_id: Uuid::new_v4(),
                series_id: Uuid::new_v4(),
                library_id: Uuid::new_v4(),
            },
            None,
        ));

        // Should return empty since recording is not enabled
        let recorded = broadcaster.take_recorded_events();
        assert!(recorded.is_empty());
        assert_eq!(broadcaster.recorded_event_count(), 0);
    }

    #[test]
    fn test_recorded_event_serialization() {
        let recorded = RecordedEvent {
            event: EntityEvent::CoverUpdated {
                entity_type: EntityType::Book,
                entity_id: Uuid::new_v4(),
                library_id: Some(Uuid::new_v4()),
            },
            timestamp: Utc::now(),
            user_id: None,
        };

        // Serialize to JSON
        let json = serde_json::to_string(&recorded).unwrap();
        assert!(json.contains("cover_updated"));

        // Deserialize back
        let deserialized: RecordedEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized.event,
            EntityEvent::CoverUpdated { .. }
        ));
    }
}
