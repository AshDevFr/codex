//! Handlers for access group operations
//!
//! Access groups bundle a set of sharing-tag allow/deny rules that can be
//! assigned to multiple users. All endpoints require admin permissions.

use super::super::dto::{
    AccessGroupDetailDto, AccessGroupDto, AccessGroupGrantDto, AccessGroupMemberDto,
    AccessGroupOidcMappingDto, AccessGroupSummaryDto, AddAccessGroupGrantRequest,
    AddAccessGroupMembersRequest, AddAccessGroupOidcMappingRequest, CreateAccessGroupRequest,
    EffectiveGrantDto, EffectiveGrantsResponse, GrantSourceDto, UpdateAccessGroupRequest,
};
use crate::{
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use codex_db::entities::user_access_groups::MembershipSource;
use codex_db::repositories::{AccessGroupRepository, SharingTagRepository};
use std::sync::Arc;
use uuid::Uuid;

// ==================== Access Group CRUD ====================

/// List all access groups (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/access-groups",
    responses(
        (status = 200, description = "List of access groups", body = Vec<AccessGroupDto>),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Access Groups"
)]
pub async fn list_access_groups(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<Vec<AccessGroupDto>>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let groups = AccessGroupRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch access groups: {}", e)))?;

    let mut items = Vec::with_capacity(groups.len());
    for group in groups {
        let member_count = AccessGroupRepository::list_members(&state.db, group.id)
            .await
            .map(|m| m.len() as u64)
            .unwrap_or(0);
        let grant_count = AccessGroupRepository::list_grants(&state.db, group.id)
            .await
            .map(|g| g.len() as u64)
            .unwrap_or(0);
        items.push(AccessGroupDto::from_model_with_counts(
            group,
            member_count,
            grant_count,
        ));
    }

    Ok(Json(items))
}

/// Get an access group by ID with full details (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/access-groups/{group_id}",
    params(
        ("group_id" = Uuid, Path, description = "Access group ID")
    ),
    responses(
        (status = 200, description = "Access group details", body = AccessGroupDetailDto),
        (status = 404, description = "Access group not found"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Access Groups"
)]
pub async fn get_access_group(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(group_id): Path<Uuid>,
) -> Result<Json<AccessGroupDetailDto>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let group = AccessGroupRepository::get_by_id(&state.db, group_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch access group: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Access group not found".to_string()))?;

    let detail = build_group_detail(&state.db, group).await?;
    Ok(Json(detail))
}

/// Create a new access group (admin only)
#[utoipa::path(
    post,
    path = "/api/v1/access-groups",
    request_body = CreateAccessGroupRequest,
    responses(
        (status = 201, description = "Access group created", body = AccessGroupDto),
        (status = 400, description = "Invalid request or group name already exists"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Access Groups"
)]
pub async fn create_access_group(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<CreateAccessGroupRequest>,
) -> Result<(StatusCode, Json<AccessGroupDto>), ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    if request.name.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "Access group name cannot be empty".to_string(),
        ));
    }

    if AccessGroupRepository::get_by_name(&state.db, &request.name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check group name: {}", e)))?
        .is_some()
    {
        return Err(ApiError::BadRequest(format!(
            "Access group with name '{}' already exists",
            request.name
        )));
    }

    let group = AccessGroupRepository::create(&state.db, &request.name, request.description)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create access group: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(AccessGroupDto::from_model_with_counts(group, 0, 0)),
    ))
}

/// Update an access group (admin only)
#[utoipa::path(
    patch,
    path = "/api/v1/access-groups/{group_id}",
    params(
        ("group_id" = Uuid, Path, description = "Access group ID")
    ),
    request_body = UpdateAccessGroupRequest,
    responses(
        (status = 200, description = "Access group updated", body = AccessGroupDto),
        (status = 404, description = "Access group not found"),
        (status = 400, description = "Invalid request or group name already exists"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Access Groups"
)]
pub async fn update_access_group(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(group_id): Path<Uuid>,
    Json(request): Json<UpdateAccessGroupRequest>,
) -> Result<Json<AccessGroupDto>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    if let Some(ref new_name) = request.name {
        if new_name.trim().is_empty() {
            return Err(ApiError::BadRequest(
                "Access group name cannot be empty".to_string(),
            ));
        }
        if let Some(existing) = AccessGroupRepository::get_by_name(&state.db, new_name)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to check group name: {}", e)))?
            && existing.id != group_id
        {
            return Err(ApiError::BadRequest(format!(
                "Access group with name '{}' already exists",
                new_name
            )));
        }
    }

    let group =
        AccessGroupRepository::update(&state.db, group_id, request.name, request.description)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update access group: {}", e)))?
            .ok_or_else(|| ApiError::NotFound("Access group not found".to_string()))?;

    let member_count = AccessGroupRepository::list_members(&state.db, group.id)
        .await
        .map(|m| m.len() as u64)
        .unwrap_or(0);
    let grant_count = AccessGroupRepository::list_grants(&state.db, group.id)
        .await
        .map(|g| g.len() as u64)
        .unwrap_or(0);

    Ok(Json(AccessGroupDto::from_model_with_counts(
        group,
        member_count,
        grant_count,
    )))
}

