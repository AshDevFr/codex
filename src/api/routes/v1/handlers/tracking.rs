//! HTTP handlers for release-tracking config + title aliases.
//!
//! Endpoints (all under `/api/v1/series/{series_id}`):
//! - `GET /tracking` — read (returns a virtual untracked row when none exists)
//! - `PATCH /tracking` — update (upserts on first write)
//! - `GET /aliases` — list aliases for the series
//! - `POST /aliases` — add a manual alias (idempotent on duplicate)
//! - `DELETE /aliases/{alias_id}` — remove an alias

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

use super::super::dto::tracking::{
    CreateSeriesAliasRequest, SeriesAliasDto, SeriesAliasListResponse, SeriesTrackingDto,
    UpdateSeriesTrackingRequest,
};
use crate::api::{
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::entities::series_aliases::alias_source;
use crate::db::repositories::{
    SeriesAliasRepository, SeriesRepository, SeriesTrackingRepository, TrackingUpdate,
};
use crate::events::{EntityChangeEvent, EntityEvent};
use crate::require_permission;

// =============================================================================
// Tracking config handlers
// =============================================================================

/// Get release-tracking config for a series.
///
/// Returns a virtual untracked row when no `series_tracking` row exists, so the
/// frontend can render the panel uniformly without special-casing absent rows.
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/tracking",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "Tracking config", body = SeriesTrackingDto),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Tracking"
)]
pub async fn get_series_tracking(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<SeriesTrackingDto>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let row = SeriesTrackingRepository::get_or_default(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch tracking: {}", e)))?;
    Ok(Json(row.into()))
}

/// Update release-tracking config for a series.
///
/// Upserts: creates the row on first write, applies the patch otherwise.
/// All fields are optional — omit to leave alone, send `null` on a nullable
/// field to clear it.
#[utoipa::path(
    patch,
    path = "/api/v1/series/{series_id}/tracking",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = UpdateSeriesTrackingRequest,
    responses(
        (status = 200, description = "Tracking config updated", body = SeriesTrackingDto),
        (status = 400, description = "Invalid tracking_status"),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Tracking"
)]
pub async fn update_series_tracking(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<UpdateSeriesTrackingRequest>,
) -> Result<Json<SeriesTrackingDto>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let update = TrackingUpdate {
        tracked: request.tracked,
        tracking_status: request.tracking_status,
        track_chapters: request.track_chapters,
        track_volumes: request.track_volumes,
        latest_known_chapter: request.latest_known_chapter,
        latest_known_volume: request.latest_known_volume,
        volume_chapter_map: request.volume_chapter_map,
        poll_interval_override_s: request.poll_interval_override_s,
        confidence_threshold_override: request.confidence_threshold_override,
        languages: request
            .languages
            .map(|opt| opt.map(|langs| serde_json::json!(langs))),
    };

    let row = SeriesTrackingRepository::upsert(&state.db, series_id, update)
        .await
        .map_err(|e| {
            // Surface validation errors (e.g., invalid tracking_status) as 400.
            if e.to_string().contains("invalid tracking_status") {
                ApiError::BadRequest(e.to_string())
            } else {
                ApiError::Internal(format!("Failed to update tracking: {}", e))
            }
        })?;

    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["tracking".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(row.into()))
}

// =============================================================================
// Alias handlers
// =============================================================================

/// List release-matching aliases for a series.
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/aliases",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "List of aliases", body = SeriesAliasListResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Tracking"
)]
pub async fn list_series_aliases(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<SeriesAliasListResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let aliases = SeriesAliasRepository::get_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch aliases: {}", e)))?;

    Ok(Json(SeriesAliasListResponse {
        aliases: aliases.into_iter().map(Into::into).collect(),
    }))
}

/// Create a release-matching alias for a series.
///
/// Idempotent: if `(series_id, alias)` already exists, returns the existing
/// row with HTTP 200 instead of inserting a duplicate.
#[utoipa::path(
    post,
    path = "/api/v1/series/{series_id}/aliases",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = CreateSeriesAliasRequest,
    responses(
        (status = 201, description = "Alias created", body = SeriesAliasDto),
        (status = 200, description = "Alias already existed (idempotent)", body = SeriesAliasDto),
        (status = 400, description = "Invalid alias (empty after normalization)"),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Tracking"
)]
pub async fn create_series_alias(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<CreateSeriesAliasRequest>,
) -> Result<(StatusCode, Json<SeriesAliasDto>), ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Determine source. HTTP defaults to `manual`; we accept `metadata` only
    // for explicit admin imports (e.g., a follow-up tool that wants to seed
    // metadata-source aliases through the API rather than the backfill task).
    let source = request
        .source
        .as_deref()
        .filter(|s| alias_source::is_valid(s))
        .unwrap_or(alias_source::MANUAL);

    // Detect insert-vs-existing by counting before/after — `create()` returns
    // the existing row on duplicate, but doesn't tell us which case we hit.
    let before = SeriesAliasRepository::count_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to count aliases: {}", e)))?;
    let alias = SeriesAliasRepository::create(&state.db, series_id, &request.alias, source)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("empty")
                || msg.contains("normalize")
                || msg.contains("invalid alias source")
            {
                ApiError::BadRequest(msg)
            } else {
                ApiError::Internal(format!("Failed to create alias: {}", e))
            }
        })?;
    let after = SeriesAliasRepository::count_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to count aliases: {}", e)))?;

    let status = if after > before {
        // Newly inserted: emit update event so the frontend invalidates its cache.
        let event = EntityChangeEvent {
            event: EntityEvent::SeriesUpdated {
                series_id,
                library_id: series.library_id,
                fields: Some(vec!["aliases".to_string()]),
            },
            timestamp: Utc::now(),
            user_id: Some(auth.user_id),
        };
        let _ = state.event_broadcaster.emit(event);
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };

    Ok((status, Json(alias.into())))
}

/// Delete a release-matching alias.
#[utoipa::path(
    delete,
    path = "/api/v1/series/{series_id}/aliases/{alias_id}",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        ("alias_id" = Uuid, Path, description = "Alias ID")
    ),
    responses(
        (status = 204, description = "Alias deleted"),
        (status = 404, description = "Series or alias not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Tracking"
)]
pub async fn delete_series_alias(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((series_id, alias_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Verify the alias actually belongs to this series before deleting.
    let row = SeriesAliasRepository::get_by_id(&state.db, alias_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch alias: {}", e)))?;
    let row = match row {
        Some(r) if r.series_id == series_id => r,
        _ => return Err(ApiError::NotFound("Alias not found".to_string())),
    };

    SeriesAliasRepository::delete(&state.db, row.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete alias: {}", e)))?;

    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["aliases".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::NO_CONTENT)
}
