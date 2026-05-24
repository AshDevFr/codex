//! Health Tracking for Plugins
//!
//! Tracks consecutive failures for plugins and determines when a plugin
//! should be auto-disabled. Used by `PluginHandle` for in-process health
//! decisions during active operations.

use std::sync::atomic::{AtomicU32, Ordering};

/// Tracks consecutive failure count for a plugin to support auto-disable logic.
pub struct HealthTracker {
    /// Maximum consecutive failures before disabling
    max_failures: u32,
    /// Number of consecutive failures
    consecutive_failures: AtomicU32,
}

impl HealthTracker {
    /// Create a new health tracker with the given failure threshold.
    pub fn new(max_failures: u32) -> Self {
        Self {
            max_failures,
            consecutive_failures: AtomicU32::new(0),
        }
    }

    /// Record a successful operation (resets consecutive failure count).
    pub async fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::SeqCst);
    }

    /// Record a failed operation (increments consecutive failure count).
    pub async fn record_failure(&self) {
        self.consecutive_failures.fetch_add(1, Ordering::SeqCst);
    }

    /// Check if the plugin should be disabled due to reaching the failure threshold.
    pub async fn should_disable(&self) -> bool {
        let failures = self.consecutive_failures.load(Ordering::SeqCst);
        failures >= self.max_failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_tracker_record_success() {
        let tracker = HealthTracker::new(3);

        // Initially no failures
        assert!(!tracker.should_disable().await);

        // Record failures then success resets count
        tracker.record_failure().await;
        tracker.record_failure().await;
        tracker.record_success().await;
        assert!(!tracker.should_disable().await);
    }

    #[tokio::test]
    async fn test_health_tracker_record_failure() {
        let tracker = HealthTracker::new(3);

        tracker.record_failure().await;
        assert!(!tracker.should_disable().await);

        tracker.record_failure().await;
        assert!(!tracker.should_disable().await);
    }

    #[tokio::test]
    async fn test_health_tracker_should_disable_at_threshold() {
        let tracker = HealthTracker::new(3);

        tracker.record_failure().await;
        tracker.record_failure().await;
        assert!(!tracker.should_disable().await);

        // Third failure hits the threshold
        tracker.record_failure().await;
        assert!(tracker.should_disable().await);
    }

    #[tokio::test]
    async fn test_health_tracker_success_resets_failures() {
        let tracker = HealthTracker::new(3);

        // Get close to threshold
        tracker.record_failure().await;
        tracker.record_failure().await;

        // Success resets
        tracker.record_success().await;
        assert!(!tracker.should_disable().await);

        // Need 3 fresh failures to hit threshold again
        tracker.record_failure().await;
        assert!(!tracker.should_disable().await);
    }
}
