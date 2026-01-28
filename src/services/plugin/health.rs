//! Health Monitoring for Plugins
//!
//! This module provides health tracking and monitoring for plugins,
//! including failure counting and auto-disable logic.
//!
//! Note: This module provides complete health monitoring infrastructure.
//! Some types and methods may not be called from external code yet but are
//! part of the complete API for plugin health management.

// Allow dead code for health monitoring infrastructure that is part of the
// complete API surface but not yet fully integrated.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::RwLock;

/// Health status of a plugin
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// Plugin is healthy
    Healthy,
    /// Plugin is degraded (some failures but still operational)
    Degraded,
    /// Plugin is unhealthy (at or near failure threshold)
    Unhealthy,
    /// Plugin health is unknown (not yet checked)
    Unknown,
    /// Plugin is disabled due to failures
    Disabled,
}

impl HealthStatus {
    pub fn as_str(&self) -> &str {
        match self {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Unhealthy => "unhealthy",
            HealthStatus::Unknown => "unknown",
            HealthStatus::Disabled => "disabled",
        }
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Current health state of a plugin
#[derive(Debug, Clone)]
pub struct HealthState {
    /// Current health status
    pub status: HealthStatus,
    /// Number of consecutive failures
    pub consecutive_failures: u32,
    /// Total failure count (lifetime)
    pub total_failures: u32,
    /// Total success count (lifetime)
    pub total_successes: u32,
    /// Last successful operation
    pub last_success_at: Option<DateTime<Utc>>,
    /// Last failed operation
    pub last_failure_at: Option<DateTime<Utc>>,
    /// Reason for current status (if applicable)
    pub reason: Option<String>,
}

impl Default for HealthState {
    fn default() -> Self {
        Self {
            status: HealthStatus::Unknown,
            consecutive_failures: 0,
            total_failures: 0,
            total_successes: 0,
            last_success_at: None,
            last_failure_at: None,
            reason: None,
        }
    }
}

/// Tracks health state for a plugin
pub struct HealthTracker {
    /// Maximum consecutive failures before disabling
    max_failures: u32,
    /// Number of consecutive failures
    consecutive_failures: AtomicU32,
    /// Total failure count
    total_failures: AtomicU32,
    /// Total success count
    total_successes: AtomicU32,
    /// Last success time
    last_success_at: RwLock<Option<DateTime<Utc>>>,
    /// Last failure time
    last_failure_at: RwLock<Option<DateTime<Utc>>>,
    /// Whether the plugin has been disabled
    disabled: RwLock<bool>,
    /// Reason for being disabled
    disabled_reason: RwLock<Option<String>>,
}

impl HealthTracker {
    /// Create a new health tracker
    pub fn new(max_failures: u32) -> Self {
        Self {
            max_failures,
            consecutive_failures: AtomicU32::new(0),
            total_failures: AtomicU32::new(0),
            total_successes: AtomicU32::new(0),
            last_success_at: RwLock::new(None),
            last_failure_at: RwLock::new(None),
            disabled: RwLock::new(false),
            disabled_reason: RwLock::new(None),
        }
    }

    /// Record a successful operation
    pub async fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::SeqCst);
        self.total_successes.fetch_add(1, Ordering::SeqCst);
        *self.last_success_at.write().await = Some(Utc::now());
    }

    /// Record a failed operation
    pub async fn record_failure(&self) {
        self.consecutive_failures.fetch_add(1, Ordering::SeqCst);
        self.total_failures.fetch_add(1, Ordering::SeqCst);
        *self.last_failure_at.write().await = Some(Utc::now());
    }

    /// Check if the plugin should be disabled due to failures
    pub async fn should_disable(&self) -> bool {
        let failures = self.consecutive_failures.load(Ordering::SeqCst);
        failures >= self.max_failures
    }

    /// Mark the plugin as disabled
    pub async fn mark_disabled(&self, reason: impl Into<String>) {
        *self.disabled.write().await = true;
        *self.disabled_reason.write().await = Some(reason.into());
    }

    /// Check if the plugin is disabled
    pub async fn is_disabled(&self) -> bool {
        *self.disabled.read().await
    }

    /// Reset the health tracker (for re-enabling)
    pub async fn reset(&self) {
        self.consecutive_failures.store(0, Ordering::SeqCst);
        *self.disabled.write().await = false;
        *self.disabled_reason.write().await = None;
    }

    /// Get the current health state
    pub async fn state(&self) -> HealthState {
        let consecutive_failures = self.consecutive_failures.load(Ordering::SeqCst);
        let total_failures = self.total_failures.load(Ordering::SeqCst);
        let total_successes = self.total_successes.load(Ordering::SeqCst);
        let last_success_at = *self.last_success_at.read().await;
        let last_failure_at = *self.last_failure_at.read().await;
        let is_disabled = *self.disabled.read().await;
        let disabled_reason = self.disabled_reason.read().await.clone();

        let status = if is_disabled {
            HealthStatus::Disabled
        } else if total_successes == 0 && total_failures == 0 {
            HealthStatus::Unknown
        } else if consecutive_failures >= self.max_failures {
            HealthStatus::Unhealthy
        } else if consecutive_failures > 0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        HealthState {
            status,
            consecutive_failures,
            total_failures,
            total_successes,
            last_success_at,
            last_failure_at,
            reason: disabled_reason,
        }
    }

