//! In-flight thumbnail request deduplication service
//!
//! This service prevents the "thundering herd" problem where multiple concurrent
//! requests for the same uncached thumbnail all try to generate it simultaneously.
//!
//! When multiple requests come in for the same thumbnail:
//! 1. The first request starts generating the thumbnail
//! 2. Subsequent requests wait for the first one to complete
//! 3. All requests receive the same result without duplicate work

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Result of thumbnail generation that can be shared between waiters
#[derive(Clone, Debug)]
pub enum ThumbnailResult {
    /// Thumbnail was successfully generated
    Success(Vec<u8>),
    /// Thumbnail generation failed with an error message
    Failed(String),
}

/// Handle for an in-flight thumbnail generation request
#[derive(Debug)]
pub struct InflightHandle {
    /// Receiver to wait for the result
    receiver: broadcast::Receiver<ThumbnailResult>,
}

impl InflightHandle {
    /// Wait for the thumbnail generation to complete
    pub async fn wait(mut self) -> Result<Vec<u8>, String> {
        match self.receiver.recv().await {
            Ok(ThumbnailResult::Success(data)) => Ok(data),
            Ok(ThumbnailResult::Failed(err)) => Err(err),
            Err(e) => Err(format!("Channel error: {}", e)),
        }
    }
}

/// Guard that removes the entry and notifies waiters when dropped
#[derive(Debug)]
pub struct GenerationGuard {
    book_id: Uuid,
    sender: broadcast::Sender<ThumbnailResult>,
    tracker: Arc<InflightThumbnailTracker>,
    completed: bool,
}

impl GenerationGuard {
    /// Mark the generation as complete with success
    pub fn complete(mut self, data: Vec<u8>) {
        self.completed = true;
        // Send to all waiters (ignore errors if no receivers)
        let _ = self.sender.send(ThumbnailResult::Success(data));
    }

    /// Mark the generation as failed
    pub fn fail(mut self, error: String) {
        self.completed = true;
        // Send to all waiters (ignore errors if no receivers)
        let _ = self.sender.send(ThumbnailResult::Failed(error));
    }
}

impl Drop for GenerationGuard {
    fn drop(&mut self) {
        // Always remove the entry when the guard is dropped
        self.tracker.inflight.remove(&self.book_id);

        // If not explicitly completed, notify waiters of failure
        if !self.completed {
            let _ = self
                .sender
                .send(ThumbnailResult::Failed("Generation cancelled".to_string()));
        }
    }
}

/// Service for tracking in-flight thumbnail generation requests
#[derive(Debug)]
pub struct InflightThumbnailTracker {
    /// Map of book_id -> broadcast sender for result notification
    inflight: DashMap<Uuid, broadcast::Sender<ThumbnailResult>>,
}

impl Default for InflightThumbnailTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl InflightThumbnailTracker {
    /// Create a new tracker
    pub fn new() -> Self {
        Self {
            inflight: DashMap::new(),
        }
    }

    /// Try to start generating a thumbnail for a book
    ///
    /// Returns:
    /// - `Ok(GenerationGuard)` if this is the first request and we should generate
    /// - `Err(InflightHandle)` if another request is already generating, wait on the handle
    pub fn try_start(self: &Arc<Self>, book_id: Uuid) -> Result<GenerationGuard, InflightHandle> {
        // Try to insert a new entry
        // Use a channel with capacity 1 since we only send one result
        let (sender, _) = broadcast::channel(1);

        // Try to insert, if already exists, subscribe to the existing sender
        match self.inflight.entry(book_id) {
            dashmap::mapref::entry::Entry::Occupied(entry) => {
                // Another request is already generating this thumbnail
                let receiver = entry.get().subscribe();
                Err(InflightHandle { receiver })
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                // We're the first, insert and return guard
                entry.insert(sender.clone());
                Ok(GenerationGuard {
                    book_id,
                    sender,
                    tracker: Arc::clone(self),
                    completed: false,
                })
            }
        }
    }

    /// Get the number of currently in-flight requests (for metrics/debugging)
    #[allow(dead_code)]
    pub fn inflight_count(&self) -> usize {
        self.inflight.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_single_request() {
        let tracker = Arc::new(InflightThumbnailTracker::new());
        let book_id = Uuid::new_v4();

        // First request should get a guard
        let guard = tracker.try_start(book_id).expect("Should get guard");

        // Complete the request
        guard.complete(vec![1, 2, 3]);

        // Entry should be removed
        assert_eq!(tracker.inflight_count(), 0);
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let tracker = Arc::new(InflightThumbnailTracker::new());
        let book_id = Uuid::new_v4();

        // First request gets the guard
        let guard = tracker.try_start(book_id).expect("Should get guard");

        // Second request should get a handle to wait
        let handle = tracker.try_start(book_id).expect_err("Should get handle");

        // Third request should also get a handle
        let handle2 = tracker.try_start(book_id).expect_err("Should get handle");

        // Spawn waiters
        let wait1 = tokio::spawn(async move { handle.wait().await });
        let wait2 = tokio::spawn(async move { handle2.wait().await });

        // Give waiters time to start
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Complete the generation
        guard.complete(vec![4, 5, 6]);

        // Both waiters should get the result
        let result1 = wait1.await.unwrap();
        let result2 = wait2.await.unwrap();

        assert_eq!(result1.unwrap(), vec![4, 5, 6]);
        assert_eq!(result2.unwrap(), vec![4, 5, 6]);
    }

    #[tokio::test]
    async fn test_failure_propagation() {
        let tracker = Arc::new(InflightThumbnailTracker::new());
        let book_id = Uuid::new_v4();

        let guard = tracker.try_start(book_id).expect("Should get guard");
        let handle = tracker.try_start(book_id).expect_err("Should get handle");

        let wait = tokio::spawn(async move { handle.wait().await });

        // Fail the generation
        guard.fail("Test error".to_string());

        let result = wait.await.unwrap();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Test error"));
    }

    #[tokio::test]
    async fn test_guard_drop_cleanup() {
        let tracker = Arc::new(InflightThumbnailTracker::new());
        let book_id = Uuid::new_v4();

        {
            let _guard = tracker.try_start(book_id).expect("Should get guard");
            assert_eq!(tracker.inflight_count(), 1);
            // Guard dropped here without completing
        }

        // Entry should be removed
        assert_eq!(tracker.inflight_count(), 0);
    }

    #[tokio::test]
    async fn test_different_books_independent() {
        let tracker = Arc::new(InflightThumbnailTracker::new());
        let book_id1 = Uuid::new_v4();
        let book_id2 = Uuid::new_v4();

        // Both should get guards (different books)
        let guard1 = tracker.try_start(book_id1).expect("Should get guard 1");
        let guard2 = tracker.try_start(book_id2).expect("Should get guard 2");

        assert_eq!(tracker.inflight_count(), 2);

        guard1.complete(vec![1]);
        guard2.complete(vec![2]);

        assert_eq!(tracker.inflight_count(), 0);
    }
}
