use anyhow::Result;
use chrono::{DateTime, Duration, Timelike, Utc};
use sea_orm::DatabaseConnection;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration as TokioDuration, interval};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};
use uuid::Uuid;

use crate::db::repositories::task_metrics::{TaskCompletionData, TaskMetricsRepository};
use crate::services::SettingsService;

/// Number of recent completions to keep for percentile calculation
const MAX_RECENT_COMPLETIONS: usize = 1000;
/// Number of duration samples to keep per task type
const MAX_SAMPLES_PER_TYPE: usize = 100;

/// A single task completion record for in-memory tracking
#[derive(Debug, Clone)]
struct TaskCompletion {
    task_type: String,
    library_id: Option<Uuid>,
    success: bool,
    retried: bool,
    duration_ms: i64,
    queue_wait_ms: i64,
    items_processed: i64,
    bytes_processed: i64,
    error: Option<String>,
    completed_at: DateTime<Utc>,
}

/// Running aggregate for a specific task type
#[derive(Debug, Clone, Default)]
struct RunningAggregate {
    count: u64,
    succeeded: u64,
    failed: u64,
    retried: u64,
    total_duration_ms: i64,
    min_duration_ms: Option<i64>,
    max_duration_ms: Option<i64>,
    total_queue_wait_ms: i64,
    items_processed: i64,
    bytes_processed: i64,
    error_count: u64,
    last_error: Option<String>,
    last_error_at: Option<DateTime<Utc>>,
    duration_samples: VecDeque<i64>,
}

impl RunningAggregate {
    fn update(&mut self, completion: &TaskCompletion) {
        self.count += 1;
        if completion.success {
            self.succeeded += 1;
        } else {
            self.failed += 1;
        }
        if completion.retried {
            self.retried += 1;
        }

        self.total_duration_ms += completion.duration_ms;
        self.min_duration_ms = Some(
            self.min_duration_ms
                .map_or(completion.duration_ms, |m| m.min(completion.duration_ms)),
        );
        self.max_duration_ms = Some(
            self.max_duration_ms
                .map_or(completion.duration_ms, |m| m.max(completion.duration_ms)),
        );
        self.total_queue_wait_ms += completion.queue_wait_ms;
        self.items_processed += completion.items_processed;
        self.bytes_processed += completion.bytes_processed;

        if completion.error.is_some() {
            self.error_count += 1;
            self.last_error = completion.error.clone();
            self.last_error_at = Some(completion.completed_at);
        }

        // Keep recent samples for percentile calculation
        self.duration_samples.push_back(completion.duration_ms);
        if self.duration_samples.len() > MAX_SAMPLES_PER_TYPE {
            self.duration_samples.pop_front();
        }
    }

    /// Calculate percentile from samples
    fn percentile(&self, p: f64) -> Option<i64> {
        if self.duration_samples.is_empty() {
            return None;
        }
        let mut sorted: Vec<i64> = self.duration_samples.iter().copied().collect();
        sorted.sort();
        let index = ((sorted.len() as f64 - 1.0) * p / 100.0).round() as usize;
        Some(sorted[index.min(sorted.len() - 1)])
    }
}

/// Helper struct for aggregating DB records by task type
#[derive(Debug, Default)]
struct DbTaskAggregate {
    count: u64,
    succeeded: u64,
    failed: u64,
    retried: u64,
    total_duration_ms: i64,
    min_duration_ms: Option<i64>,
    max_duration_ms: Option<i64>,
    total_queue_wait_ms: i64,
    items_processed: u64,
    bytes_processed: u64,
    duration_samples: Vec<i64>,
    last_error: Option<String>,
    last_error_at: Option<DateTime<Utc>>,
}

/// Calculate p50 and p95 percentiles from a slice of duration samples
fn calculate_percentiles_from_samples(samples: &[i64]) -> (u64, u64) {
    if samples.is_empty() {
        return (0, 0);
    }

    let mut sorted: Vec<i64> = samples.to_vec();
    sorted.sort();

    let p50_index = ((sorted.len() as f64 - 1.0) * 0.50).round() as usize;
    let p95_index = ((sorted.len() as f64 - 1.0) * 0.95).round() as usize;

    let p50 = sorted[p50_index.min(sorted.len() - 1)] as u64;
    let p95 = sorted[p95_index.min(sorted.len() - 1)] as u64;

    (p50, p95)
}

