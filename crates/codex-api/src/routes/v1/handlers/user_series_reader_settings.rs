//! Handlers for a user's per-series reader settings.
//!
//! These are deliberately not gated on `series:write`. Reading direction and
//! the layout settings beside it describe how a book is made, and the stored
//! value is routinely absent or wrong, so every reader needs a way to render a
//! series correctly for themselves. Changing what *every* user sees is a
//! separate, permissioned act against the series metadata.

use super::super::dto::{PatchSeriesReaderSettingsRequest, SeriesReaderSettingsResponse};
use crate::{AppState, error::ApiError, extractors::AuthContext};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use codex_db::repositories::{SeriesRepository, UserSeriesReaderSettingsRepository};
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
    )),
    tags(
        (name = "User Reader Settings", description = "Per-user, per-series reader overrides")
    )
)]
#[allow(dead_code)] // OpenAPI documentation struct - referenced by utoipa derive macros
pub struct UserSeriesReaderSettingsApi;

/// Confirm the series exists, so a typo returns 404 rather than silently
/// storing settings against an id that means nothing.
async fn ensure_series_exists(state: &Arc<AppState>, series_id: Uuid) -> Result<(), ApiError> {
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;
    Ok(())
}

/// Get the authenticated user's reader overrides for a series
///
/// The response is sparse: a field is absent when the user has not overridden
/// it, and the reader inherits from the series metadata or the library default
/// instead. A user with no overrides gets an empty object, not a 404.
///
/// Reading direction also arrives already resolved on the book responses, so a
/// client rendering a book does not need this endpoint for that field. It is
/// here for the settings UI, which has to show which values are overridden and
/// which are inherited.
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
    ensure_series_exists(&state, series_id).await?;

    let settings =
        UserSeriesReaderSettingsRepository::get_for_user_series(&state.db, auth.user_id, series_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .unwrap_or_default();

    Ok(Json(settings.into()))
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
    ensure_series_exists(&state, series_id).await?;

    let current =
        UserSeriesReaderSettingsRepository::get_for_user_series(&state.db, auth.user_id, series_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
            .unwrap_or_default();

    let merged = request.apply_to(current);

    // An emptied record is stored as no record: the repository drops the row.
    UserSeriesReaderSettingsRepository::upsert(&state.db, auth.user_id, series_id, merged)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    Ok(Json(merged.into()))
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
    ensure_series_exists(&state, series_id).await?;

    UserSeriesReaderSettingsRepository::delete(&state.db, auth.user_id, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}
