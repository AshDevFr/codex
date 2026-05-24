//! Handlers for the unified filter-preset endpoints.
//!
//! Powers both the library list-page saved-filter dropdowns (`scope = "list"`)
//! and the advanced search page (`scope = "search"`).

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use sea_orm::DbErr;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::api::extractors::auth::{AppState, AuthContext};
use codex_db::repositories::{
    FilterPresetRepository, ListFilterPresetsQuery as RepoListQuery, UpdateFilterPreset,
};

use super::super::dto::filter_preset::{
    CreateFilterPresetRequest, FilterPresetDto, FilterPresetListResponse, ListFilterPresetsQuery,
    UpdateFilterPresetRequest, validate_condition, validate_scope, validate_target,
};

const NAME_MAX_LEN: usize = 100;

fn validate_name(name: &str) -> Result<(), ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ApiError::BadRequest("name cannot be empty".to_string()));
    }
    if trimmed.chars().count() > NAME_MAX_LEN {
        return Err(ApiError::BadRequest(format!(
            "name cannot exceed {NAME_MAX_LEN} characters"
        )));
    }
    Ok(())
}

/// Map an error from a repository call into the right API error. A unique
/// constraint violation surfaces as a 409 so the client can prompt the user
/// to pick a different name.
fn map_create_error(err: anyhow::Error) -> ApiError {
    if let Some(db_err) = err.downcast_ref::<DbErr>() {
        let msg = db_err.to_string().to_lowercase();
        if msg.contains("unique") || msg.contains("constraint") || msg.contains("duplicate") {
            return ApiError::Conflict(
                "A preset with this name already exists for the same scope/target/library"
                    .to_string(),
            );
        }
    }
    ApiError::Internal(format!("Failed to save preset: {err}"))
}

/// POST /api/v1/filter-presets - Create a new preset.
#[utoipa::path(
    post,
    path = "/api/v1/filter-presets",
    request_body = CreateFilterPresetRequest,
    responses(
        (status = 201, description = "Preset created", body = FilterPresetDto),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "Duplicate preset name"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Filter Presets"
)]
pub async fn create_filter_preset(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<CreateFilterPresetRequest>,
) -> Result<(StatusCode, Json<FilterPresetDto>), ApiError> {
    validate_name(&request.name)?;
    validate_scope(&request.scope).map_err(ApiError::BadRequest)?;
    validate_target(&request.target).map_err(ApiError::BadRequest)?;
    validate_condition(&request.target, &request.condition).map_err(ApiError::BadRequest)?;

    let preset = FilterPresetRepository::create(
        &state.db,
        auth.user_id,
        &request.scope,
        &request.target,
        request.name.trim(),
        request.condition,
        request.query,
        request.sort,
        request.library_id,
    )
    .await
    .map_err(map_create_error)?;

    Ok((
        StatusCode::CREATED,
        Json(FilterPresetDto::from_model(&preset)),
    ))
}

/// GET /api/v1/filter-presets - List the caller's presets.
#[utoipa::path(
    get,
    path = "/api/v1/filter-presets",
    params(
        ("scope" = Option<String>, Query, description = "Filter by scope ('list' or 'search')"),
        ("target" = Option<String>, Query, description = "Filter by target ('series' or 'books')"),
        ("libraryId" = Option<Uuid>, Query, description = "Filter by library id"),
    ),
    responses(
        (status = 200, description = "List of presets", body = FilterPresetListResponse),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Filter Presets"
)]
pub async fn list_filter_presets(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Query(query): Query<ListFilterPresetsQuery>,
) -> Result<Json<FilterPresetListResponse>, ApiError> {
    if let Some(scope) = query.scope.as_deref() {
        validate_scope(scope).map_err(ApiError::BadRequest)?;
    }
    if let Some(target) = query.target.as_deref() {
        validate_target(target).map_err(ApiError::BadRequest)?;
    }

    let presets = FilterPresetRepository::list_for_user(
        &state.db,
        auth.user_id,
        RepoListQuery {
            scope: query.scope.as_deref(),
            target: query.target.as_deref(),
            library_id: query.library_id,
        },
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to list presets: {e}")))?;

    let dtos = presets.iter().map(FilterPresetDto::from_model).collect();
    Ok(Json(FilterPresetListResponse { presets: dtos }))
}

/// GET /api/v1/filter-presets/{id} - Fetch a single preset.
#[utoipa::path(
    get,
    path = "/api/v1/filter-presets/{id}",
    params(("id" = Uuid, Path, description = "Preset id")),
    responses(
        (status = 200, description = "Preset detail", body = FilterPresetDto),
        (status = 404, description = "Preset not found"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Filter Presets"
)]
pub async fn get_filter_preset(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<FilterPresetDto>, ApiError> {
    let preset = FilterPresetRepository::find_by_id_and_user(&state.db, id, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load preset: {e}")))?
        .ok_or_else(|| ApiError::NotFound("Preset not found".to_string()))?;
    Ok(Json(FilterPresetDto::from_model(&preset)))
}

/// PUT /api/v1/filter-presets/{id} - Update a preset.
#[utoipa::path(
    put,
    path = "/api/v1/filter-presets/{id}",
    params(("id" = Uuid, Path, description = "Preset id")),
    request_body = UpdateFilterPresetRequest,
    responses(
        (status = 200, description = "Preset updated", body = FilterPresetDto),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Preset not found"),
        (status = 409, description = "Duplicate preset name"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Filter Presets"
)]
pub async fn update_filter_preset(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateFilterPresetRequest>,
) -> Result<Json<FilterPresetDto>, ApiError> {
    validate_name(&request.name)?;

    // We need the existing row's `target` to validate the condition shape.
    let existing = FilterPresetRepository::find_by_id_and_user(&state.db, id, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load preset: {e}")))?
        .ok_or_else(|| ApiError::NotFound("Preset not found".to_string()))?;

    validate_condition(&existing.target, &request.condition).map_err(ApiError::BadRequest)?;

    let updated = FilterPresetRepository::update(
        &state.db,
        id,
        auth.user_id,
        UpdateFilterPreset {
            name: Some(request.name.trim().to_string()),
            condition: Some(request.condition),
            query: Some(request.query),
            sort: Some(request.sort),
            library_id: Some(request.library_id),
        },
    )
    .await
    .map_err(map_create_error)?
    .ok_or_else(|| ApiError::NotFound("Preset not found".to_string()))?;

    Ok(Json(FilterPresetDto::from_model(&updated)))
}

/// DELETE /api/v1/filter-presets/{id} - Delete a preset.
#[utoipa::path(
    delete,
    path = "/api/v1/filter-presets/{id}",
    params(("id" = Uuid, Path, description = "Preset id")),
    responses(
        (status = 204, description = "Preset deleted"),
        (status = 404, description = "Preset not found"),
    ),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Filter Presets"
)]
pub async fn delete_filter_preset(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = FilterPresetRepository::delete_by_id_for_user(&state.db, id, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete preset: {e}")))?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("Preset not found".to_string()))
    }
}