/// In-memory metrics collector
struct MetricsCollector {
    /// Rolling window of recent completions
    recent: VecDeque<TaskCompletion>,
    /// Running aggregates by task type
    aggregates: HashMap<String, RunningAggregate>,
    /// Last flush timestamp
    last_flush: DateTime<Utc>,
    /// Completions since last flush (to persist)
    pending_completions: Vec<TaskCompletion>,
}

impl MetricsCollector {
    fn new() -> Self {
        Self {
            recent: VecDeque::new(),
            aggregates: HashMap::new(),
            last_flush: Utc::now(),
            pending_completions: Vec::new(),
        }
    }

    fn record(&mut self, completion: TaskCompletion) {
        // Update running aggregate
        let aggregate = self
            .aggregates
            .entry(completion.task_type.clone())
            .or_default();
        aggregate.update(&completion);

        // Keep in recent window
        self.recent.push_back(completion.clone());
        if self.recent.len() > MAX_RECENT_COMPLETIONS {
            self.recent.pop_front();
        }

        // Add to pending for persistence
        self.pending_completions.push(completion);
    }

    fn take_pending(&mut self) -> Vec<TaskCompletion> {
        std::mem::take(&mut self.pending_completions)
    }

    fn clear(&mut self) {
        self.recent.clear();
        self.aggregates.clear();
        self.pending_completions.clear();
        self.last_flush = Utc::now();
    }
}

/// Task metrics service for collecting and aggregating task performance data
#[derive(Clone)]
pub struct TaskMetricsService {
    collector: Arc<RwLock<MetricsCollector>>,
    db: DatabaseConnection,
    settings: Arc<SettingsService>,
}

impl TaskMetricsService {
    /// Create a new task metrics service
    pub fn new(db: DatabaseConnection, settings: Arc<SettingsService>) -> Self {
        Self {
            collector: Arc::new(RwLock::new(MetricsCollector::new())),
            db,
            settings,
        }
    }

    /// Check if metrics persistence is enabled
    async fn is_persistence_enabled(&self) -> bool {
        let retention = self
            .settings
            .get_string("metrics.task_retention_days", "30")
            .await
            .unwrap_or_else(|_| "30".to_string());
        retention != "disabled"
    }

    /// Get the retention period in days
    async fn retention_days(&self) -> Option<i64> {
        let retention = self
            .settings
            .get_string("metrics.task_retention_days", "30")
            .await
            .unwrap_or_else(|_| "30".to_string());

        match retention.as_str() {
            "disabled" => None,
            s => s.parse().ok(),
        }
    }

    /// Record a task completion
    #[allow(clippy::too_many_arguments)] // All fields describe a single task completion - maps 1:1 to internal TaskCompletion struct
    pub async fn record(
        &self,
        task_type: String,
        library_id: Option<Uuid>,
        success: bool,
        retried: bool,
        duration_ms: i64,
        queue_wait_ms: i64,
        items_processed: i64,
        bytes_processed: i64,
        error: Option<String>,
    ) {
        let completion = TaskCompletion {
            task_type,
            library_id,
            success,
            retried,
            duration_ms,
            queue_wait_ms,
            items_processed,
            bytes_processed,
            error,
            completed_at: Utc::now(),
        };

        let mut collector = self.collector.write().await;
        collector.record(completion);
    }

    /// Get the count of pending completions waiting to be flushed
    #[cfg(test)]
    pub async fn pending_completions(&self) -> usize {
        let collector = self.collector.read().await;
        collector.pending_completions.len()
    }

