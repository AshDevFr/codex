//! Handlers for sharing tag operations
//!
//! Sharing tags control content access. Admins can create tags, assign them to series,
//! and grant users access to content via these tags.

use super::super::dto::{
    CreateSharingTagRequest, ModifySeriesSharingTagRequest, SetSeriesSharingTagsRequest,
    SetUserSharingTagGrantRequest, SharingTagDto, SharingTagListResponse, SharingTagSummaryDto,
    UpdateSharingTagRequest, UserSharingTagGrantDto, UserSharingTagGrantsResponse,
};
use crate::api::{
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::repositories::SharingTagRepository;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

// ==================== Sharing Tag CRUD ====================

/// List all sharing tags (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/admin/sharing-tags",
    responses(
        (status = 200, description = "List of sharing tags", body = SharingTagListResponse),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "sharing-tags"
)]
pub async fn list_sharing_tags(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<SharingTagListResponse>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let tags = SharingTagRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch sharing tags: {}", e)))?;

    let mut items = Vec::with_capacity(tags.len());
    for tag in tags {
        let series_count = SharingTagRepository::count_series_with_tag(&state.db, tag.id)
            .await
            .unwrap_or(0);
        let user_count = SharingTagRepository::count_users_with_tag(&state.db, tag.id)
            .await
            .unwrap_or(0);
        items.push(SharingTagDto::from_model_with_counts(
            tag,
            series_count,
            user_count,
        ));
    }

    let total = items.len();
    Ok(Json(SharingTagListResponse { items, total }))
}

/// Get a sharing tag by ID (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/admin/sharing-tags/{tag_id}",
    params(
        ("tag_id" = Uuid, Path, description = "Sharing tag ID")
    ),
    responses(
        (status = 200, description = "Sharing tag details", body = SharingTagDto),
        (status = 404, description = "Sharing tag not found"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "sharing-tags"
)]
pub async fn get_sharing_tag(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(tag_id): Path<Uuid>,
) -> Result<Json<SharingTagDto>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let tag = SharingTagRepository::get_by_id(&state.db, tag_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch sharing tag: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Sharing tag not found".to_string()))?;

    let series_count = SharingTagRepository::count_series_with_tag(&state.db, tag.id)
        .await
        .unwrap_or(0);
    let user_count = SharingTagRepository::count_users_with_tag(&state.db, tag.id)
        .await
        .unwrap_or(0);

    Ok(Json(SharingTagDto::from_model_with_counts(
        tag,
        series_count,
        user_count,
    )))
}

/// Create a new sharing tag (admin only)
#[utoipa::path(
    post,
    path = "/api/v1/admin/sharing-tags",
    request_body = CreateSharingTagRequest,
    responses(
        (status = 201, description = "Sharing tag created", body = SharingTagDto),
        (status = 400, description = "Invalid request or tag name already exists"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "sharing-tags"
)]
pub async fn create_sharing_tag(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<CreateSharingTagRequest>,
) -> Result<(StatusCode, Json<SharingTagDto>), ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    // Check if tag name already exists
    if SharingTagRepository::get_by_name(&state.db, &request.name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check tag name: {}", e)))?
        .is_some()
    {
        return Err(ApiError::BadRequest(format!(
            "Sharing tag with name '{}' already exists",
            request.name
        )));
    }

    let tag = SharingTagRepository::create(&state.db, &request.name, request.description)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create sharing tag: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(SharingTagDto::from_model_with_counts(tag, 0, 0)),
    ))
}

