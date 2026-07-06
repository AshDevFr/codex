//! Event broadcaster for entity change notifications
//!
//! TODO: Remove allow(dead_code) once event broadcasting features are fully integrated

#![allow(dead_code)]

use super::types::{EntityChangeEvent, EntityEvent, TaskProgressEvent};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, mpsc};
use tracing::debug;

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
#[derive(Debug)]
pub struct EventBroadcaster {
    entity_sender: broadcast::Sender<EntityChangeEvent>,
    task_sender: broadcast::Sender<TaskProgressEvent>,
    /// Optional event recording for cross-process bridging in distributed deployments
    recorded_events: Option<Arc<RwLock<Vec<RecordedEvent>>>>,
    /// Optional out-of-process sink for task progress events. In distributed
    /// deployments the worker process has no local SSE subscribers, so task
    /// progress is forwarded here (lossy, non-blocking) to be re-published to
    /// the web server via PostgreSQL LISTEN/NOTIFY.
    task_notifier: Option<mpsc::Sender<TaskProgressEvent>>,
    /// Flag to track if the broadcaster has been shut down
    shutdown: Arc<std::sync::atomic::AtomicBool>,
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
            task_notifier: None,
            shutdown: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Attach an out-of-process sink for task progress events.
    ///
    /// When set, every `emit_task` call also forwards a clone of the event to
    /// `notifier`. This is used in distributed worker processes to bridge task
    /// progress to the web server (which has the live SSE subscribers) via
    /// PostgreSQL LISTEN/NOTIFY. Forwarding is lossy and non-blocking: if the
    /// channel is full the event is dropped, which is acceptable for progress.
    pub fn with_task_notifier(mut self, notifier: mpsc::Sender<TaskProgressEvent>) -> Self {
        self.task_notifier = Some(notifier);
        self
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
    // The SendError carries the original event back to the caller; that is
    // tokio's contract, not something we control. The event payload is
    // by-value already and doesn't justify boxing the error variant.
    #[allow(clippy::result_large_err)]
    pub fn emit(
        &self,
        event: EntityChangeEvent,
    ) -> Result<usize, broadcast::error::SendError<EntityChangeEvent>> {
        // Record event if recording is enabled
        if let Some(ref recorded) = self.recorded_events
            && let Ok(mut events) = recorded.write()
        {
            events.push(RecordedEvent {
                event: event.event.clone(),
                timestamp: event.timestamp,
                user_id: event.user_id,
            });
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
    ) -> Result<usize, Box<broadcast::error::SendError<TaskProgressEvent>>> {
        // Forward to the out-of-process sink first (distributed mode). This is
        // lossy by design: a full channel drops the event rather than blocking
        // the task, since progress is a best-effort UI hint.
        if let Some(ref notifier) = self.task_notifier {
            let _ = notifier.try_send(event.clone());
        }

        match self.task_sender.send(event.clone()) {
            Ok(count) => {
                debug!(
                    "Broadcast task event to {} subscribers: task_id={}, type={}, status={:?}",
                    count, event.task_id, event.task_type, event.status
                );
                Ok(count)
            }
            Err(e) => {
                // No active receivers is expected, not an error: worker
                // processes have no local task-event subscribers (those live in
                // the web server). Cross-process delivery goes through the
                // task_notifier sink above, so log at debug to avoid spam.
                debug!("No subscribers for task event: {:?}", e);
                Err(Box::new(e))
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

    /// Check if the broadcaster has been shut down
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Shutdown the broadcaster, signaling all SSE streams to close
    ///
    /// This sets a shutdown flag and sends a final event to wake up
    /// any receivers that are waiting. SSE handlers should check
    /// `is_shutdown()` and exit their loops.
    pub fn shutdown(&self) {
        debug!("Shutting down event broadcaster");
        self.shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);

        // Send a dummy event to wake up any receivers waiting on recv()
        // They will then check is_shutdown() and exit
        // We don't care if this fails (no subscribers)
        let _ = self
            .entity_sender
            .send(EntityChangeEvent::shutdown_signal());
        let _ = self.task_sender.send(TaskProgressEvent::shutdown_signal());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EntityEvent, EntityType};
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

    #[tokio::test]
    async fn test_emit_task_forwards_to_notifier_without_subscribers() {
        // Simulates a distributed worker: no local task-event subscribers, but
        // an out-of-process sink is attached. The event must reach the sink even
        // though the local broadcast has zero receivers.
        let (tx, mut rx) = mpsc::channel(8);
        let broadcaster = EventBroadcaster::new(100).with_task_notifier(tx);

        let task_id = Uuid::new_v4();
        let event = TaskProgressEvent::progress(
            task_id,
            "scan_library",
            3,
            10,
            Some("scanning".to_string()),
            None,
            None,
            None,
        );

        // No local subscribers -> emit_task returns Err, but that is expected
        // and must not prevent forwarding to the sink.
        assert!(broadcaster.emit_task(event).is_err());

        let forwarded = rx.recv().await.expect("event should reach the sink");
        assert_eq!(forwarded.task_id, task_id);
        assert_eq!(forwarded.task_type, "scan_library");
    }

    #[tokio::test]
    async fn test_emit_task_without_notifier_is_noop_forward() {
        // Without a notifier the method still works and simply reports no
        // subscribers; nothing to forward.
        let broadcaster = EventBroadcaster::new(100);
        let event = TaskProgressEvent::started(Uuid::new_v4(), "scan_library", None, None, None);
        assert!(broadcaster.emit_task(event).is_err());
    }

    #[tokio::test]
    async fn test_shutdown_sets_flag() {
        let broadcaster = EventBroadcaster::new(100);
        assert!(!broadcaster.is_shutdown());

        broadcaster.shutdown();
        assert!(broadcaster.is_shutdown());
    }

    #[tokio::test]
    async fn test_shutdown_wakes_receivers() {
        let broadcaster = EventBroadcaster::new(100);
        let mut entity_receiver = broadcaster.subscribe();
        let mut task_receiver = broadcaster.subscribe_tasks();

        // Shutdown should send signals that wake up waiting receivers
        broadcaster.shutdown();

        // Receivers should get the shutdown signal
        let entity_event = entity_receiver.recv().await.unwrap();
        assert!(entity_event.is_shutdown());

        let task_event = task_receiver.recv().await.unwrap();
        assert!(task_event.is_shutdown());
    }
}