    /// Flush pending metrics to the database
    pub async fn flush(&self) -> Result<u64> {
        // Check if persistence is enabled
        if !self.is_persistence_enabled().await {
            // Clear pending without persisting
            let mut collector = self.collector.write().await;
            let count = collector.pending_completions.len();
            collector.pending_completions.clear();
            collector.last_flush = Utc::now();
            return Ok(count as u64);
        }

        // Take pending completions
        let pending = {
            let mut collector = self.collector.write().await;
            collector.last_flush = Utc::now();
            collector.take_pending()
        };

        let count = pending.len();
        if count == 0 {
            return Ok(0);
        }

        // Persist each completion
        for completion in pending {
            let data = TaskCompletionData {
                task_type: completion.task_type,
                library_id: completion.library_id,
                success: completion.success,
                retried: completion.retried,
                duration_ms: completion.duration_ms,
                queue_wait_ms: completion.queue_wait_ms,
                items_processed: completion.items_processed,
                bytes_processed: completion.bytes_processed,
                error: completion.error,
            };

            if let Err(e) = TaskMetricsRepository::record_completion(&self.db, data).await {
                error!("Failed to persist task metrics: {}", e);
            }
        }

        debug!("Flushed {} task metrics to database", count);
        Ok(count as u64)
    }

    /// Run cleanup of old metrics based on retention setting
    pub async fn cleanup(&self) -> Result<u64> {
        match self.retention_days().await {
            Some(days) => {
                let deleted = TaskMetricsRepository::cleanup_old_metrics(&self.db, days).await?;
                Ok(deleted)
            }
            None => Ok(0), // Disabled, nothing to clean
        }
    }

    /// Run rollup of hourly metrics to daily
    pub async fn rollup(&self) -> Result<u64> {
        if !self.is_persistence_enabled().await {
            return Ok(0);
        }
        TaskMetricsRepository::rollup_hourly_to_daily(&self.db).await
    }

    /// Delete all metrics data
    pub async fn nuke_all(&self) -> Result<u64> {
        // Clear in-memory data
        {
            let mut collector = self.collector.write().await;
            collector.clear();
        }

        // Delete from database
        TaskMetricsRepository::nuke_all_metrics(&self.db).await
    }