/// Update a sharing tag (admin only)
#[utoipa::path(
    patch,
    path = "/api/v1/admin/sharing-tags/{tag_id}",
    params(
        ("tag_id" = Uuid, Path, description = "Sharing tag ID")
    ),
    request_body = UpdateSharingTagRequest,
    responses(
        (status = 200, description = "Sharing tag updated", body = SharingTagDto),
        (status = 404, description = "Sharing tag not found"),
        (status = 400, description = "Invalid request or tag name already exists"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "sharing-tags"
)]
pub async fn update_sharing_tag(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(tag_id): Path<Uuid>,
    Json(request): Json<UpdateSharingTagRequest>,
) -> Result<Json<SharingTagDto>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    // If renaming, check that new name doesn't conflict
    if let Some(ref new_name) = request.name {
        if let Some(existing) = SharingTagRepository::get_by_name(&state.db, new_name)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to check tag name: {}", e)))?
        {
            if existing.id != tag_id {
                return Err(ApiError::BadRequest(format!(
                    "Sharing tag with name '{}' already exists",
                    new_name
                )));
            }
        }
    }

    let tag = SharingTagRepository::update(&state.db, tag_id, request.name, request.description)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update sharing tag: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Sharing tag not found".to_string()))?;

    let series_count = SharingTagRepository::count_series_with_tag(&state.db, tag.id)
        .await
        .unwrap_or(0);
    let user_count = SharingTagRepository::count_users_with_tag(&state.db, tag.id)
        .await
        .unwrap_or(0);

    Ok(Json(SharingTagDto::from_model_with_counts(
        tag,
        series_count,
        user_count,
    )))
}

/// Delete a sharing tag (admin only)
#[utoipa::path(
    delete,
    path = "/api/v1/admin/sharing-tags/{tag_id}",
    params(
        ("tag_id" = Uuid, Path, description = "Sharing tag ID")
    ),
    responses(
        (status = 204, description = "Sharing tag deleted"),
        (status = 404, description = "Sharing tag not found"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "sharing-tags"
)]
pub async fn delete_sharing_tag(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(tag_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let deleted = SharingTagRepository::delete(&state.db, tag_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete sharing tag: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("Sharing tag not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ==================== Series Sharing Tags ====================

/// Get sharing tags for a series (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/sharing-tags",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "List of sharing tags for the series", body = Vec<SharingTagSummaryDto>),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "sharing-tags"
)]
pub async fn get_series_sharing_tags(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<Vec<SharingTagSummaryDto>>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let tags = SharingTagRepository::get_tags_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series sharing tags: {}", e)))?;

    let dtos: Vec<SharingTagSummaryDto> =
        tags.into_iter().map(SharingTagSummaryDto::from).collect();

    Ok(Json(dtos))
}

/// Set sharing tags for a series (replaces existing) (admin only)
#[utoipa::path(
    put,
    path = "/api/v1/series/{series_id}/sharing-tags",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = SetSeriesSharingTagsRequest,
    responses(
        (status = 200, description = "Sharing tags set", body = Vec<SharingTagSummaryDto>),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "sharing-tags"
)]
pub async fn set_series_sharing_tags(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<SetSeriesSharingTagsRequest>,
) -> Result<Json<Vec<SharingTagSummaryDto>>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let tags =
        SharingTagRepository::set_tags_for_series(&state.db, series_id, request.sharing_tag_ids)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to set series sharing tags: {}", e)))?;

    let dtos: Vec<SharingTagSummaryDto> =
        tags.into_iter().map(SharingTagSummaryDto::from).collect();

    Ok(Json(dtos))
}

/// Add a sharing tag to a series (admin only)
#[utoipa::path(
    post,
    path = "/api/v1/series/{series_id}/sharing-tags",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = ModifySeriesSharingTagRequest,
    responses(
        (status = 200, description = "Sharing tag added"),
        (status = 400, description = "Tag already assigned"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "sharing-tags"
)]
pub async fn add_series_sharing_tag(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<ModifySeriesSharingTagRequest>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let added =
        SharingTagRepository::add_tag_to_series(&state.db, series_id, request.sharing_tag_id)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to add sharing tag to series: {}", e))
            })?;

    if !added {
        return Err(ApiError::BadRequest(
            "Sharing tag is already assigned to this series".to_string(),
        ));
    }

    Ok(StatusCode::OK)
}

