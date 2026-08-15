//! Reading statistics, aggregated from the session log.

use super::super::dto::{
    DurationBreakdownDto, ReadingByDeviceDto, ReadingByFormatDto, ReadingBySeriesDto,
    ReadingPeriodDto, ReadingStatsGranularity, ReadingStatsQuery, ReadingStatsResponse,
    ReadingSummaryDto,
};
use crate::{AppState, error::ApiError, extractors::AuthContext, permissions::Permission};
use axum::{
    Json,
    extract::{Query, State},
};
use chrono::{Duration, Utc};
use codex_db::repositories::{ReadingStatsRepository, StatsWindow};
use std::sync::Arc;
use utoipa::OpenApi;

/// How far back an unqualified request looks.
///
/// Long enough to show a habit rather than a week's noise, short enough that the
/// daily series is still a sensible size to render.
const DEFAULT_WINDOW_DAYS: i64 = 90;

/// Most series a single response will rank.
///
/// A top-N list stops being useful long before this, and the cap keeps one
/// request from turning into a full table scan of somebody's whole library.
const MAX_SERIES_LIMIT: u64 = 50;
const DEFAULT_SERIES_LIMIT: u64 = 10;

#[derive(OpenApi)]
#[openapi(
    paths(get_reading_stats),
    components(schemas(
        ReadingStatsResponse,
        ReadingSummaryDto,
        ReadingPeriodDto,
        ReadingByDeviceDto,
        ReadingBySeriesDto,
        ReadingByFormatDto,
        DurationBreakdownDto,
        ReadingStatsGranularity,
    )),
    tags(
        (name = "Reading Statistics", description = "Aggregated reading time and pages")
    )
)]
#[allow(dead_code)] // OpenAPI documentation struct - referenced by utoipa derive macros
pub struct ReadingStatsApi;

/// Reading statistics for the authenticated user
///
/// Totals, a time series, and breakdowns by device, series and format, all over
/// one window so no two panels can disagree about which dates they cover.
///
/// Reading time is reported as two figures rather than one. Clients that measure
/// their own sessions report time directly; the Komga-compatible, OPDS and
/// KOReader surfaces cannot, so theirs is reconstructed from the gaps between
/// their writes. That reconstruction undercounts and is blind to reading done
/// from an already-downloaded book, so it is kept separable rather than blended
/// into a total that would quietly overstate its own accuracy.
///
/// Always scoped to the caller. There is no way to read another user's history.
#[utoipa::path(
    get,
    path = "/api/v1/reading-stats",
    params(ReadingStatsQuery),
    responses(
        (status = 200, description = "Reading statistics for the window", body = ReadingStatsResponse),
        (status = 400, description = "The window ends before it starts"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Reading Statistics"
)]
pub async fn get_reading_stats(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Query(query): Query<ReadingStatsQuery>,
) -> Result<Json<ReadingStatsResponse>, ApiError> {
    auth.require_permission(&Permission::ProgressRead)?;

    let to = query.to.unwrap_or_else(Utc::now);
    let from = query
        .from
        .unwrap_or_else(|| to - Duration::days(DEFAULT_WINDOW_DAYS));

    if to < from {
        return Err(ApiError::BadRequest(
            "the statistics window ends before it starts".to_string(),
        ));
    }

    let granularity = query.granularity.unwrap_or(ReadingStatsGranularity::Day);
    let series_limit = query
        .series_limit
        .unwrap_or(DEFAULT_SERIES_LIMIT)
        .clamp(1, MAX_SERIES_LIMIT);

    let window = StatsWindow { from, to };
    let user_id = auth.user_id;

    // Five aggregations over the same window. Run sequentially rather than
    // concurrently: they contend for the same connection pool, and spending
    // five connections to shave milliseconds off a page nobody loads in a loop
    // is a bad trade against every other request in flight.
    let summary = ReadingStatsRepository::summary(&state.db, user_id, window)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to summarise reading: {}", e)))?;
    let periods = ReadingStatsRepository::by_period(&state.db, user_id, window, granularity.into())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to bucket reading: {}", e)))?;
    let devices = ReadingStatsRepository::by_device(&state.db, user_id, window)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to group by device: {}", e)))?;
    let series = ReadingStatsRepository::by_series(&state.db, user_id, window, series_limit)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to group by series: {}", e)))?;
    let formats = ReadingStatsRepository::by_format(&state.db, user_id, window)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to group by format: {}", e)))?;

    Ok(Json(ReadingStatsResponse {
        from,
        to,
        granularity,
        summary: summary.into(),
        periods: periods.into_iter().map(Into::into).collect(),
        devices: devices.into_iter().map(Into::into).collect(),
        series: series.into_iter().map(Into::into).collect(),
        formats: formats.into_iter().map(Into::into).collect(),
    }))
}
