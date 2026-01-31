use axum::{extract::State, Json};
use chrono::Utc;
use std::sync::Arc;

use super::super::dto::{
    LibraryMetricsDto, MetricsDto, PluginMethodMetricsDto, PluginMetricsDto, PluginMetricsResponse,
    PluginMetricsSummaryDto,
};
use crate::api::{error::ApiError, extractors::AuthContext, permissions::Permission, AppState};
use crate::db::repositories::MetricsRepository;

/// Get inventory metrics (library/book counts)
///
/// Returns counts and sizes for libraries, series, and books in the system.
/// This endpoint provides an inventory overview of your digital library.
///
/// # Permission Required
/// - `libraries:read` or admin status
#[utoipa::path(
    get,
    path = "/api/v1/metrics/inventory",
    responses(
        (status = 200, description = "Inventory metrics retrieved successfully", body = MetricsDto),
        (status = 403, description = "Permission denied"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Metrics"
)]
pub async fn get_inventory_metrics(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<MetricsDto>, ApiError> {
    // Check permission - metrics are considered system-level library information
    auth.require_permission(&Permission::LibrariesRead)?;

    // Gather all metrics in parallel
    let (
        library_count,
        series_count,
        book_count,
        total_book_size,
        user_count,
        database_size,
        page_count,
        library_metrics,
    ) = tokio::try_join!(
        async {
            MetricsRepository::count_libraries(&state.db)
                .await
                .map_err(|e| anyhow::anyhow!("count_libraries: {}", e))
        },
        async {
            MetricsRepository::count_series(&state.db)
                .await
                .map_err(|e| anyhow::anyhow!("count_series: {}", e))
        },
        async {
            MetricsRepository::count_books(&state.db)
                .await
                .map_err(|e| anyhow::anyhow!("count_books: {}", e))
        },
        async {
            MetricsRepository::total_book_size(&state.db)
                .await
                .map_err(|e| anyhow::anyhow!("total_book_size: {}", e))
        },
        async {
            MetricsRepository::count_users(&state.db)
                .await
                .map_err(|e| anyhow::anyhow!("count_users: {}", e))
        },
        async {
            MetricsRepository::database_size(&state.db)
                .await
                .map_err(|e| anyhow::anyhow!("database_size: {}", e))
        },
        async {
            MetricsRepository::count_pages(&state.db)
                .await
                .map_err(|e| anyhow::anyhow!("count_pages: {}", e))
        },
        async {
            MetricsRepository::library_metrics(&state.db)
                .await
                .map_err(|e| anyhow::anyhow!("library_metrics: {}", e))
        },
    )
    .map_err(|e| ApiError::Internal(format!("Failed to gather metrics: {}", e)))?;

    // Convert library metrics to DTOs
    let libraries = library_metrics
        .into_iter()
        .map(|m| LibraryMetricsDto {
            id: m.id,
            name: m.name,
            series_count: m.series_count,
            book_count: m.book_count,
            total_size: m.total_size,
        })
        .collect();

    Ok(Json(MetricsDto {
        library_count,
        series_count,
        book_count,
        total_book_size,
        user_count,
        database_size,
        page_count,
        libraries,
    }))
}

/// Get plugin metrics
///
/// Returns real-time performance statistics for all plugins including:
/// - Summary metrics across all plugins
/// - Per-plugin breakdown with timing, error rates, and health status
/// - Per-method breakdown within each plugin
///
/// # Permission Required
/// - `libraries:read` or admin status
#[utoipa::path(
    get,
    path = "/api/v1/metrics/plugins",
    responses(
        (status = 200, description = "Plugin metrics retrieved successfully", body = PluginMetricsResponse),
        (status = 403, description = "Permission denied"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Metrics"
)]
pub async fn get_plugin_metrics(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<PluginMetricsResponse>, ApiError> {
    auth.require_permission(&Permission::LibrariesRead)?;

    let summary = state.plugin_metrics_service.get_summary().await;
    let plugin_snapshots = state.plugin_metrics_service.get_all_metrics().await;

    // Convert snapshots to DTOs
    let plugins: Vec<PluginMetricsDto> = plugin_snapshots
        .into_iter()
        .map(|snapshot| {
            let by_method = if snapshot.by_method.is_empty() {
                None
            } else {
                Some(
                    snapshot
                        .by_method
                        .into_iter()
                        .map(|(name, m)| {
                            (
                                name,
                                PluginMethodMetricsDto {
                                    method: m.method,
                                    requests_total: m.requests_total,
                                    requests_success: m.requests_success,
                                    requests_failed: m.requests_failed,
                                    avg_duration_ms: m.avg_duration_ms,
                                },
                            )
                        })
                        .collect(),
                )
            };

            let failure_counts = if snapshot.failure_counts.is_empty() {
                None
            } else {
                Some(snapshot.failure_counts)
            };

            PluginMetricsDto {
                plugin_id: snapshot.plugin_id,
                plugin_name: snapshot.plugin_name,
                requests_total: snapshot.requests_total,
                requests_success: snapshot.requests_success,
                requests_failed: snapshot.requests_failed,
                avg_duration_ms: snapshot.avg_duration_ms,
                rate_limit_rejections: snapshot.rate_limit_rejections,
                error_rate_pct: snapshot.error_rate_pct,
                last_success: snapshot.last_success,
                last_failure: snapshot.last_failure,
                health_status: snapshot.health_status.as_str().to_string(),
                by_method,
                failure_counts,
            }
        })
        .collect();

    Ok(Json(PluginMetricsResponse {
        updated_at: Utc::now(),
        summary: PluginMetricsSummaryDto {
            total_plugins: summary.total_plugins,
            healthy_plugins: summary.healthy_plugins,
            degraded_plugins: summary.degraded_plugins,
            unhealthy_plugins: summary.unhealthy_plugins,
            total_requests: summary.total_requests,
            total_success: summary.total_success,
            total_failed: summary.total_failed,
            total_rate_limit_rejections: summary.total_rate_limit_rejections,
        },
        plugins,
    }))
}
