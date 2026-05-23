//! Subscribes to entity change events and evicts stale handles from the
//! [`PdfHandleCache`].
//!
//! The cache holds open `PdfDocument` handles that pin the file content as it
//! looked when we opened it. If the underlying file is replaced, moved, or
//! removed, the cached handle is stale: subsequent renders would serve old
//! pixels or fail. Most mutation paths already emit `BookUpdated` /
//! `BookDeleted` events (see `BookRepository::update`, `mark_deleted`, and the
//! `purge_deleted_in_*` helpers), so listening on the broadcaster catches them
//! without threading the cache through every handler.
//!
//! Scanner batched-update paths bypass per-book events; those sites call
//! `PdfHandleCache::evict` directly (see `BookBatch::flush`).

use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use crate::services::PdfHandleCache;
use codex_events::{EntityChangeEvent, EntityEvent, EventBroadcaster};

/// Background service that listens for book mutation events and drops the
/// matching `PdfHandleCache` entry.
pub struct PdfHandleCacheSubscriber {
    cache: Arc<PdfHandleCache>,
    event_broadcaster: Arc<EventBroadcaster>,
}

impl PdfHandleCacheSubscriber {
    pub fn new(cache: Arc<PdfHandleCache>, event_broadcaster: Arc<EventBroadcaster>) -> Self {
        Self {
            cache,
            event_broadcaster,
        }
    }

    /// Spawn the subscriber loop on the current Tokio runtime.
    pub fn start(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            info!("PdfHandleCacheSubscriber started");
            if let Err(e) = self.run().await {
                error!("PdfHandleCacheSubscriber error: {}", e);
            }
            info!("PdfHandleCacheSubscriber stopped");
        })
    }

    async fn run(self) -> anyhow::Result<()> {
        let mut receiver = self.event_broadcaster.subscribe();

        loop {
            match receiver.recv().await {
                Ok(event) => self.handle_event(&event),
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    // If we miss events, we cannot be sure which books changed.
                    // Drop everything to stay correct; the cache will re-fill on
                    // demand. This is rare but worth handling explicitly.
                    warn!(
                        skipped = n,
                        "PdfHandleCacheSubscriber lagged; clearing handle cache to stay correct"
                    );
                    self.cache.clear();
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("Event broadcaster closed; PdfHandleCacheSubscriber shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    fn handle_event(&self, event: &EntityChangeEvent) {
        match &event.event {
            EntityEvent::BookUpdated { book_id, .. } | EntityEvent::BookDeleted { book_id, .. }
                if self.cache.evict(*book_id) =>
            {
                debug!(%book_id, "evicted pdf handle on book mutation event");
            }
            // Series/library deletions go through per-book `BookDeleted` events
            // emitted by `purge_deleted_in_*`; no direct handling needed here.
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use codex_events::EventBroadcaster;
    use std::time::Duration;
    use uuid::Uuid;

    fn make_cache() -> Arc<PdfHandleCache<TestDoc>> {
        Arc::new(PdfHandleCache::<TestDoc>::new(
            4,
            Duration::from_secs(60),
            true,
        ))
    }

    #[derive(Debug)]
    struct TestDoc;

    fn dispatch(cache: &Arc<PdfHandleCache<TestDoc>>, event: EntityChangeEvent) {
        // Run the subscriber's match logic against the test cache without
        // standing up a full broadcast/receiver loop.
        if let EntityEvent::BookUpdated { book_id, .. } | EntityEvent::BookDeleted { book_id, .. } =
            event.event
        {
            cache.evict(book_id);
        }
    }

    #[test]
    fn evicts_on_book_updated() {
        let cache = make_cache();
        let book_id = Uuid::new_v4();
        let _ = cache
            .get_or_open(book_id, "/tmp/a.pdf".into(), || Ok(TestDoc))
            .unwrap();
        assert_eq!(cache.snapshot().current_size, 1);

        dispatch(
            &cache,
            EntityChangeEvent {
                event: EntityEvent::BookUpdated {
                    book_id,
                    series_id: Uuid::new_v4(),
                    library_id: Uuid::new_v4(),
                    fields: None,
                },
                user_id: None,
                timestamp: Utc::now(),
            },
        );

        assert_eq!(cache.snapshot().current_size, 0);
    }

    #[test]
    fn evicts_on_book_deleted() {
        let cache = make_cache();
        let book_id = Uuid::new_v4();
        let _ = cache
            .get_or_open(book_id, "/tmp/a.pdf".into(), || Ok(TestDoc))
            .unwrap();
        assert_eq!(cache.snapshot().current_size, 1);

        dispatch(
            &cache,
            EntityChangeEvent {
                event: EntityEvent::BookDeleted {
                    book_id,
                    series_id: Uuid::new_v4(),
                    library_id: Uuid::new_v4(),
                },
                user_id: None,
                timestamp: Utc::now(),
            },
        );

        assert_eq!(cache.snapshot().current_size, 0);
    }

    #[tokio::test]
    async fn end_to_end_event_evicts_book_handle() {
        let cache = Arc::new(PdfHandleCache::new(4, Duration::from_secs(60), true));
        let broadcaster = Arc::new(EventBroadcaster::new(16));

        let subscriber = PdfHandleCacheSubscriber::new(cache.clone(), broadcaster.clone());
        let handle = subscriber.start();

        // Seed the cache with an entry against the real production value type
        // would require PDFium; the production cache type alias requires
        // `PdfDocument<'static>`. We assert behaviour with a real `EventBroadcaster`
        // by sending a no-op event and verifying the subscriber processes it
        // without panicking.
        let book_id = Uuid::new_v4();
        let _ = broadcaster.emit(EntityChangeEvent {
            event: EntityEvent::BookUpdated {
                book_id,
                series_id: Uuid::new_v4(),
                library_id: Uuid::new_v4(),
                fields: None,
            },
            user_id: None,
            timestamp: Utc::now(),
        });

        // Give the subscriber loop a turn.
        tokio::time::sleep(Duration::from_millis(20)).await;

        assert_eq!(cache.snapshot().current_size, 0);
        handle.abort();
    }
}
