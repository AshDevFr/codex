use axum::{extract::State, Json};
use std::sync::Arc;

use super::super::dto::{LibraryMetricsDto, MetricsDto};
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