    /// Get current aggregates by task type (combines database + in-memory data)
    pub async fn get_current_aggregates(&self) -> HashMap<String, TaskTypeMetrics> {
        let mut result = HashMap::new();

        // First, get full database records from the last 24 hours
        // We use get_current_aggregates (which returns full Model records) instead of
        // get_aggregated_by_type (which only returns SUMs) so we can access duration_samples
        if let Ok(db_records) = TaskMetricsRepository::get_current_aggregates(&self.db).await {
            // Group records by task_type and aggregate manually
            let mut task_aggregates: HashMap<String, DbTaskAggregate> = HashMap::new();

            for record in db_records {
                let entry = task_aggregates.entry(record.task_type.clone()).or_default();

                entry.count += record.count as u64;
                entry.succeeded += record.succeeded as u64;
                entry.failed += record.failed as u64;
                entry.retried += record.retried as u64;
                entry.total_duration_ms += record.total_duration_ms;
                entry.total_queue_wait_ms += record.total_queue_wait_ms;
                entry.items_processed += record.items_processed as u64;
                entry.bytes_processed += record.bytes_processed as u64;

                if let Some(min) = record.min_duration_ms {
                    entry.min_duration_ms = Some(entry.min_duration_ms.map_or(min, |m| m.min(min)));
                }
                if let Some(max) = record.max_duration_ms {
                    entry.max_duration_ms = Some(entry.max_duration_ms.map_or(max, |m| m.max(max)));
                }

                // Extract and merge duration_samples
                if let Some(ref samples_json) = record.duration_samples
                    && let Ok(samples) = serde_json::from_value::<Vec<i64>>(samples_json.clone())
                {
                    entry.duration_samples.extend(samples);
                }

                // Track most recent error
                if let Some(ref error_at) = record.last_error_at
                    && entry
                        .last_error_at
                        .is_none_or(|existing| error_at > &existing)
                {
                    entry.last_error = record.last_error.clone();
                    entry.last_error_at = Some(*error_at);
                }
            }

            // Convert aggregates to TaskTypeMetrics
            for (task_type, agg) in task_aggregates {
                let count = agg.count;
                let (p50, p95) = calculate_percentiles_from_samples(&agg.duration_samples);

                result.insert(
                    task_type,
                    TaskTypeMetrics {
                        executed: count,
                        succeeded: agg.succeeded,
                        failed: agg.failed,
                        retried: agg.retried,
                        avg_duration_ms: if count > 0 {
                            agg.total_duration_ms as f64 / count as f64
                        } else {
                            0.0
                        },
                        min_duration_ms: agg.min_duration_ms.unwrap_or(0) as u64,
                        max_duration_ms: agg.max_duration_ms.unwrap_or(0) as u64,
                        p50_duration_ms: p50,
                        p95_duration_ms: p95,
                        avg_queue_wait_ms: if count > 0 {
                            agg.total_queue_wait_ms as f64 / count as f64
                        } else {
                            0.0
                        },
                        items_processed: agg.items_processed,
                        bytes_processed: agg.bytes_processed,
                        throughput_per_sec: 0.0, // Calculated later with time window
                        error_rate_pct: if count > 0 {
                            (agg.failed as f64 / count as f64) * 100.0
                        } else {
                            0.0
                        },
                        last_error: agg.last_error,
                        last_error_at: agg.last_error_at,
                    },
                );
            }
        }

        // Then merge with in-memory aggregates (which may have more recent data not yet flushed)
        let collector = self.collector.read().await;
        for (task_type, agg) in &collector.aggregates {
            // In-memory data represents unflushed completions since last flush
            // We add these to any existing database counts
            if let Some(existing) = result.get_mut(task_type) {
                existing.executed += agg.count;
                existing.succeeded += agg.succeeded;
                existing.failed += agg.failed;
                existing.retried += agg.retried;

                let total_count = existing.executed;
                let new_total_duration = (existing.avg_duration_ms
                    * (total_count - agg.count) as f64)
                    + agg.total_duration_ms as f64;
                existing.avg_duration_ms = new_total_duration / total_count as f64;

                let new_total_queue_wait = (existing.avg_queue_wait_ms
                    * (total_count - agg.count) as f64)
                    + agg.total_queue_wait_ms as f64;
                existing.avg_queue_wait_ms = new_total_queue_wait / total_count as f64;

                existing.items_processed += agg.items_processed as u64;
                existing.bytes_processed += agg.bytes_processed as u64;

                // Update min/max
                if let Some(min) = agg.min_duration_ms {
                    existing.min_duration_ms = existing.min_duration_ms.min(min as u64);
                }
                if let Some(max) = agg.max_duration_ms {
                    existing.max_duration_ms = existing.max_duration_ms.max(max as u64);
                }

                // Use in-memory percentiles since they're from recent samples
                existing.p50_duration_ms = agg.percentile(50.0).unwrap_or(0) as u64;
                existing.p95_duration_ms = agg.percentile(95.0).unwrap_or(0) as u64;

                existing.error_rate_pct = if total_count > 0 {
                    (existing.failed as f64 / total_count as f64) * 100.0
                } else {
                    0.0
                };

                // Use in-memory error if more recent
                if agg.last_error_at.is_some() {
                    existing.last_error = agg.last_error.clone();
                    existing.last_error_at = agg.last_error_at;
                }
            } else {
                // No database data for this task type, use in-memory only
                result.insert(
                    task_type.clone(),
                    TaskTypeMetrics {
                        executed: agg.count,
                        succeeded: agg.succeeded,
                        failed: agg.failed,
                        retried: agg.retried,
                        avg_duration_ms: if agg.count > 0 {
                            agg.total_duration_ms as f64 / agg.count as f64
                        } else {
                            0.0
                        },
                        min_duration_ms: agg.min_duration_ms.unwrap_or(0) as u64,
                        max_duration_ms: agg.max_duration_ms.unwrap_or(0) as u64,
                        p50_duration_ms: agg.percentile(50.0).unwrap_or(0) as u64,
                        p95_duration_ms: agg.percentile(95.0).unwrap_or(0) as u64,
                        avg_queue_wait_ms: if agg.count > 0 {
                            agg.total_queue_wait_ms as f64 / agg.count as f64
                        } else {
                            0.0
                        },
                        items_processed: agg.items_processed as u64,
                        bytes_processed: agg.bytes_processed as u64,
                        throughput_per_sec: 0.0,
                        error_rate_pct: if agg.count > 0 {
                            (agg.failed as f64 / agg.count as f64) * 100.0
                        } else {
                            0.0
                        },
                        last_error: agg.last_error.clone(),
                        last_error_at: agg.last_error_at,
                    },
                );
            }
        }

        result
    }

