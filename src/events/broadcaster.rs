use super::types::EntityChangeEvent;
use tokio::sync::broadcast;
use tracing::{debug, warn};

/// Event broadcaster for entity change notifications
#[derive(Debug, Clone)]
pub struct EventBroadcaster {
    sender: broadcast::Sender<EntityChangeEvent>,
}

impl EventBroadcaster {
    /// Create a new event broadcaster with the specified channel capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        debug!("Created event broadcaster with capacity {}", capacity);
        Self { sender }
    }

    /// Subscribe to entity change events
    pub fn subscribe(&self) -> broadcast::Receiver<EntityChangeEvent> {
        self.sender.subscribe()
    }

    /// Emit an entity change event to all subscribers
    /// Returns the number of receivers that received the event
    pub fn emit(
        &self,
        event: EntityChangeEvent,
    ) -> Result<usize, broadcast::error::SendError<EntityChangeEvent>> {
        match self.sender.send(event.clone()) {
            Ok(count) => {
                debug!(
                    "Broadcast entity event to {} subscribers: {:?}",
                    count, event.event
                );
                Ok(count)
            }
            Err(e) => {
                warn!("Failed to broadcast entity event: {:?}", e);
                Err(e)
            }
        }
    }

    /// Get the number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::EntityEvent;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_broadcaster_creation() {
        let broadcaster = EventBroadcaster::new(100);
        assert_eq!(broadcaster.subscriber_count(), 0);
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
}