/// Delete an access group (admin only)
#[utoipa::path(
    delete,
    path = "/api/v1/access-groups/{group_id}",
    params(
        ("group_id" = Uuid, Path, description = "Access group ID")
    ),
    responses(
        (status = 204, description = "Access group deleted"),
        (status = 404, description = "Access group not found"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Access Groups"
)]
pub async fn delete_access_group(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(group_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let deleted = AccessGroupRepository::delete(&state.db, group_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete access group: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("Access group not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ==================== Members ====================

/// Add users to an access group (admin only)
#[utoipa::path(
    post,
    path = "/api/v1/access-groups/{group_id}/members",
    params(
        ("group_id" = Uuid, Path, description = "Access group ID")
    ),
    request_body = AddAccessGroupMembersRequest,
    responses(
        (status = 200, description = "Members added", body = Vec<AccessGroupMemberDto>),
        (status = 404, description = "Access group not found"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Access Groups"
)]
pub async fn add_access_group_members(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(group_id): Path<Uuid>,
    Json(request): Json<AddAccessGroupMembersRequest>,
) -> Result<Json<Vec<AccessGroupMemberDto>>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    // Verify group exists
    AccessGroupRepository::get_by_id(&state.db, group_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch access group: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Access group not found".to_string()))?;

    for user_id in &request.user_ids {
        AccessGroupRepository::add_member(&state.db, group_id, *user_id, MembershipSource::Manual)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to add member: {}", e)))?;
    }

    // Return current members
    let members = build_member_list(&state.db, group_id).await?;
    Ok(Json(members))
}

/// Remove a user from an access group (admin only)
#[utoipa::path(
    delete,
    path = "/api/v1/access-groups/{group_id}/members/{user_id}",
    params(
        ("group_id" = Uuid, Path, description = "Access group ID"),
        ("user_id" = Uuid, Path, description = "User ID to remove")
    ),
    responses(
        (status = 204, description = "Member removed"),
        (status = 404, description = "Member not found in group"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Access Groups"
)]
pub async fn remove_access_group_member(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((group_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let removed = AccessGroupRepository::remove_member(&state.db, group_id, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to remove member: {}", e)))?;

    if !removed {
        return Err(ApiError::NotFound(
            "User is not a member of this access group".to_string(),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ==================== User's Access Groups ====================

/// List a user's access groups (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/users/{user_id}/access-groups",
    params(
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "List of access groups for the user", body = Vec<AccessGroupSummaryDto>),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Access Groups"
)]
pub async fn get_user_access_groups(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<AccessGroupSummaryDto>>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let groups = AccessGroupRepository::list_for_user(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch user access groups: {}", e)))?;

    let dtos: Vec<AccessGroupSummaryDto> = groups.into_iter().map(Into::into).collect();
    Ok(Json(dtos))
}

// ==================== Grants ====================

/// Add a tag grant to an access group (admin only)
#[utoipa::path(
    post,
    path = "/api/v1/access-groups/{group_id}/grants",
    params(
        ("group_id" = Uuid, Path, description = "Access group ID")
    ),
    request_body = AddAccessGroupGrantRequest,
    responses(
        (status = 200, description = "Grant added/updated", body = AccessGroupGrantDto),
        (status = 404, description = "Access group or sharing tag not found"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Access Groups"
)]
pub async fn add_access_group_grant(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(group_id): Path<Uuid>,
    Json(request): Json<AddAccessGroupGrantRequest>,
) -> Result<Json<AccessGroupGrantDto>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    // Verify group exists
    AccessGroupRepository::get_by_id(&state.db, group_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch access group: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Access group not found".to_string()))?;

    // Verify sharing tag exists
    let tag = SharingTagRepository::get_by_id(&state.db, request.sharing_tag_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch sharing tag: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Sharing tag not found".to_string()))?;

    let grant = AccessGroupRepository::set_grant(
        &state.db,
        group_id,
        request.sharing_tag_id,
        request.access_mode,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to set grant: {}", e)))?;

    Ok(Json(AccessGroupGrantDto {
        sharing_tag_id: grant.sharing_tag_id,
        sharing_tag_name: tag.name,
        access_mode: grant.get_access_mode(),
        created_at: grant.created_at,
    }))
}

/// Remove a tag grant from an access group (admin only)
#[utoipa::path(
    delete,
    path = "/api/v1/access-groups/{group_id}/grants/{tag_id}",
    params(
        ("group_id" = Uuid, Path, description = "Access group ID"),
        ("tag_id" = Uuid, Path, description = "Sharing tag ID")
    ),
    responses(
        (status = 204, description = "Grant removed"),
        (status = 404, description = "Grant not found"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Access Groups"
)]
pub async fn remove_access_group_grant(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((group_id, tag_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let removed = AccessGroupRepository::remove_grant(&state.db, group_id, tag_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to remove grant: {}", e)))?;

    if !removed {
        return Err(ApiError::NotFound(
            "Access group does not have a grant for this sharing tag".to_string(),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ==================== OIDC Mappings ====================

/// Add an OIDC mapping to an access group (admin only)
#[utoipa::path(
    post,
    path = "/api/v1/access-groups/{group_id}/oidc-mappings",
    params(
        ("group_id" = Uuid, Path, description = "Access group ID")
    ),
    request_body = AddAccessGroupOidcMappingRequest,
    responses(
        (status = 200, description = "OIDC mapping added", body = AccessGroupOidcMappingDto),
        (status = 404, description = "Access group not found"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Access Groups"
)]
pub async fn add_access_group_oidc_mapping(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(group_id): Path<Uuid>,
    Json(request): Json<AddAccessGroupOidcMappingRequest>,
) -> Result<Json<AccessGroupOidcMappingDto>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    // Verify group exists
    AccessGroupRepository::get_by_id(&state.db, group_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch access group: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Access group not found".to_string()))?;

    if request.oidc_group_name.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "OIDC group name cannot be empty".to_string(),
        ));
    }

    let mapping =
        AccessGroupRepository::add_oidc_mapping(&state.db, group_id, &request.oidc_group_name)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to add OIDC mapping: {}", e)))?;

    Ok(Json(AccessGroupOidcMappingDto {
        id: mapping.id,
        oidc_group_name: mapping.oidc_group_name,
        created_at: mapping.created_at,
    }))
}

/// Remove an OIDC mapping from an access group (admin only)
#[utoipa::path(
    delete,
    path = "/api/v1/access-groups/{group_id}/oidc-mappings/{mapping_id}",
    params(
        ("group_id" = Uuid, Path, description = "Access group ID"),
        ("mapping_id" = Uuid, Path, description = "OIDC mapping ID")
    ),
    responses(
        (status = 204, description = "OIDC mapping removed"),
        (status = 404, description = "OIDC mapping not found"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Access Groups"
)]
pub async fn remove_access_group_oidc_mapping(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((_group_id, mapping_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    let removed = AccessGroupRepository::remove_oidc_mapping(&state.db, mapping_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to remove OIDC mapping: {}", e)))?;

    if !removed {
        return Err(ApiError::NotFound("OIDC mapping not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ==================== Effective Grants Debug ====================

/// Get effective grants for a user with source attribution (admin only)
///
/// Returns each (tag, access_mode) with the sources that contribute it
/// (user override or group name). Useful for debugging "why can/can't this
/// user see content X".
#[utoipa::path(
    get,
    path = "/api/v1/users/{user_id}/effective-grants",
    params(
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "Effective grants with source attribution", body = EffectiveGrantsResponse),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Access Groups"
)]
pub async fn get_user_effective_grants(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(user_id): Path<Uuid>,
) -> Result<Json<EffectiveGrantsResponse>, ApiError> {
    auth.require_permission(&Permission::SystemAdmin)?;

    // Collect per-user grants
    let user_grants = SharingTagRepository::get_grants_with_tags_for_user(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch user grants: {}", e)))?;

    // Collect group grants
    let groups = AccessGroupRepository::list_for_user(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch user groups: {}", e)))?;

    // Build a map: (tag_id, access_mode) -> (tag_name, Vec<source>)
    use std::collections::HashMap;

    // Key: (tag_id, access_mode_str)
    let mut grant_map: HashMap<(Uuid, String), (String, Vec<GrantSourceDto>)> = HashMap::new();

    // Add user grants
    for (grant, tag) in &user_grants {
        let mode = grant.get_access_mode();
        let key = (grant.sharing_tag_id, mode.as_str().to_string());
        let entry = grant_map
            .entry(key)
            .or_insert_with(|| (tag.name.clone(), Vec::new()));
        entry.1.push(GrantSourceDto {
            kind: "user".to_string(),
            group_id: None,
            group_name: None,
        });
    }

    // Add group grants
    for group in &groups {
        let group_grants = AccessGroupRepository::list_grants(&state.db, group.id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch group grants: {}", e)))?;

        for grant in group_grants {
            let mode = grant.get_access_mode();
            let key = (grant.sharing_tag_id, mode.as_str().to_string());

            // We need the tag name; look it up if not already in the map
            let tag_name = if let Some((name, _)) = grant_map.get(&key) {
                name.clone()
            } else {
                SharingTagRepository::get_by_id(&state.db, grant.sharing_tag_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|t| t.name)
                    .unwrap_or_else(|| format!("unknown ({})", grant.sharing_tag_id))
            };

            let entry = grant_map
                .entry(key)
                .or_insert_with(|| (tag_name, Vec::new()));
            entry.1.push(GrantSourceDto {
                kind: "group".to_string(),
                group_id: Some(group.id),
                group_name: Some(group.name.clone()),
            });
        }
    }

    // Convert map to sorted vec
    let mut grants: Vec<EffectiveGrantDto> = grant_map
        .into_iter()
        .map(|((tag_id, mode_str), (tag_name, sources))| {
            let access_mode = mode_str
                .parse()
                .unwrap_or(codex_db::entities::user_sharing_tags::AccessMode::Allow);
            EffectiveGrantDto {
                sharing_tag_id: tag_id,
                sharing_tag_name: tag_name,
                access_mode,
                sources,
            }
        })
        .collect();

    // Sort by tag name, then mode for deterministic output
    grants.sort_by(|a, b| {
        a.sharing_tag_name
            .cmp(&b.sharing_tag_name)
            .then(a.access_mode.as_str().cmp(b.access_mode.as_str()))
    });

    Ok(Json(EffectiveGrantsResponse { user_id, grants }))
}

// ==================== Helpers ====================

async fn build_group_detail(
    db: &sea_orm::DatabaseConnection,
    group: codex_db::entities::access_groups::Model,
) -> Result<AccessGroupDetailDto, ApiError> {
    let members = build_member_list(db, group.id).await?;

    let grant_rows = AccessGroupRepository::list_grants(db, group.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch grants: {}", e)))?;

    let mut grants = Vec::with_capacity(grant_rows.len());
    for grant in grant_rows {
        let tag_name = SharingTagRepository::get_by_id(db, grant.sharing_tag_id)
            .await
            .ok()
            .flatten()
            .map(|t| t.name)
            .unwrap_or_else(|| format!("unknown ({})", grant.sharing_tag_id));
        grants.push(AccessGroupGrantDto {
            sharing_tag_id: grant.sharing_tag_id,
            sharing_tag_name: tag_name,
            access_mode: grant.get_access_mode(),
            created_at: grant.created_at,
        });
    }

    let oidc_rows = AccessGroupRepository::list_oidc_mappings(db, group.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch OIDC mappings: {}", e)))?;

    let oidc_mappings = oidc_rows
        .into_iter()
        .map(|m| AccessGroupOidcMappingDto {
            id: m.id,
            oidc_group_name: m.oidc_group_name,
            created_at: m.created_at,
        })
        .collect();

    Ok(AccessGroupDetailDto {
        id: group.id,
        name: group.name,
        description: group.description,
        members,
        grants,
        oidc_mappings,
        created_at: group.created_at,
        updated_at: group.updated_at,
    })
}

async fn build_member_list(
    db: &sea_orm::DatabaseConnection,
    group_id: Uuid,
) -> Result<Vec<AccessGroupMemberDto>, ApiError> {
    let membership_rows = AccessGroupRepository::list_members(db, group_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch members: {}", e)))?;

    let user_rows = AccessGroupRepository::list_member_users(db, group_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch member users: {}", e)))?;

    // Build a username lookup
    let user_map: std::collections::HashMap<Uuid, &str> = user_rows
        .iter()
        .map(|u| (u.id, u.username.as_str()))
        .collect();

    Ok(membership_rows
        .into_iter()
        .map(|m| AccessGroupMemberDto {
            user_id: m.user_id,
            username: user_map.get(&m.user_id).unwrap_or(&"unknown").to_string(),
            source: m.source.clone(),
            created_at: m.created_at,
        })
        .collect())
}