    /// Get summary statistics (combines database + in-memory data)
    pub async fn get_summary(&self) -> TaskMetricsSummary {
        let mut total_executed = 0u64;
        let mut total_succeeded = 0u64;
        let mut total_failed = 0u64;
        let mut total_duration_ms = 0i64;
        let mut total_queue_wait_ms = 0i64;

        // Get database aggregates from the last 24 hours
        let from = Utc::now() - Duration::hours(24);
        let to = Utc::now();

        if let Ok(db_aggregates) =
            TaskMetricsRepository::get_aggregated_by_type(&self.db, from, to).await
        {
            for agg in db_aggregates {
                total_executed += agg.total_count as u64;
                total_succeeded += agg.total_succeeded as u64;
                total_failed += agg.total_failed as u64;
                total_duration_ms += agg.sum_duration_ms;
                total_queue_wait_ms += agg.sum_queue_wait_ms;
            }
        }

        // Add in-memory data (not yet flushed)
        let collector = self.collector.read().await;
        for agg in collector.aggregates.values() {
            total_executed += agg.count;
            total_succeeded += agg.succeeded;
            total_failed += agg.failed;
            total_duration_ms += agg.total_duration_ms;
            total_queue_wait_ms += agg.total_queue_wait_ms;
        }

        // Calculate throughput based on recent completions
        let tasks_per_minute = if !collector.recent.is_empty() {
            let oldest = collector.recent.front().map(|c| c.completed_at);
            let newest = collector.recent.back().map(|c| c.completed_at);

            if let (Some(oldest), Some(newest)) = (oldest, newest) {
                let duration_mins = (newest - oldest).num_seconds() as f64 / 60.0;
                if duration_mins > 0.0 {
                    collector.recent.len() as f64 / duration_mins
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            0.0
        };

        TaskMetricsSummary {
            total_executed,
            total_succeeded,
            total_failed,
            avg_duration_ms: if total_executed > 0 {
                total_duration_ms as f64 / total_executed as f64
            } else {
                0.0
            },
            avg_queue_wait_ms: if total_executed > 0 {
                total_queue_wait_ms as f64 / total_executed as f64
            } else {
                0.0
            },
            tasks_per_minute,
        }
    }

    /// Get historical metrics from database
    pub async fn get_history(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        task_type: Option<&str>,
        granularity: &str,
    ) -> Result<Vec<TaskMetricsDataPoint>> {
        let records =
            TaskMetricsRepository::get_metrics_history(&self.db, from, to, task_type, granularity)
                .await?;

        let points = records
            .into_iter()
            .map(|r| TaskMetricsDataPoint {
                period_start: r.period_start,
                task_type: Some(r.task_type),
                count: r.count as u64,
                succeeded: r.succeeded as u64,
                failed: r.failed as u64,
                avg_duration_ms: if r.count > 0 {
                    r.total_duration_ms as f64 / r.count as f64
                } else {
                    0.0
                },
                min_duration_ms: r.min_duration_ms.unwrap_or(0) as u64,
                max_duration_ms: r.max_duration_ms.unwrap_or(0) as u64,
                items_processed: r.items_processed as u64,
                bytes_processed: r.bytes_processed as u64,
            })
            .collect();

        Ok(points)
    }

    /// Get retention setting value
    pub async fn get_retention_setting(&self) -> String {
        self.settings
            .get_string("metrics.task_retention_days", "30")
            .await
            .unwrap_or_else(|_| "30".to_string())
    }

    /// Get oldest metric timestamp
    pub async fn get_oldest_metric(&self) -> Result<Option<DateTime<Utc>>> {
        TaskMetricsRepository::get_oldest_metric(&self.db).await
    }

    /// Start background jobs for flushing, cleanup, and rollup
    ///
    /// Accepts a `CancellationToken` for graceful shutdown support.
    /// Returns `JoinHandle`s for each background task that can be awaited on shutdown.
    pub fn start_background_jobs(
        self: Arc<Self>,
        cancel_token: CancellationToken,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let mut handles = Vec::with_capacity(3);

        // Flush job - every 1 minute
        {
            let service = self.clone();
            let token = cancel_token.clone();
            handles.push(tokio::spawn(async move {
                let mut flush_interval = interval(TokioDuration::from_secs(60));

                loop {
                    tokio::select! {
                        _ = token.cancelled() => {
                            // Final flush before shutdown
                            debug!("Task metrics flush job shutting down, performing final flush");
                            if let Err(e) = service.flush().await {
                                error!("Failed to flush task metrics during shutdown: {}", e);
                            }
                            break;
                        }
                        _ = flush_interval.tick() => {
                            if let Err(e) = service.flush().await {
                                error!("Failed to flush task metrics: {}", e);
                            }
                        }
                    }
                }
            }));
        }

        // Cleanup job - every hour
        {
            let service = self.clone();
            let token = cancel_token.clone();
            handles.push(tokio::spawn(async move {
                let mut cleanup_interval = interval(TokioDuration::from_secs(3600));

                loop {
                    tokio::select! {
                        _ = token.cancelled() => {
                            debug!("Task metrics cleanup job shutting down");
                            break;
                        }
                        _ = cleanup_interval.tick() => {
                            if let Err(e) = service.cleanup().await {
                                error!("Failed to cleanup task metrics: {}", e);
                            }
                        }
                    }
                }
            }));
        }

        // Rollup job - every day at midnight (check hourly)
        {
            let service = self;
            let token = cancel_token;
            handles.push(tokio::spawn(async move {
                let mut rollup_interval = interval(TokioDuration::from_secs(3600));

                loop {
                    tokio::select! {
                        _ = token.cancelled() => {
                            debug!("Task metrics rollup job shutting down");
                            break;
                        }
                        _ = rollup_interval.tick() => {
                            // Only run rollup at midnight (hour 0)
                            let now = Utc::now();
                            if now.time().hour() == 0
                                && let Err(e) = service.rollup().await {
                                    error!("Failed to rollup task metrics: {}", e);
                                }
                        }
                    }
                }
            }));
        }

        handles
    }
}

/// Summary metrics for all task types
#[derive(Debug, Clone)]
pub struct TaskMetricsSummary {
    pub total_executed: u64,
    pub total_succeeded: u64,
    pub total_failed: u64,
    pub avg_duration_ms: f64,
    pub avg_queue_wait_ms: f64,
    pub tasks_per_minute: f64,
}

/// Metrics for a specific task type
#[derive(Debug, Clone)]
pub struct TaskTypeMetrics {
    pub executed: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub retried: u64,
    pub avg_duration_ms: f64,
    pub min_duration_ms: u64,
    pub max_duration_ms: u64,
    pub p50_duration_ms: u64,
    pub p95_duration_ms: u64,
    pub avg_queue_wait_ms: f64,
    pub items_processed: u64,
    pub bytes_processed: u64,
    pub throughput_per_sec: f64,
    pub error_rate_pct: f64,
    pub last_error: Option<String>,
    pub last_error_at: Option<DateTime<Utc>>,
}

/// Historical data point
#[derive(Debug, Clone)]
pub struct TaskMetricsDataPoint {
    pub period_start: DateTime<Utc>,
    pub task_type: Option<String>,
    pub count: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub avg_duration_ms: f64,
    pub min_duration_ms: u64,
    pub max_duration_ms: u64,
    pub items_processed: u64,
    pub bytes_processed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::setup_test_db;

    async fn create_test_service() -> TaskMetricsService {
        let db = setup_test_db().await;
        let settings = Arc::new(SettingsService::new(db.clone()).await.unwrap());
        TaskMetricsService::new(db, settings)
    }

    #[tokio::test]
    async fn test_record_completion() {
        let service = create_test_service().await;

        service
            .record(
                "scan_library".to_string(),
                None,
                true,
                false,
                1000,
                50,
                10,
                1024,
                None,
            )
            .await;

        let aggregates = service.get_current_aggregates().await;
        assert!(aggregates.contains_key("scan_library"));

        let scan_metrics = &aggregates["scan_library"];
        assert_eq!(scan_metrics.executed, 1);
        assert_eq!(scan_metrics.succeeded, 1);
        assert_eq!(scan_metrics.failed, 0);
    }

    #[tokio::test]
    async fn test_record_multiple_completions() {
        let service = create_test_service().await;

        // Record successful completion
        service
            .record(
                "analyze_book".to_string(),
                None,
                true,
                false,
                500,
                25,
                1,
                512,
                None,
            )
            .await;

        // Record failed completion
        service
            .record(
                "analyze_book".to_string(),
                None,
                false,
                true,
                1500,
                100,
                0,
                0,
                Some("Test error".to_string()),
            )
            .await;

        let aggregates = service.get_current_aggregates().await;
        let metrics = &aggregates["analyze_book"];

        assert_eq!(metrics.executed, 2);
        assert_eq!(metrics.succeeded, 1);
        assert_eq!(metrics.failed, 1);
        assert_eq!(metrics.retried, 1);
        assert!(metrics.last_error.is_some());
    }

    #[tokio::test]
    async fn test_summary() {
        let service = create_test_service().await;

        for i in 0..5 {
            service
                .record(
                    format!("task_{}", i % 2),
                    None,
                    i % 3 != 0,
                    false,
                    100 * (i + 1) as i64,
                    10,
                    1,
                    100,
                    None,
                )
                .await;
        }

        let summary = service.get_summary().await;
        assert_eq!(summary.total_executed, 5);
        assert!(summary.total_succeeded > 0);
        assert!(summary.avg_duration_ms > 0.0);
    }

    #[tokio::test]
    async fn test_flush() {
        let service = create_test_service().await;

        service
            .record(
                "test_task".to_string(),
                None,
                true,
                false,
                100,
                10,
                1,
                100,
                None,
            )
            .await;

        let flushed = service.flush().await.expect("Flush failed");
        assert_eq!(flushed, 1);

        // Second flush should return 0 (nothing pending)
        let flushed = service.flush().await.expect("Flush failed");
        assert_eq!(flushed, 0);
    }

    #[tokio::test]
    async fn test_nuke_all() {
        let service = create_test_service().await;

        // Record some data
        for i in 0..3 {
            service
                .record(
                    format!("task_{}", i),
                    None,
                    true,
                    false,
                    100,
                    10,
                    1,
                    100,
                    None,
                )
                .await;
        }

        // Flush to database
        service.flush().await.expect("Flush failed");

        // Nuke all
        service.nuke_all().await.expect("Nuke failed");

        // Verify in-memory is cleared
        let aggregates = service.get_current_aggregates().await;
        assert!(aggregates.is_empty());
    }

    #[tokio::test]
    async fn test_percentile_calculation() {
        let service = create_test_service().await;

        // Record completions with varying durations
        let durations = vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];
        for d in durations {
            service
                .record(
                    "percentile_test".to_string(),
                    None,
                    true,
                    false,
                    d,
                    10,
                    1,
                    100,
                    None,
                )
                .await;
        }

        let aggregates = service.get_current_aggregates().await;
        let metrics = &aggregates["percentile_test"];

        // P50 should be around 500-600
        assert!(metrics.p50_duration_ms >= 500);
        assert!(metrics.p50_duration_ms <= 600);

        // P95 should be around 900-1000
        assert!(metrics.p95_duration_ms >= 900);
        assert!(metrics.p95_duration_ms <= 1000);
    }

    #[tokio::test]
    async fn test_percentile_calculation_from_db() {
        let service = create_test_service().await;

        // Record completions with varying durations
        let durations = vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];
        for d in durations {
            service
                .record(
                    "db_percentile_test".to_string(),
                    None,
                    true,
                    false,
                    d,
                    10,
                    1,
                    100,
                    None,
                )
                .await;
        }

        // Flush to database - this moves data from in-memory to DB
        let flushed = service.flush().await.expect("Flush failed");
        assert_eq!(flushed, 10);

        // Get aggregates - this should retrieve data from DB and calculate percentiles
        // from the duration_samples stored in the DB
        let aggregates = service.get_current_aggregates().await;
        let metrics = &aggregates["db_percentile_test"];

        // P50 should be around 500-600 (NOT 0 as it was before the fix)
        assert!(
            metrics.p50_duration_ms >= 500,
            "P50 should be >= 500, but got {}",
            metrics.p50_duration_ms
        );
        assert!(
            metrics.p50_duration_ms <= 600,
            "P50 should be <= 600, but got {}",
            metrics.p50_duration_ms
        );

        // P95 should be around 900-1000 (NOT 0 as it was before the fix)
        assert!(
            metrics.p95_duration_ms >= 900,
            "P95 should be >= 900, but got {}",
            metrics.p95_duration_ms
        );
        assert!(
            metrics.p95_duration_ms <= 1000,
            "P95 should be <= 1000, but got {}",
            metrics.p95_duration_ms
        );
    }