/// Remove a sharing tag from a series (admin only)
#[utoipa::path(
    delete,
    path = "/api/v1/series/{series_id}/sharing-tags/{tag_id}",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        ("tag_id" = Uuid, Path, description = "Sharing tag ID")
    ),
    responses(
        (status = 204, description = "Sharing tag removed"),
        (status = 404, description = "Sharing tag not assigned to series"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "sharing-tags"
)]
pub async fn remove_series_sharing_tag(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((series_id, tag_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let removed = SharingTagRepository::remove_tag_from_series(&state.db, series_id, tag_id)
        .await
        .map_err(|e| {
            ApiError::Internal(format!("Failed to remove sharing tag from series: {}", e))
        })?;

    if !removed {
        return Err(ApiError::NotFound(
            "Sharing tag is not assigned to this series".to_string(),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ==================== User Sharing Tag Grants ====================

/// Get sharing tag grants for a user (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/users/{user_id}/sharing-tags",
    params(
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "List of sharing tag grants for the user", body = UserSharingTagGrantsResponse),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "sharing-tags"
)]
pub async fn get_user_sharing_tags(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(user_id): Path<Uuid>,
) -> Result<Json<UserSharingTagGrantsResponse>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let grants_with_tags = SharingTagRepository::get_grants_with_tags_for_user(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch user sharing tags: {}", e)))?;

    let grants: Vec<UserSharingTagGrantDto> = grants_with_tags
        .into_iter()
        .map(|(grant, tag)| UserSharingTagGrantDto::from_models(grant, tag))
        .collect();

    Ok(Json(UserSharingTagGrantsResponse { user_id, grants }))
}

/// Set a user's sharing tag grant (admin only)
#[utoipa::path(
    put,
    path = "/api/v1/users/{user_id}/sharing-tags",
    params(
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    request_body = SetUserSharingTagGrantRequest,
    responses(
        (status = 200, description = "Sharing tag grant set", body = UserSharingTagGrantDto),
        (status = 404, description = "Sharing tag not found"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "sharing-tags"
)]
pub async fn set_user_sharing_tag(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(user_id): Path<Uuid>,
    Json(request): Json<SetUserSharingTagGrantRequest>,
) -> Result<Json<UserSharingTagGrantDto>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    // Verify the sharing tag exists
    let tag = SharingTagRepository::get_by_id(&state.db, request.sharing_tag_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch sharing tag: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Sharing tag not found".to_string()))?;

    let grant = SharingTagRepository::set_user_grant(
        &state.db,
        user_id,
        request.sharing_tag_id,
        request.access_mode,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to set user sharing tag grant: {}", e)))?;

    Ok(Json(UserSharingTagGrantDto::from_models(grant, tag)))
}

/// Remove a user's sharing tag grant (admin only)
#[utoipa::path(
    delete,
    path = "/api/v1/users/{user_id}/sharing-tags/{tag_id}",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
        ("tag_id" = Uuid, Path, description = "Sharing tag ID")
    ),
    responses(
        (status = 204, description = "Sharing tag grant removed"),
        (status = 404, description = "Grant not found"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "sharing-tags"
)]
pub async fn remove_user_sharing_tag(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((user_id, tag_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let removed = SharingTagRepository::remove_user_grant(&state.db, user_id, tag_id)
        .await
        .map_err(|e| {
            ApiError::Internal(format!("Failed to remove user sharing tag grant: {}", e))
        })?;

    if !removed {
        return Err(ApiError::NotFound(
            "User does not have a grant for this sharing tag".to_string(),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ==================== Current User's Sharing Tags ====================

/// Get current user's sharing tag grants
#[utoipa::path(
    get,
    path = "/api/v1/user/sharing-tags",
    responses(
        (status = 200, description = "List of sharing tag grants for the current user", body = UserSharingTagGrantsResponse),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "sharing-tags"
)]
pub async fn get_my_sharing_tags(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<UserSharingTagGrantsResponse>, ApiError> {
    let grants_with_tags =
        SharingTagRepository::get_grants_with_tags_for_user(&state.db, auth.user_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch user sharing tags: {}", e)))?;

    let grants: Vec<UserSharingTagGrantDto> = grants_with_tags
        .into_iter()
        .map(|(grant, tag)| UserSharingTagGrantDto::from_models(grant, tag))
        .collect();

    Ok(Json(UserSharingTagGrantsResponse {
        user_id: auth.user_id,
        grants,
    }))
}
