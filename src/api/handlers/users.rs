use crate::api::{
    dto::{CreateUserRequest, UpdateUserRequest, UserDto},
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::entities::users;
use crate::db::repositories::UserRepository;
use crate::require_admin;
use crate::utils::password;
use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

/// List all users (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/users",
    responses(
        (status = 200, description = "List of users", body = Vec<UserDto>),
        (status = 403, description = "Forbidden - Admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "users"
)]
pub async fn list_users(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<Vec<UserDto>>, ApiError> {
    require_admin!(auth)?;

    let users = UserRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch users: {}", e)))?;

    let dtos: Vec<UserDto> = users
        .into_iter()
        .map(|user| UserDto {
            id: user.id,
            username: user.username,
            email: user.email,
            is_admin: user.is_admin,
            is_active: user.is_active,
            last_login_at: user.last_login_at,
            created_at: user.created_at,
            updated_at: user.updated_at,
        })
        .collect();

    Ok(Json(dtos))
}

/// Get user by ID (admin only)
#[utoipa::path(
    get,
    path = "/api/v1/users/{id}",
    params(
        ("id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User details", body = UserDto),
        (status = 404, description = "User not found"),
        (status = 403, description = "Forbidden - Admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "users"
)]
pub async fn get_user(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<UserDto>, ApiError> {
    require_admin!(auth)?;

    let user = UserRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch user: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    let dto = UserDto {
        id: user.id,
        username: user.username,
        email: user.email,
        is_admin: user.is_admin,
        is_active: user.is_active,
        last_login_at: user.last_login_at,
        created_at: user.created_at,
        updated_at: user.updated_at,
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
    tag = "users"
)]
pub async fn create_user(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<CreateUserRequest>,
) -> Result<Json<UserDto>, ApiError> {
    require_admin!(auth)?;

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

    // Determine permissions based on admin flag
    let permissions = if request.is_admin {
        let perms: Vec<_> = crate::api::permissions::ADMIN_PERMISSIONS.iter().cloned().collect();
        serde_json::to_value(perms)
            .map_err(|e| ApiError::Internal(format!("Failed to serialize permissions: {}", e)))?
    } else {
        let perms: Vec<_> = crate::api::permissions::READONLY_PERMISSIONS.iter().cloned().collect();
        serde_json::to_value(perms)
            .map_err(|e| ApiError::Internal(format!("Failed to serialize permissions: {}", e)))?
    };

    let now = Utc::now();
    let model = users::Model {
        id: Uuid::new_v4(),
        username: request.username,
        email: request.email,
        password_hash,
        is_admin: request.is_admin,
        is_active: true,
        permissions,
        last_login_at: None,
        created_at: now,
        updated_at: now,
    };

    let user = UserRepository::create(&state.db, &model)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create user: {}", e)))?;

    let dto = UserDto {
        id: user.id,
        username: user.username,
        email: user.email,
        is_admin: user.is_admin,
        is_active: user.is_active,
        last_login_at: user.last_login_at,
        created_at: user.created_at,
        updated_at: user.updated_at,
    };

    Ok(Json(dto))
}

/// Update a user (admin only)
#[utoipa::path(
    put,
    path = "/api/v1/users/{id}",
    params(
        ("id" = Uuid, Path, description = "User ID")
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
    tag = "users"
)]
pub async fn update_user(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<UserDto>, ApiError> {
    require_admin!(auth)?;

    // Fetch existing user
    let mut user = UserRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch user: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    // Update fields
    if let Some(username) = request.username {
        // Check username is not taken by another user
        if let Ok(Some(existing)) = UserRepository::get_by_username(&state.db, &username).await {
            if existing.id != id {
                return Err(ApiError::BadRequest("Username already exists".to_string()));
            }
        }
        user.username = username;
    }

    if let Some(email) = request.email {
        // Check email is not taken by another user
        if let Ok(Some(existing)) = UserRepository::get_by_email(&state.db, &email).await {
            if existing.id != id {
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

    if let Some(is_admin) = request.is_admin {
        user.is_admin = is_admin;

        // Update permissions based on admin flag
        let permissions = if is_admin {
            let perms: Vec<_> = crate::api::permissions::ADMIN_PERMISSIONS.iter().cloned().collect();
            serde_json::to_value(perms)
                .map_err(|e| ApiError::Internal(format!("Failed to serialize permissions: {}", e)))?
        } else {
            let perms: Vec<_> = crate::api::permissions::READONLY_PERMISSIONS.iter().cloned().collect();
            serde_json::to_value(perms)
                .map_err(|e| ApiError::Internal(format!("Failed to serialize permissions: {}", e)))?
        };
        user.permissions = permissions;
    }

    if let Some(is_active) = request.is_active {
        user.is_active = is_active;
    }

    user.updated_at = Utc::now();

    let updated = UserRepository::update(&state.db, &user)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update user: {}", e)))?;

    let dto = UserDto {
        id: updated.id,
        username: updated.username,
        email: updated.email,
        is_admin: updated.is_admin,
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
    path = "/api/v1/users/{id}",
    params(
        ("id" = Uuid, Path, description = "User ID")
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
    tag = "users"
)]
pub async fn delete_user(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<(), ApiError> {
    require_admin!(auth)?;

    // Prevent self-deletion
    if auth.user_id == id {
        return Err(ApiError::BadRequest("Cannot delete your own account".to_string()));
    }

    UserRepository::delete(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete user: {}", e)))?;

    Ok(())
}
