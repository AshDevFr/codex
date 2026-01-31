//! Plugin Metrics Service
//!
//! Provides in-memory metrics collection for plugin operations.
//! This service tracks:
//! - Request counts by plugin and method
//! - Request durations
//! - Rate limit rejections
//! - Failure counts by error code
//! - Plugin health status
//!
//! Unlike task metrics, plugin metrics are ephemeral (in-memory only)
//! since they're primarily for real-time observability rather than
//! historical analysis.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Plugin health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginHealthStatus {
    /// Plugin is healthy (recent success, no failures)
    Healthy,
    /// Plugin has some failures but is still operational
    Degraded,
    /// Plugin is unhealthy (many failures or disabled)
    Unhealthy,
    /// Plugin health status is unknown (no recent operations)
    Unknown,
}

impl PluginHealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unhealthy => "unhealthy",
            Self::Unknown => "unknown",
        }
    }
}

/// Atomic counters for a single plugin's metrics
#[derive(Debug)]
struct PluginCounters {
    /// Total requests made
    requests_total: AtomicU64,
    /// Successful requests
    requests_success: AtomicU64,
    /// Failed requests
    requests_failed: AtomicU64,
    /// Total request duration in milliseconds
    total_duration_ms: AtomicU64,
    /// Rate limit rejections
    rate_limit_rejections: AtomicU64,
}

impl Default for PluginCounters {
    fn default() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            requests_success: AtomicU64::new(0),
            requests_failed: AtomicU64::new(0),
            total_duration_ms: AtomicU64::new(0),
            rate_limit_rejections: AtomicU64::new(0),
        }
    }
}

/// Per-method counters within a plugin
#[derive(Debug, Default)]
struct MethodCounters {
    /// Requests by method name
    by_method: HashMap<String, PluginCounters>,
}

/// Failure tracking for a plugin
#[derive(Debug)]
struct FailureRecord {
    /// Error code (e.g., "INIT_ERROR", "TIMEOUT")
    error_code: String,
    /// Timestamp of the failure (useful for future time-based analysis)
    #[allow(dead_code)]
    timestamp: DateTime<Utc>,
}

/// Entry for a single plugin's metrics
struct PluginMetricsEntry {
    /// Plugin ID
    plugin_id: Uuid,
    /// Plugin name (for display)
    plugin_name: String,
    /// Aggregate counters
    counters: PluginCounters,
    /// Per-method breakdown
    method_counters: MethodCounters,
    /// Recent failures for pattern analysis
    recent_failures: Vec<FailureRecord>,
    /// Last successful request timestamp
    last_success: Option<DateTime<Utc>>,
    /// Last failure timestamp
    last_failure: Option<DateTime<Utc>>,
    /// Current health status
    health_status: PluginHealthStatus,
}

impl PluginMetricsEntry {
    fn new(plugin_id: Uuid, plugin_name: String) -> Self {
        Self {
            plugin_id,
            plugin_name,
            counters: PluginCounters::default(),
            method_counters: MethodCounters::default(),
            recent_failures: Vec::new(),
            last_success: None,
            last_failure: None,
            health_status: PluginHealthStatus::Unknown,
        }
    }
}

/// Snapshot of metrics for a single plugin (returned by API)
#[derive(Debug, Clone)]
pub struct PluginMetricsSnapshot {
    pub plugin_id: Uuid,
    pub plugin_name: String,
    pub requests_total: u64,
    pub requests_success: u64,
    pub requests_failed: u64,
    pub avg_duration_ms: f64,
    pub rate_limit_rejections: u64,
    pub error_rate_pct: f64,
    pub last_success: Option<DateTime<Utc>>,
    pub last_failure: Option<DateTime<Utc>>,
    pub health_status: PluginHealthStatus,
    pub by_method: HashMap<String, MethodMetrics>,
    pub failure_counts: HashMap<String, u64>,
}

