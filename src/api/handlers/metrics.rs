use axum::{extract::State, Json};
use std::sync::Arc;

use crate::api::{
    dto::{LibraryMetricsDto, MetricsDto},
    error::ApiError,
    extractors::AuthContext,
    permissions::Permission,
};
use crate::db::repositories::MetricsRepository;

use super::AppState;

/// Get application metrics
///
/// # Permission Required
/// - `libraries:read` or admin status
#[utoipa::path(
    get,
    path = "/metrics",
    responses(
        (status = 200, description = "Application metrics retrieved successfully", body = MetricsDto),
        (status = 403, description = "Permission denied"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Metrics"
)]
pub async fn get_metrics(
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
        MetricsRepository::count_libraries(&state.db),
        MetricsRepository::count_series(&state.db),
        MetricsRepository::count_books(&state.db),
        MetricsRepository::total_book_size(&state.db),
        MetricsRepository::count_users(&state.db),
        MetricsRepository::database_size(&state.db),
        MetricsRepository::count_pages(&state.db),
        MetricsRepository::library_metrics(&state.db),
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