    #[tokio::test]
    async fn test_background_jobs_graceful_shutdown() {
        let db = setup_test_db().await;
        let settings = Arc::new(SettingsService::new(db.clone()).await.unwrap());
        let service = Arc::new(TaskMetricsService::new(db, settings));

        // Record some data to ensure flush has something to process during shutdown
        service
            .record(
                "shutdown_test".to_string(),
                None,
                true,
                false,
                100,
                10,
                1,
                100,
                None,
            )
            .await;

        // Create a cancellation token
        let cancel_token = CancellationToken::new();

        // Start background jobs
        let handles = service.clone().start_background_jobs(cancel_token.clone());

        // Verify we got 3 handles (flush, cleanup, rollup)
        assert_eq!(handles.len(), 3);

        // Let them run for a bit
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Verify all tasks are still running
        for handle in &handles {
            assert!(!handle.is_finished());
        }

        // Cancel and wait for graceful shutdown
        cancel_token.cancel();

        // All tasks should complete within a reasonable time
        for (i, handle) in handles.into_iter().enumerate() {
            let result = tokio::time::timeout(tokio::time::Duration::from_secs(5), handle).await;
            assert!(
                result.is_ok(),
                "Background job {} did not shutdown in time",
                i
            );
            assert!(
                result.unwrap().is_ok(),
                "Background job {} panicked during shutdown",
                i
            );
        }
    }

    #[tokio::test]
    async fn test_background_jobs_final_flush_on_shutdown() {
        let db = setup_test_db().await;
        let settings = Arc::new(SettingsService::new(db.clone()).await.unwrap());
        let service = Arc::new(TaskMetricsService::new(db, settings));

        // Record some data
        service
            .record(
                "final_flush_test".to_string(),
                None,
                true,
                false,
                100,
                10,
                1,
                100,
                None,
            )
            .await;

        // Verify data is pending
        let pending = service.pending_completions().await;
        assert_eq!(pending, 1, "Should have 1 pending completion");

        // Create a cancellation token
        let cancel_token = CancellationToken::new();

        // Start background jobs
        let handles = service.clone().start_background_jobs(cancel_token.clone());

        // Give the tasks time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Cancel immediately (before the first interval tick)
        cancel_token.cancel();

        // Wait for all tasks to complete
        for handle in handles {
            let _ = handle.await;
        }

        // After shutdown, the flush task should have performed a final flush
        // The pending completions should be 0 since they were flushed
        let pending = service.pending_completions().await;
        assert_eq!(
            pending, 0,
            "Pending completions should be 0 after final flush during shutdown"
        );
    }
}
