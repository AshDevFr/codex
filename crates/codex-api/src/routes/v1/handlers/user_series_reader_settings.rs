//! Handlers for a user's per-series reader settings.
//!
//! These are deliberately not gated on `series:write`. Reading direction and
//! the layout settings beside it describe how a book is made, and the stored
//! value is routinely absent or wrong, so every reader needs a way to render a
//! series correctly for themselves. Changing what *every* user sees is a
//! separate, permissioned act against the series metadata.

use super::super::dto::{
    InheritedFrom, PatchSeriesReaderSettingsRequest, SeriesReaderSettingsResponse,
};
use crate::{AppState, error::ApiError, extractors::AuthContext};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use codex_db::entities::series;
use codex_db::repositories::{
    LibraryRepository, SeriesMetadataRepository, SeriesRepository,
    UserSeriesReaderSettingsRepository,
};
use codex_models::reading_direction::ReadingDirection;
use std::sync::Arc;
use utoipa::OpenApi;
use uuid::Uuid;

#[derive(OpenApi)]
#[openapi(
    paths(
        get_series_reader_settings,
        patch_series_reader_settings,
        delete_series_reader_settings,
    ),
    components(schemas(
        SeriesReaderSettingsResponse,
        PatchSeriesReaderSettingsRequest,
        InheritedFrom,
    )),
    tags(
        (name = "User Reader Settings", description = "Per-user, per-series reader overrides")
    )
)]
#[allow(dead_code)] // OpenAPI documentation struct - referenced by utoipa derive macros
pub struct UserSeriesReaderSettingsApi;

/// Confirm the series exists, so a typo returns 404 rather than silently
/// storing settings against an id that means nothing.
///
/// Returns the series because the caller needs its `library_id` to resolve what
/// the user inherits, and re-fetching it would be a second query for a row
/// already in hand.
async fn require_series(state: &Arc<AppState>, series_id: Uuid) -> Result<series::Model, ApiError> {
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))
}

/// The reading direction this user would see with no override of their own.
///
/// The layers below the user, in the same precedence the book responses use:
/// series metadata, then the library default. A layer holding an unparseable
/// legacy value is skipped rather than surfaced, exactly as
/// [`ReadingDirection::resolve`] skips it on the book path.
///
/// This resolves one layer short of that chain on purpose: the user's own
/// override is what a caller is deciding whether to drop, so including it would
/// report the value being replaced as the value replacing it.
///
/// The source travels with the value instead of being derived a second time,
/// so the two cannot disagree about which layer answered.
async fn inherited_reading_direction(
    state: &Arc<AppState>,
    series: &series::Model,
) -> Result<Option<(ReadingDirection, InheritedFrom)>, ApiError> {
    let metadata = SeriesMetadataRepository::get_by_series_id(&state.db, series.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;
    let library = LibraryRepository::get_by_id(&state.db, series.library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let layers = [
        (
            metadata
                .as_ref()
                .and_then(|m| m.reading_direction.as_deref()),
            InheritedFrom::Series,
        ),
        (
            library
                .as_ref()
                .map(|l| l.default_reading_direction.as_str()),
            InheritedFrom::Library,
        ),
    ];

    Ok(layers
        .into_iter()
        .find_map(|(raw, source)| ReadingDirection::parse_stored(raw).map(|d| (d, source))))
}

/// Get the authenticated user's reader overrides for a series
///
/// The response is sparse: an override field is absent when the user has not
/// set it, and the reader inherits from the series metadata or the library
/// default instead. A user with no overrides gets a 200, not a 404.
///
/// The `inherited*` fields report what the user would get with no override,
/// whether or not one is set. Book responses carry the direction already
/// resolved, so this is the only way a client can see the layer beneath its own
/// override, and the only way it can offer to drop it by name.
#[utoipa::path(
    get,
    path = "/api/v1/user/series/{series_id}/reader-settings",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
    ),
    responses(
        (status = 200, description = "Reader settings retrieved", body = SeriesReaderSettingsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = []),
    ),
    tag = "User Reader Settings"
)]
pub async fn get_series_reader_settings(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<SeriesReaderSettingsResponse>, ApiError> {
    let series = require_series(&state, series_id).await?;

    let settings =
        UserSeriesReaderSettingsRepository::get_for_user_series(&state.db, auth.user_id, series_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .unwrap_or_default();

    let inherited = inherited_reading_direction(&state, &series).await?;

    Ok(Json(SeriesReaderSettingsResponse::new(settings, inherited)))
}

/// Update the authenticated user's reader overrides for a series
///
/// Ordinary PATCH semantics, per field: absent leaves the setting alone, an
/// explicit `null` clears the override so the setting inherits again, and a
/// value overrides it.
///
/// Per-key clearing exists because the record is sparse. Each setting is
/// independently inherited or overridden, so undoing one must not require
/// wiping the rest and re-setting them. When the last override is cleared the
/// stored record is removed entirely, which is the same end state as `DELETE`.
///
/// This writes only what this user sees. Changing the series for everyone is
/// `PATCH /api/v1/series/{series_id}/metadata`, which requires `series:write`.
#[utoipa::path(
    patch,
    path = "/api/v1/user/series/{series_id}/reader-settings",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
    ),
    request_body = PatchSeriesReaderSettingsRequest,
    responses(
        (status = 200, description = "Reader settings updated", body = SeriesReaderSettingsResponse),
        (status = 400, description = "Invalid setting value"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = []),
    ),
    tag = "User Reader Settings"
)]
pub async fn patch_series_reader_settings(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<PatchSeriesReaderSettingsRequest>,
) -> Result<Json<SeriesReaderSettingsResponse>, ApiError> {
    let series = require_series(&state, series_id).await?;

    let current =
        UserSeriesReaderSettingsRepository::get_for_user_series(&state.db, auth.user_id, series_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .unwrap_or_default();

    // Validated here rather than by serde so the failure is a 400 with a
    // message naming the valid values, matching the series metadata endpoints.
    let direction = request
        .validated_direction()
        .map_err(ApiError::BadRequest)?;
    let merged = request.apply_to(current, direction);

    // An emptied record is stored as no record: the repository drops the row.
    UserSeriesReaderSettingsRepository::upsert(&state.db, auth.user_id, series_id, merged)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    // The client writes this response straight into its cache, so it has to
    // carry the inherited layer too: a leaner response here would blank the
    // reset affordance until the next refetch.
    let inherited = inherited_reading_direction(&state, &series).await?;

    Ok(Json(SeriesReaderSettingsResponse::new(merged, inherited)))
}

/// Clear all of the authenticated user's reader overrides for a series
///
/// The series inherits fully again: series metadata, then the library default.
/// Clearing settings that were never set is not an error.
#[utoipa::path(
    delete,
    path = "/api/v1/user/series/{series_id}/reader-settings",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
    ),
    responses(
        (status = 204, description = "Reader settings cleared"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = []),
    ),
    tag = "User Reader Settings"
)]
pub async fn delete_series_reader_settings(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_series(&state, series_id).await?;

    UserSeriesReaderSettingsRepository::delete(&state.db, auth.user_id, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}
