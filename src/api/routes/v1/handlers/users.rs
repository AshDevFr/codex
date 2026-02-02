use super::super::dto::{
    common::PaginationLinkBuilder, CreateUserRequest, PaginatedResponse, UpdateUserRequest,
    UserDetailDto, UserDto, UserListParams, UserSharingTagGrantDto,
};
use super::paginated_response;
use crate::api::{
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::{Permission, UserRole},
};
use crate::db::entities::users;
use crate::db::repositories::{SharingTagRepository, UserListFilter, UserRepository};
use crate::require_permission;
use crate::utils::password;
use axum::{
    extract::{Path, Query, State},
    response::Response,
    Json,
};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

/// Parse permissions from JSON value (stored as array of strings in database)
fn parse_permissions_json(json: &serde_json::Value) -> Vec<String> {
    json.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Get the currently authenticated user's profile
#[utoipa::path(
    get,
    path = "/api/v1/user",
    responses(
        (status = 200, description = "Current user's profile with sharing tags", body = UserDetailDto),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Current User"
)]
pub async fn get_current_user(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<UserDetailDto>, ApiError> {
    // Fetch full user details from database (auth context has cached subset)
    let user = UserRepository::get_by_id(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch user: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    // Fetch sharing tag grants for the user
    let grants_with_tags =
        SharingTagRepository::get_grants_with_tags_for_user(&state.db, auth.user_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch user sharing tags: {}", e)))?;

    let sharing_tags: Vec<UserSharingTagGrantDto> = grants_with_tags
        .into_iter()
        .map(|(grant, tag)| UserSharingTagGrantDto::from_models(grant, tag))
        .collect();

    let role = user.get_role();
    let permissions = parse_permissions_json(&user.permissions);
    let dto = UserDetailDto {
        id: user.id,
        username: user.username,
        email: user.email,
        role,
        permissions,
        is_active: user.is_active,
        last_login_at: user.last_login_at,
        created_at: user.created_at,
        updated_at: user.updated_at,
        sharing_tags,
    };

    Ok(Json(dto))
}

/// List all users (admin only) with pagination and filtering
#[utoipa::path(
    get,
    path = "/api/v1/users",
    params(UserListParams),
    responses(
        (status = 200, description = "Paginated list of users", body = PaginatedResponse<UserDto>),
        (status = 403, description = "Forbidden - Admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Users"
)]
pub async fn list_users(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(params): Query<UserListParams>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::UsersRead)?;

    // Validate and clamp pagination params (1-indexed)
    let params = params.validate(100);

    // Build filter
    let filter = UserListFilter {
        role: params.role.as_ref().map(|r| r.to_string()),
        sharing_tag: params.sharing_tag.clone(),
        sharing_tag_mode: params.sharing_tag_mode.clone(),
    };

    let result =
        UserRepository::list_paginated(&state.db, &filter, params.offset(), params.limit())
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch users: {}", e)))?;

    let dtos: Vec<UserDto> = result
        .users
        .into_iter()
        .map(|user| {
            let role = user.get_role();
            let permissions = parse_permissions_json(&user.permissions);
            UserDto {
                id: user.id,
                username: user.username,
                email: user.email,
                role,
                permissions,
                is_active: user.is_active,
                last_login_at: user.last_login_at,
                created_at: user.created_at,
                updated_at: user.updated_at,
            }
        })
        .collect();

    // Build pagination links
    let total_pages = if params.page_size == 0 {
        0
    } else {
        result.total.div_ceil(params.page_size)
    };
    let mut link_builder =
        PaginationLinkBuilder::new("/api/v1/users", params.page, params.page_size, total_pages);
    if let Some(ref role) = params.role {
        link_builder = link_builder.with_param("role", &role.to_string());
    }
    if let Some(ref tag) = params.sharing_tag {
        link_builder = link_builder.with_param("sharing_tag", tag);
    }
    if let Some(ref mode) = params.sharing_tag_mode {
        link_builder = link_builder.with_param("sharing_tag_mode", mode);
    }

    let response = PaginatedResponse::with_builder(
        dtos,
        params.page,
        params.page_size,
        result.total,
        &link_builder,
    );

    Ok(paginated_response(response, &link_builder))
}

/// Get user by ID (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/users/{user_id}",
    params(
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User details with sharing tags", body = UserDetailDto),
        (status = 404, description = "User not found"),
        (status = 403, description = "Forbidden - Admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Users"
)]
pub async fn get_user(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(user_id): Path<Uuid>,
) -> Result<Json<UserDetailDto>, ApiError> {
    require_permission!(auth, Permission::UsersRead)?;

    let user = UserRepository::get_by_id(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch user: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    // Fetch sharing tag grants for the user
    let grants_with_tags = SharingTagRepository::get_grants_with_tags_for_user(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch user sharing tags: {}", e)))?;

    let sharing_tags: Vec<UserSharingTagGrantDto> = grants_with_tags
        .into_iter()
        .map(|(grant, tag)| UserSharingTagGrantDto::from_models(grant, tag))
        .collect();

    let role = user.get_role();
    let permissions = parse_permissions_json(&user.permissions);
    let dto = UserDetailDto {
        id: user.id,
        username: user.username,
        email: user.email,
        role,
        permissions,
        is_active: user.is_active,
        last_login_at: user.last_login_at,
        created_at: user.created_at,
        updated_at: user.updated_at,
        sharing_tags,
    };

    Ok(Json(dto))
}

/// Create a new user (admin only)
#[utoipa::path(
    post,
    path = "/api/v1/users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created", body = UserDto),
        (status = 400, description = "Invalid request"),
        (status = 403, description = "Forbidden - Admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Users"
)]
pub async fn create_user(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<CreateUserRequest>,
) -> Result<Json<UserDto>, ApiError> {
    require_permission!(auth, Permission::UsersWrite)?;

    // Validate username is not taken
    if let Ok(Some(_)) = UserRepository::get_by_username(&state.db, &request.username).await {
        return Err(ApiError::BadRequest("Username already exists".to_string()));
    }

    // Validate email is not taken
    if let Ok(Some(_)) = UserRepository::get_by_email(&state.db, &request.email).await {
        return Err(ApiError::BadRequest("Email already exists".to_string()));
    }

    // Hash password
    let password_hash = password::hash_password(&request.password)
        .map_err(|e| ApiError::Internal(format!("Failed to hash password: {}", e)))?;

    // Get role (defaults to Reader if not specified)
    let role = request.role.unwrap_or(UserRole::Reader);

    let now = Utc::now();
    let model = users::Model {
        id: Uuid::new_v4(),
        username: request.username,
        email: request.email,
        password_hash,
        role: role.to_string(),
        is_active: true,
        email_verified: true, // Admin-created users are verified by default
        permissions: serde_json::json!([]), // Custom permissions (empty = use role defaults)
        last_login_at: None,
        created_at: now,
        updated_at: now,
    };

    let user = UserRepository::create(&state.db, &model)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create user: {}", e)))?;

    let role = user.get_role();
    let permissions = parse_permissions_json(&user.permissions);
    let dto = UserDto {
        id: user.id,
        username: user.username,
        email: user.email,
        role,
        permissions,
        is_active: user.is_active,
        last_login_at: user.last_login_at,
        created_at: user.created_at,
        updated_at: user.updated_at,
    };

    Ok(Json(dto))
}

/// Update a user (admin only, partial update)
#[utoipa::path(
    patch,
    path = "/api/v1/users/{user_id}",
    params(
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "User updated", body = UserDto),
        (status = 404, description = "User not found"),
        (status = 403, description = "Forbidden - Admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Users"
)]
pub async fn update_user(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(user_id): Path<Uuid>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<UserDto>, ApiError> {
    require_permission!(auth, Permission::UsersWrite)?;

    // Fetch existing user
    let mut user = UserRepository::get_by_id(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch user: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    // Update fields
    if let Some(username) = request.username {
        // Check username is not taken by another user
        if let Ok(Some(existing)) = UserRepository::get_by_username(&state.db, &username).await {
            if existing.id != user_id {
                return Err(ApiError::BadRequest("Username already exists".to_string()));
            }
        }
        user.username = username;
    }

    if let Some(email) = request.email {
        // Check email is not taken by another user
        if let Ok(Some(existing)) = UserRepository::get_by_email(&state.db, &email).await {
            if existing.id != user_id {
                return Err(ApiError::BadRequest("Email already exists".to_string()));
            }
        }
        user.email = email;
    }

    if let Some(password) = request.password {
        let password_hash = password::hash_password(&password)
            .map_err(|e| ApiError::Internal(format!("Failed to hash password: {}", e)))?;
        user.password_hash = password_hash;
    }

    if let Some(role) = request.role {
        user.role = role.to_string();
    }

    if let Some(is_active) = request.is_active {
        user.is_active = is_active;
    }

    if let Some(permissions) = request.permissions {
        // Validate permissions are valid permission strings
        for perm in &permissions {
            // Normalize: convert kebab-case to colon format if needed
            let normalized = perm.replace('-', ":");
            if normalized.parse::<Permission>().is_err() {
                return Err(ApiError::BadRequest(format!(
                    "Invalid permission: {}",
                    perm
                )));
            }
        }
        user.permissions = serde_json::json!(permissions);
    }

    user.updated_at = Utc::now();

    let updated = UserRepository::update(&state.db, &user)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update user: {}", e)))?;

    let role = updated.get_role();
    let permissions = parse_permissions_json(&updated.permissions);
    let dto = UserDto {
        id: updated.id,
        username: updated.username,
        email: updated.email,
        role,
        permissions,
        is_active: updated.is_active,
        last_login_at: updated.last_login_at,
        created_at: updated.created_at,
        updated_at: updated.updated_at,
    };

    Ok(Json(dto))
}

/// Delete a user (admin only)
#[utoipa::path(
    delete,
    path = "/api/v1/users/{user_id}",
    params(
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 204, description = "User deleted"),
        (status = 404, description = "User not found"),
        (status = 403, description = "Forbidden - Admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Users"
)]
pub async fn delete_user(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(user_id): Path<Uuid>,
) -> Result<(), ApiError> {
    require_permission!(auth, Permission::UsersDelete)?;

    // Prevent self-deletion
    if auth.user_id == user_id {
        return Err(ApiError::BadRequest(
            "Cannot delete your own account".to_string(),
        ));
    }

    UserRepository::delete(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete user: {}", e)))?;

    Ok(())
}
