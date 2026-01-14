use axum::{extract::Query, extract::State, Json};
use chrono::{Duration, Utc};
use std::sync::Arc;

use crate::api::{
    dto::{
        MetricsCleanupResponse, MetricsNukeResponse, QueueHealthMetricsDto,
        TaskMetricsDataPointDto, TaskMetricsHistoryQuery, TaskMetricsHistoryResponse,
        TaskMetricsResponse, TaskMetricsSummaryDto, TaskTypeMetricsDto,
    },
    error::ApiError,
    extractors::AuthContext,
    permissions::Permission,
};
use crate::db::repositories::TaskRepository;

use super::AppState;

/// Get current task metrics
///
/// Returns real-time task performance statistics including:
/// - Summary metrics across all task types
/// - Per-task-type breakdown with timing, throughput, and error rates
/// - Queue health metrics (pending, processing, stale counts)
///
/// # Permission Required
/// - `libraries:read` or admin status
#[utoipa::path(
    get,
    path = "/api/v1/metrics/tasks",
    responses(
        (status = 200, description = "Task metrics retrieved successfully", body = TaskMetricsResponse),
        (status = 403, description = "Permission denied"),
        (status = 503, description = "Task metrics service not available"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Metrics"
)]
pub async fn get_task_metrics(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<TaskMetricsResponse>, ApiError> {
    auth.require_permission(&Permission::LibrariesRead)?;

    let metrics_service = state
        .task_metrics_service
        .as_ref()
        .ok_or_else(|| ApiError::Internal("Task metrics service not available".to_string()))?;

    // Get current aggregates and summary in parallel
    let (summary, aggregates, retention, queue_stats) = tokio::try_join!(
        async { Ok::<_, anyhow::Error>(metrics_service.get_summary().await) },
        async { Ok::<_, anyhow::Error>(metrics_service.get_current_aggregates().await) },
        async { Ok::<_, anyhow::Error>(metrics_service.get_retention_setting().await) },
        async {
            TaskRepository::get_stats(&state.db)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get queue stats: {}", e))
        },
    )
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Convert aggregates to DTOs
    let by_type: Vec<TaskTypeMetricsDto> = aggregates
        .into_iter()
        .map(|(task_type, m)| TaskTypeMetricsDto {
            task_type,
            executed: m.executed,
            succeeded: m.succeeded,
            failed: m.failed,
            retried: m.retried,
            avg_duration_ms: m.avg_duration_ms,
            min_duration_ms: m.min_duration_ms,
            max_duration_ms: m.max_duration_ms,
            p50_duration_ms: m.p50_duration_ms,
            p95_duration_ms: m.p95_duration_ms,
            avg_queue_wait_ms: m.avg_queue_wait_ms,
            items_processed: m.items_processed,
            bytes_processed: m.bytes_processed,
            throughput_per_sec: m.throughput_per_sec,
            error_rate_pct: m.error_rate_pct,
            last_error: m.last_error,
            last_error_at: m.last_error_at,
        })
        .collect();

    // Get oldest pending task age
    let oldest_pending_age_ms = None; // Would need additional query

    Ok(Json(TaskMetricsResponse {
        updated_at: Utc::now(),
        retention,
        summary: TaskMetricsSummaryDto {
            total_executed: summary.total_executed,
            total_succeeded: summary.total_succeeded,
            total_failed: summary.total_failed,
            avg_duration_ms: summary.avg_duration_ms,
            avg_queue_wait_ms: summary.avg_queue_wait_ms,
            tasks_per_minute: summary.tasks_per_minute,
        },
        by_type,
        queue: QueueHealthMetricsDto {
            pending_count: queue_stats.pending,
            processing_count: queue_stats.processing,
            stale_count: queue_stats.stale,
            oldest_pending_age_ms,
        },
    }))
}

/// Get task metrics history
///
/// Returns historical task performance data for trend analysis.
/// Data is aggregated by hour or day depending on the granularity parameter.
///
/// # Permission Required
/// - `libraries:read` or admin status
#[utoipa::path(
    get,
    path = "/api/v1/metrics/tasks/history",
    params(TaskMetricsHistoryQuery),
    responses(
        (status = 200, description = "Task metrics history retrieved successfully", body = TaskMetricsHistoryResponse),
        (status = 403, description = "Permission denied"),
        (status = 503, description = "Task metrics service not available"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Metrics"
)]
pub async fn get_task_metrics_history(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Query(query): Query<TaskMetricsHistoryQuery>,
) -> Result<Json<TaskMetricsHistoryResponse>, ApiError> {
    auth.require_permission(&Permission::LibrariesRead)?;

    let metrics_service = state
        .task_metrics_service
        .as_ref()
        .ok_or_else(|| ApiError::Internal("Task metrics service not available".to_string()))?;

    let days = query.days.unwrap_or(7).clamp(1, 180);
    let granularity = query.granularity.as_deref().unwrap_or("hour");
    let task_type = query.task_type.as_deref();

    let to = Utc::now();
    let from = to - Duration::days(days as i64);

    let history = metrics_service
        .get_history(from, to, task_type, granularity)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get metrics history: {}", e)))?;

    let points: Vec<TaskMetricsDataPointDto> = history
        .into_iter()
        .map(|p| TaskMetricsDataPointDto {
            period_start: p.period_start,
            task_type: p.task_type,
            count: p.count,
            succeeded: p.succeeded,
            failed: p.failed,
            avg_duration_ms: p.avg_duration_ms,
            min_duration_ms: p.min_duration_ms,
            max_duration_ms: p.max_duration_ms,
            items_processed: p.items_processed,
            bytes_processed: p.bytes_processed,
        })
        .collect();

    Ok(Json(TaskMetricsHistoryResponse {
        from,
        to,
        granularity: granularity.to_string(),
        points,
    }))
}

/// Trigger manual metrics cleanup
///
/// Deletes metric records older than the configured retention period.
/// This operation normally runs automatically daily.
///
/// # Permission Required
/// - Admin status required
#[utoipa::path(
    post,
    path = "/api/v1/metrics/tasks/cleanup",
    responses(
        (status = 200, description = "Cleanup completed successfully", body = MetricsCleanupResponse),
        (status = 403, description = "Permission denied - admin required"),
        (status = 503, description = "Task metrics service not available"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Metrics"
)]
pub async fn trigger_metrics_cleanup(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<MetricsCleanupResponse>, ApiError> {
    auth.require_admin()?;

    let metrics_service = state
        .task_metrics_service
        .as_ref()
        .ok_or_else(|| ApiError::Internal("Task metrics service not available".to_string()))?;

    let deleted_count = metrics_service
        .cleanup()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to cleanup metrics: {}", e)))?;

    let retention_days = metrics_service.get_retention_setting().await;
    let oldest_remaining = metrics_service
        .get_oldest_metric()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get oldest metric: {}", e)))?;

    Ok(Json(MetricsCleanupResponse {
        deleted_count,
        retention_days,
        oldest_remaining,
    }))
}

/// Delete all task metrics
///
/// Permanently deletes all task metric records from the database
/// and clears in-memory aggregates. This action cannot be undone.
///
/// # Permission Required
/// - Admin status required
#[utoipa::path(
    delete,
    path = "/api/v1/metrics/tasks",
    responses(
        (status = 200, description = "All metrics deleted successfully", body = MetricsNukeResponse),
        (status = 403, description = "Permission denied - admin required"),
        (status = 503, description = "Task metrics service not available"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Metrics"
)]
pub async fn nuke_task_metrics(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<MetricsNukeResponse>, ApiError> {
    auth.require_admin()?;

    let metrics_service = state
        .task_metrics_service
        .as_ref()
        .ok_or_else(|| ApiError::Internal("Task metrics service not available".to_string()))?;

    let deleted_count = metrics_service
        .nuke_all()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to nuke metrics: {}", e)))?;

    Ok(Json(MetricsNukeResponse { deleted_count }))
}