    /// Get the current health status
    pub async fn status(&self) -> HealthStatus {
        self.state().await.status
    }
}

/// Monitor for managing health checks across multiple plugins
pub struct HealthMonitor {
    /// Check interval
    check_interval: std::time::Duration,
    /// Whether monitoring is active
    active: RwLock<bool>,
}

impl HealthMonitor {
    /// Create a new health monitor
    pub fn new(check_interval: std::time::Duration) -> Self {
        Self {
            check_interval,
            active: RwLock::new(false),
        }
    }

    /// Get the check interval
    pub fn check_interval(&self) -> std::time::Duration {
        self.check_interval
    }

    /// Check if monitoring is active
    pub async fn is_active(&self) -> bool {
        *self.active.read().await
    }

    /// Start monitoring (placeholder - actual implementation in Phase 2)
    pub async fn start(&self) {
        *self.active.write().await = true;
    }

    /// Stop monitoring
    pub async fn stop(&self) {
        *self.active.write().await = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_as_str() {
        assert_eq!(HealthStatus::Healthy.as_str(), "healthy");
        assert_eq!(HealthStatus::Degraded.as_str(), "degraded");
        assert_eq!(HealthStatus::Unhealthy.as_str(), "unhealthy");
        assert_eq!(HealthStatus::Unknown.as_str(), "unknown");
        assert_eq!(HealthStatus::Disabled.as_str(), "disabled");
    }

    #[test]
    fn test_health_status_display() {
        assert_eq!(format!("{}", HealthStatus::Healthy), "healthy");
        assert_eq!(format!("{}", HealthStatus::Disabled), "disabled");
    }

    #[test]
    fn test_health_state_default() {
        let state = HealthState::default();
        assert_eq!(state.status, HealthStatus::Unknown);
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.total_failures, 0);
        assert_eq!(state.total_successes, 0);
        assert!(state.last_success_at.is_none());
        assert!(state.last_failure_at.is_none());
        assert!(state.reason.is_none());
    }

    #[tokio::test]
    async fn test_health_tracker_initial_state() {
        let tracker = HealthTracker::new(3);
        let state = tracker.state().await;

        assert_eq!(state.status, HealthStatus::Unknown);
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.total_failures, 0);
        assert_eq!(state.total_successes, 0);
    }

    #[tokio::test]
    async fn test_health_tracker_record_success() {
        let tracker = HealthTracker::new(3);

        tracker.record_success().await;
        let state = tracker.state().await;

        assert_eq!(state.status, HealthStatus::Healthy);
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.total_successes, 1);
        assert!(state.last_success_at.is_some());
    }

    #[tokio::test]
    async fn test_health_tracker_record_failure() {
        let tracker = HealthTracker::new(3);

        tracker.record_failure().await;
        let state = tracker.state().await;

        assert_eq!(state.status, HealthStatus::Degraded);
        assert_eq!(state.consecutive_failures, 1);
        assert_eq!(state.total_failures, 1);
        assert!(state.last_failure_at.is_some());
    }

    #[tokio::test]
    async fn test_health_tracker_success_resets_failures() {
        let tracker = HealthTracker::new(3);

        // Record some failures
        tracker.record_failure().await;
        tracker.record_failure().await;
        assert_eq!(tracker.state().await.consecutive_failures, 2);

        // Success should reset consecutive failures
        tracker.record_success().await;
        assert_eq!(tracker.state().await.consecutive_failures, 0);
        assert_eq!(tracker.state().await.total_failures, 2); // Total unchanged
    }

    #[tokio::test]
    async fn test_health_tracker_should_disable() {
        let tracker = HealthTracker::new(3);

        // Not enough failures yet
        tracker.record_failure().await;
        tracker.record_failure().await;
        assert!(!tracker.should_disable().await);

        // Third failure should trigger disable
        tracker.record_failure().await;
        assert!(tracker.should_disable().await);
        assert_eq!(tracker.state().await.status, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_health_tracker_mark_disabled() {
        let tracker = HealthTracker::new(3);

        tracker.mark_disabled("Too many failures").await;
        let state = tracker.state().await;

        assert_eq!(state.status, HealthStatus::Disabled);
        assert!(tracker.is_disabled().await);
        assert_eq!(state.reason, Some("Too many failures".to_string()));
    }

    #[tokio::test]
    async fn test_health_tracker_reset() {
        let tracker = HealthTracker::new(3);

        // Add some failures and disable
        tracker.record_failure().await;
        tracker.record_failure().await;
        tracker.record_failure().await;
        tracker.mark_disabled("Test").await;

        assert!(tracker.is_disabled().await);

        // Reset should clear disabled state
        tracker.reset().await;

        assert!(!tracker.is_disabled().await);
        assert_eq!(tracker.state().await.consecutive_failures, 0);
        // Note: total_failures is NOT reset
        assert_eq!(tracker.state().await.total_failures, 3);
    }

    #[tokio::test]
    async fn test_health_monitor_lifecycle() {
        let monitor = HealthMonitor::new(std::time::Duration::from_secs(30));

        assert!(!monitor.is_active().await);
        assert_eq!(monitor.check_interval(), std::time::Duration::from_secs(30));

        monitor.start().await;
        assert!(monitor.is_active().await);

        monitor.stop().await;
        assert!(!monitor.is_active().await);
    }
}