/// Metrics breakdown by method
#[derive(Debug, Clone)]
pub struct MethodMetrics {
    pub method: String,
    pub requests_total: u64,
    pub requests_success: u64,
    pub requests_failed: u64,
    pub avg_duration_ms: f64,
}

/// Summary metrics across all plugins
#[derive(Debug, Clone)]
pub struct PluginMetricsSummary {
    pub total_plugins: u64,
    pub healthy_plugins: u64,
    pub degraded_plugins: u64,
    pub unhealthy_plugins: u64,
    pub total_requests: u64,
    pub total_success: u64,
    pub total_failed: u64,
    pub total_rate_limit_rejections: u64,
}

/// Service for collecting and aggregating plugin metrics
#[derive(Clone)]
pub struct PluginMetricsService {
    /// Per-plugin metrics
    plugins: Arc<RwLock<HashMap<Uuid, PluginMetricsEntry>>>,
    /// Maximum recent failures to keep per plugin
    max_recent_failures: usize,
}

impl Default for PluginMetricsService {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginMetricsService {
    /// Create a new plugin metrics service
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            max_recent_failures: 100,
        }
    }

    /// Record a successful plugin request
    pub async fn record_success(
        &self,
        plugin_id: Uuid,
        plugin_name: &str,
        method: &str,
        duration_ms: u64,
    ) {
        let mut plugins = self.plugins.write().await;
        let entry = plugins
            .entry(plugin_id)
            .or_insert_with(|| PluginMetricsEntry::new(plugin_id, plugin_name.to_string()));

        // Update aggregate counters
        entry
            .counters
            .requests_total
            .fetch_add(1, Ordering::Relaxed);
        entry
            .counters
            .requests_success
            .fetch_add(1, Ordering::Relaxed);
        entry
            .counters
            .total_duration_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
        entry.last_success = Some(Utc::now());

        // Update per-method counters
        let method_counters = entry
            .method_counters
            .by_method
            .entry(method.to_string())
            .or_default();
        method_counters
            .requests_total
            .fetch_add(1, Ordering::Relaxed);
        method_counters
            .requests_success
            .fetch_add(1, Ordering::Relaxed);
        method_counters
            .total_duration_ms
            .fetch_add(duration_ms, Ordering::Relaxed);

        // Update health status
        entry.health_status = self.calculate_health(&entry.counters, entry.last_failure);
    }

    /// Record a failed plugin request
    pub async fn record_failure(
        &self,
        plugin_id: Uuid,
        plugin_name: &str,
        method: &str,
        duration_ms: u64,
        error_code: Option<&str>,
    ) {
        let mut plugins = self.plugins.write().await;
        let entry = plugins
            .entry(plugin_id)
            .or_insert_with(|| PluginMetricsEntry::new(plugin_id, plugin_name.to_string()));

        // Update aggregate counters
        entry
            .counters
            .requests_total
            .fetch_add(1, Ordering::Relaxed);
        entry
            .counters
            .requests_failed
            .fetch_add(1, Ordering::Relaxed);
        entry
            .counters
            .total_duration_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
        entry.last_failure = Some(Utc::now());

        // Update per-method counters
        let method_counters = entry
            .method_counters
            .by_method
            .entry(method.to_string())
            .or_default();
        method_counters
            .requests_total
            .fetch_add(1, Ordering::Relaxed);
        method_counters
            .requests_failed
            .fetch_add(1, Ordering::Relaxed);
        method_counters
            .total_duration_ms
            .fetch_add(duration_ms, Ordering::Relaxed);

        // Record failure for analysis
        let error_code = error_code.unwrap_or("UNKNOWN").to_string();
        entry.recent_failures.push(FailureRecord {
            error_code,
            timestamp: Utc::now(),
        });

        // Trim to max size
        if entry.recent_failures.len() > self.max_recent_failures {
            entry.recent_failures.remove(0);
        }

        // Update health status
        entry.health_status = self.calculate_health(&entry.counters, entry.last_failure);
    }

    /// Record a rate limit rejection
    pub async fn record_rate_limit(&self, plugin_id: Uuid, plugin_name: &str) {
        let mut plugins = self.plugins.write().await;
        let entry = plugins
            .entry(plugin_id)
            .or_insert_with(|| PluginMetricsEntry::new(plugin_id, plugin_name.to_string()));

        entry
            .counters
            .rate_limit_rejections
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Update plugin health status directly (e.g., when disabled)
    pub async fn set_health_status(&self, plugin_id: Uuid, status: PluginHealthStatus) {
        let mut plugins = self.plugins.write().await;
        if let Some(entry) = plugins.get_mut(&plugin_id) {
            entry.health_status = status;
        }
    }

    /// Get metrics snapshot for a specific plugin
    #[allow(dead_code)] // Used in tests; useful for future single-plugin endpoint
    pub async fn get_plugin_metrics(&self, plugin_id: Uuid) -> Option<PluginMetricsSnapshot> {
        let plugins = self.plugins.read().await;
        plugins
            .get(&plugin_id)
            .map(|entry| self.build_snapshot(entry))
    }

    /// Get metrics snapshots for all plugins
    pub async fn get_all_metrics(&self) -> Vec<PluginMetricsSnapshot> {
        let plugins = self.plugins.read().await;
        plugins
            .values()
            .map(|entry| self.build_snapshot(entry))
            .collect()
    }

    /// Get summary metrics across all plugins
    pub async fn get_summary(&self) -> PluginMetricsSummary {
        let plugins = self.plugins.read().await;

        let mut summary = PluginMetricsSummary {
            total_plugins: plugins.len() as u64,
            healthy_plugins: 0,
            degraded_plugins: 0,
            unhealthy_plugins: 0,
            total_requests: 0,
            total_success: 0,
            total_failed: 0,
            total_rate_limit_rejections: 0,
        };

        for entry in plugins.values() {
            match entry.health_status {
                PluginHealthStatus::Healthy => summary.healthy_plugins += 1,
                PluginHealthStatus::Degraded => summary.degraded_plugins += 1,
                PluginHealthStatus::Unhealthy => summary.unhealthy_plugins += 1,
                PluginHealthStatus::Unknown => {}
            }

            summary.total_requests += entry.counters.requests_total.load(Ordering::Relaxed);
            summary.total_success += entry.counters.requests_success.load(Ordering::Relaxed);
            summary.total_failed += entry.counters.requests_failed.load(Ordering::Relaxed);
            summary.total_rate_limit_rejections +=
                entry.counters.rate_limit_rejections.load(Ordering::Relaxed);
        }

        summary
    }

    /// Clear all metrics (useful for testing)
    #[cfg(test)]
    pub async fn clear(&self) {
        let mut plugins = self.plugins.write().await;
        plugins.clear();
    }

    /// Remove metrics for a specific plugin
    pub async fn remove_plugin(&self, plugin_id: Uuid) {
        let mut plugins = self.plugins.write().await;
        plugins.remove(&plugin_id);
    }

    /// Calculate health status based on counters
    fn calculate_health(
        &self,
        counters: &PluginCounters,
        last_failure: Option<DateTime<Utc>>,
    ) -> PluginHealthStatus {
        let total = counters.requests_total.load(Ordering::Relaxed);
        let failed = counters.requests_failed.load(Ordering::Relaxed);

        if total == 0 {
            return PluginHealthStatus::Unknown;
        }

        let error_rate = failed as f64 / total as f64;

        // Check for recent failures (within last 5 minutes)
        let recent_failure = last_failure.is_some_and(|t| (Utc::now() - t).num_minutes() < 5);

        if error_rate > 0.5 || (recent_failure && error_rate > 0.2) {
            PluginHealthStatus::Unhealthy
        } else if error_rate > 0.1 || recent_failure {
            PluginHealthStatus::Degraded
        } else {
            PluginHealthStatus::Healthy
        }
    }

    /// Build a snapshot from an entry
    fn build_snapshot(&self, entry: &PluginMetricsEntry) -> PluginMetricsSnapshot {
        let requests_total = entry.counters.requests_total.load(Ordering::Relaxed);
        let requests_success = entry.counters.requests_success.load(Ordering::Relaxed);
        let requests_failed = entry.counters.requests_failed.load(Ordering::Relaxed);
        let total_duration_ms = entry.counters.total_duration_ms.load(Ordering::Relaxed);

        let avg_duration_ms = if requests_total > 0 {
            total_duration_ms as f64 / requests_total as f64
        } else {
            0.0
        };

        let error_rate_pct = if requests_total > 0 {
            (requests_failed as f64 / requests_total as f64) * 100.0
        } else {
            0.0
        };

        // Build per-method metrics
        let by_method: HashMap<String, MethodMetrics> = entry
            .method_counters
            .by_method
            .iter()
            .map(|(method, counters)| {
                let total = counters.requests_total.load(Ordering::Relaxed);
                let duration = counters.total_duration_ms.load(Ordering::Relaxed);
                (
                    method.clone(),
                    MethodMetrics {
                        method: method.clone(),
                        requests_total: total,
                        requests_success: counters.requests_success.load(Ordering::Relaxed),
                        requests_failed: counters.requests_failed.load(Ordering::Relaxed),
                        avg_duration_ms: if total > 0 {
                            duration as f64 / total as f64
                        } else {
                            0.0
                        },
                    },
                )
            })
            .collect();

        // Count failures by error code
        let mut failure_counts: HashMap<String, u64> = HashMap::new();
        for failure in &entry.recent_failures {
            *failure_counts
                .entry(failure.error_code.clone())
                .or_insert(0) += 1;
        }

        PluginMetricsSnapshot {
            plugin_id: entry.plugin_id,
            plugin_name: entry.plugin_name.clone(),
            requests_total,
            requests_success,
            requests_failed,
            avg_duration_ms,
            rate_limit_rejections: entry.counters.rate_limit_rejections.load(Ordering::Relaxed),
            error_rate_pct,
            last_success: entry.last_success,
            last_failure: entry.last_failure,
            health_status: entry.health_status,
            by_method,
            failure_counts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_success() {
        let service = PluginMetricsService::new();
        let plugin_id = Uuid::new_v4();

        service
            .record_success(plugin_id, "test-plugin", "search", 100)
            .await;
        service
            .record_success(plugin_id, "test-plugin", "search", 200)
            .await;

        let metrics = service.get_plugin_metrics(plugin_id).await.unwrap();
        assert_eq!(metrics.requests_total, 2);
        assert_eq!(metrics.requests_success, 2);
        assert_eq!(metrics.requests_failed, 0);
        assert_eq!(metrics.avg_duration_ms, 150.0);
        assert_eq!(metrics.health_status, PluginHealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_record_failure() {
        let service = PluginMetricsService::new();
        let plugin_id = Uuid::new_v4();

        service
            .record_failure(plugin_id, "test-plugin", "search", 100, Some("TIMEOUT"))
            .await;

        let metrics = service.get_plugin_metrics(plugin_id).await.unwrap();
        assert_eq!(metrics.requests_total, 1);
        assert_eq!(metrics.requests_success, 0);
        assert_eq!(metrics.requests_failed, 1);
        assert_eq!(metrics.error_rate_pct, 100.0);
        assert_eq!(metrics.failure_counts.get("TIMEOUT"), Some(&1));
    }

    #[tokio::test]
    async fn test_health_status_calculation() {
        let service = PluginMetricsService::new();
        let plugin_id = Uuid::new_v4();

        // Start with success
        for _ in 0..10 {
            service
                .record_success(plugin_id, "test-plugin", "search", 100)
                .await;
        }
        let metrics = service.get_plugin_metrics(plugin_id).await.unwrap();
        assert_eq!(metrics.health_status, PluginHealthStatus::Healthy);

        // Add some failures (>10% error rate = degraded)
        for _ in 0..2 {
            service
                .record_failure(plugin_id, "test-plugin", "search", 100, None)
                .await;
        }
        let metrics = service.get_plugin_metrics(plugin_id).await.unwrap();
        assert_eq!(metrics.health_status, PluginHealthStatus::Degraded);

        // Add many more failures (>50% error rate = unhealthy)
        for _ in 0..10 {
            service
                .record_failure(plugin_id, "test-plugin", "search", 100, None)
                .await;
        }
        let metrics = service.get_plugin_metrics(plugin_id).await.unwrap();
        assert_eq!(metrics.health_status, PluginHealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_rate_limit_recording() {
        let service = PluginMetricsService::new();
        let plugin_id = Uuid::new_v4();

        service.record_rate_limit(plugin_id, "test-plugin").await;
        service.record_rate_limit(plugin_id, "test-plugin").await;

        let metrics = service.get_plugin_metrics(plugin_id).await.unwrap();
        assert_eq!(metrics.rate_limit_rejections, 2);
    }

    #[tokio::test]
    async fn test_per_method_metrics() {
        let service = PluginMetricsService::new();
        let plugin_id = Uuid::new_v4();

        service
            .record_success(plugin_id, "test-plugin", "search", 100)
            .await;
        service
            .record_success(plugin_id, "test-plugin", "search", 200)
            .await;
        service
            .record_success(plugin_id, "test-plugin", "get_metadata", 50)
            .await;
        service
            .record_failure(plugin_id, "test-plugin", "get_metadata", 300, None)
            .await;

        let metrics = service.get_plugin_metrics(plugin_id).await.unwrap();

        let search_metrics = metrics.by_method.get("search").unwrap();
        assert_eq!(search_metrics.requests_total, 2);
        assert_eq!(search_metrics.requests_success, 2);
        assert_eq!(search_metrics.avg_duration_ms, 150.0);

        let get_metadata_metrics = metrics.by_method.get("get_metadata").unwrap();
        assert_eq!(get_metadata_metrics.requests_total, 2);
        assert_eq!(get_metadata_metrics.requests_success, 1);
        assert_eq!(get_metadata_metrics.requests_failed, 1);
    }

    #[tokio::test]
    async fn test_summary() {
        let service = PluginMetricsService::new();

        let plugin1 = Uuid::new_v4();
        let plugin2 = Uuid::new_v4();

        // Plugin 1: healthy
        for _ in 0..10 {
            service
                .record_success(plugin1, "plugin-1", "search", 100)
                .await;
        }

        // Plugin 2: unhealthy (all failures)
        for _ in 0..5 {
            service
                .record_failure(plugin2, "plugin-2", "search", 100, Some("ERROR"))
                .await;
        }

        let summary = service.get_summary().await;
        assert_eq!(summary.total_plugins, 2);
        assert_eq!(summary.healthy_plugins, 1);
        assert_eq!(summary.unhealthy_plugins, 1);
        assert_eq!(summary.total_requests, 15);
        assert_eq!(summary.total_success, 10);
        assert_eq!(summary.total_failed, 5);
    }

    #[tokio::test]
    async fn test_clear() {
        let service = PluginMetricsService::new();
        let plugin_id = Uuid::new_v4();

        service
            .record_success(plugin_id, "test-plugin", "search", 100)
            .await;
        assert!(service.get_plugin_metrics(plugin_id).await.is_some());

        service.clear().await;
        assert!(service.get_plugin_metrics(plugin_id).await.is_none());
    }

    #[tokio::test]
    async fn test_remove_plugin() {
        let service = PluginMetricsService::new();
        let plugin1 = Uuid::new_v4();
        let plugin2 = Uuid::new_v4();

        service
            .record_success(plugin1, "plugin-1", "search", 100)
            .await;
        service
            .record_success(plugin2, "plugin-2", "search", 100)
            .await;

        service.remove_plugin(plugin1).await;

        assert!(service.get_plugin_metrics(plugin1).await.is_none());
        assert!(service.get_plugin_metrics(plugin2).await.is_some());
    }
}
