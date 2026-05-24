use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Task metrics response - current performance statistics
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskMetricsResponse {
    /// When the metrics were last updated
    #[schema(example = "2026-01-11T12:00:00Z")]
    pub updated_at: DateTime<Utc>,

    /// Current retention setting
    #[schema(example = "30")]
    pub retention: String,

    /// Overall summary statistics
    pub summary: TaskMetricsSummaryDto,

    /// Per-task-type breakdown
    pub by_type: Vec<TaskTypeMetricsDto>,

    /// Queue health metrics
    pub queue: QueueHealthMetricsDto,
}

/// Summary metrics across all task types
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskMetricsSummaryDto {
    /// Total tasks executed since last restart
    #[schema(example = 1250)]
    pub total_executed: u64,

    /// Total successful tasks
    #[schema(example = 1200)]
    pub total_succeeded: u64,

    /// Total failed tasks
    #[schema(example = 50)]
    pub total_failed: u64,

    /// Average duration in milliseconds
    #[schema(example = 1500.5)]
    pub avg_duration_ms: f64,

    /// Average queue wait time in milliseconds
    #[schema(example = 250.0)]
    pub avg_queue_wait_ms: f64,

    /// Tasks processed per minute (recent average)
    #[schema(example = 15.5)]
    pub tasks_per_minute: f64,
}

/// Metrics for a specific task type
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskTypeMetricsDto {
    /// Task type name
    #[schema(example = "scan_library")]
    pub task_type: String,

    /// Number of executions
    #[schema(example = 100)]
    pub executed: u64,

    /// Successful executions
    #[schema(example = 95)]
    pub succeeded: u64,

    /// Failed executions
    #[schema(example = 5)]
    pub failed: u64,

    /// Retried executions
    #[schema(example = 10)]
    pub retried: u64,

    /// Average duration in milliseconds
    #[schema(example = 2500.0)]
    pub avg_duration_ms: f64,

    /// Minimum duration in milliseconds
    #[schema(example = 500)]
    pub min_duration_ms: u64,

    /// Maximum duration in milliseconds
    #[schema(example = 15000)]
    pub max_duration_ms: u64,

    /// 50th percentile (median) duration
    #[schema(example = 2000)]
    pub p50_duration_ms: u64,

    /// 95th percentile duration
    #[schema(example = 8000)]
    pub p95_duration_ms: u64,

    /// Average queue wait time in milliseconds
    #[schema(example = 150.0)]
    pub avg_queue_wait_ms: f64,

    /// Total items processed
    #[schema(example = 5000)]
    pub items_processed: u64,

    /// Total bytes processed
    #[schema(example = 1073741824)]
    pub bytes_processed: u64,

    /// Throughput rate per second
    #[schema(example = 25.5)]
    pub throughput_per_sec: f64,

    /// Error rate as percentage
    #[schema(example = 5.0)]
    pub error_rate_pct: f64,

    /// Most recent error message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,

    /// When the last error occurred
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_at: Option<DateTime<Utc>>,
}

/// Queue health metrics
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct QueueHealthMetricsDto {
    /// Number of tasks waiting to run
    #[schema(example = 25)]
    pub pending_count: u64,

    /// Number of tasks currently executing
    #[schema(example = 4)]
    pub processing_count: u64,

    /// Number of stale/stuck tasks
    #[schema(example = 0)]
    pub stale_count: u64,

    /// Age of oldest pending task in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_pending_age_ms: Option<u64>,
}

/// Task metrics history response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskMetricsHistoryResponse {
    /// Start of the time range
    #[schema(example = "2026-01-04T00:00:00Z")]
    pub from: DateTime<Utc>,

    /// End of the time range
    #[schema(example = "2026-01-11T00:00:00Z")]
    pub to: DateTime<Utc>,

    /// Granularity of the data points
    #[schema(example = "hour")]
    pub granularity: String,

    /// Historical data points
    pub points: Vec<TaskMetricsDataPointDto>,
}

/// A single historical data point
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskMetricsDataPointDto {
    /// Start of this period
    #[schema(example = "2026-01-11T10:00:00Z")]
    pub period_start: DateTime<Utc>,

    /// Task type (if filtered)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,

    /// Number of tasks in this period
    #[schema(example = 50)]
    pub count: u64,

    /// Successful tasks
    #[schema(example = 48)]
    pub succeeded: u64,

    /// Failed tasks
    #[schema(example = 2)]
    pub failed: u64,

    /// Average duration in milliseconds
    #[schema(example = 1200.0)]
    pub avg_duration_ms: f64,

    /// Minimum duration
    #[schema(example = 200)]
    pub min_duration_ms: u64,

    /// Maximum duration
    #[schema(example = 5000)]
    pub max_duration_ms: u64,

    /// Items processed in this period
    #[schema(example = 500)]
    pub items_processed: u64,

    /// Bytes processed in this period
    #[schema(example = 1073741824)]
    pub bytes_processed: u64,
}

/// Query parameters for history endpoint
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct TaskMetricsHistoryQuery {
    /// Number of days to retrieve (default: 7)
    #[param(example = 7)]
    pub days: Option<i32>,

    /// Filter by task type
    #[param(example = "scan_library")]
    pub task_type: Option<String>,

    /// Granularity: "hour" or "day" (default: hour)
    #[param(example = "hour")]
    pub granularity: Option<String>,
}

/// Response for cleanup operation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetricsCleanupResponse {
    /// Number of metric records deleted
    #[schema(example = 500)]
    pub deleted_count: u64,

    /// Current retention setting
    #[schema(example = "30")]
    pub retention_days: String,

    /// Timestamp of oldest remaining record
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_remaining: Option<DateTime<Utc>>,
}

/// Response for nuke (delete all) operation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetricsNukeResponse {
    /// Number of metric records deleted
    #[schema(example = 15000)]
    pub deleted_count: u64,
}
