use crate::api::{
    dto::{ApiKeyDto, CreateApiKeyRequest, CreateApiKeyResponse, UpdateApiKeyRequest},
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::{serialize_permissions, Permission},
};
use crate::db::entities::api_keys;
use crate::db::repositories::ApiKeyRepository;
use crate::utils::password;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rand::Rng;
use sea_orm::ActiveModelTrait;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

/// List API keys for the authenticated user
/// Users can only see their own keys unless they are admin
#[utoipa::path(
    get,
    path = "/api/v1/api-keys",
    responses(
        (status = 200, description = "List of API keys", body = Vec<ApiKeyDto>),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "api-keys"
)]
pub async fn list_api_keys(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<Vec<ApiKeyDto>>, ApiError> {
    auth.require_permission(&Permission::ApiKeysRead)?;

    // Users can only see their own keys unless admin
    let user_id = if auth.is_admin {
        // Admin can see all keys - for now, we'll show only their own
        // If you want admins to see all keys, you'd need to add a query parameter
        auth.user_id
    } else {
        auth.user_id
    };

    let keys = ApiKeyRepository::list_by_user(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch API keys: {}", e)))?;

    let dtos: Vec<ApiKeyDto> = keys
        .into_iter()
        .map(|key| ApiKeyDto {
            id: key.id,
            user_id: key.user_id,
            name: key.name,
            key_prefix: key.key_prefix,
            permissions: key.permissions,
            is_active: key.is_active,
            expires_at: key.expires_at,
            last_used_at: key.last_used_at,
            created_at: key.created_at,
            updated_at: key.updated_at,
        })
        .collect();

    Ok(Json(dtos))
}

/// Get API key by ID
/// Users can only get their own keys unless they are admin
#[utoipa::path(
    get,
    path = "/api/v1/api-keys/{id}",
    params(
        ("id" = Uuid, Path, description = "API key ID")
    ),
    responses(
        (status = 200, description = "API key details", body = ApiKeyDto),
        (status = 404, description = "API key not found"),
        (status = 403, description = "Forbidden - Missing permission or not owner"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "api-keys"
)]
pub async fn get_api_key(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiKeyDto>, ApiError> {
    auth.require_permission(&Permission::ApiKeysRead)?;

    let key = ApiKeyRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch API key: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("API key not found".to_string()))?;

    // Users can only access their own keys unless admin
    if !auth.is_admin && key.user_id != auth.user_id {
        return Err(ApiError::Forbidden(
            "You can only access your own API keys".to_string(),
        ));
    }

    let dto = ApiKeyDto {
        id: key.id,
        user_id: key.user_id,
        name: key.name,
        key_prefix: key.key_prefix,
        permissions: key.permissions,
        is_active: key.is_active,
        expires_at: key.expires_at,
        last_used_at: key.last_used_at,
        created_at: key.created_at,
        updated_at: key.updated_at,
    };

    Ok(Json(dto))
}

/// Create a new API key
/// API keys are always associated with the authenticated user
#[utoipa::path(
    post,
    path = "/api/v1/api-keys",
    request_body = CreateApiKeyRequest,
    responses(
        (status = 201, description = "API key created", body = CreateApiKeyResponse),
        (status = 400, description = "Invalid request"),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "api-keys"
)]
pub async fn create_api_key(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), ApiError> {
    auth.require_permission(&Permission::ApiKeysWrite)?;

    // Determine permissions for the new API key
    let permissions: HashSet<Permission> = if let Some(perm_strings) = request.permissions {
        // Parse provided permissions
        let mut perms = HashSet::new();
        for perm_str in perm_strings {
            // Normalize permission string: convert kebab-case to colon format
            // e.g., "libraries-read" -> "libraries:read"
            let normalized = perm_str.replace('-', ":");
            let perm = normalized.parse::<Permission>().map_err(|e| {
                ApiError::BadRequest(format!("Invalid permission: {} ({})", perm_str, e))
            })?;
            // Users can only grant permissions they have (unless admin)
            if !auth.is_admin && !auth.has_permission(&perm) {
                return Err(ApiError::Forbidden(format!(
                    "You don't have permission to grant: {}",
                    perm_str
                )));
            }
            perms.insert(perm);
        }
        perms
    } else {
        // Use user's current permissions
        auth.permissions.iter().cloned().collect()
    };

    // Generate API key
    let (plaintext_key, api_key_model) = generate_api_key(auth.user_id, request.name, &permissions)
        .map_err(|e| ApiError::Internal(format!("Failed to generate API key: {}", e)))?;

    // Set expiration if provided
    let mut model = api_key_model;
    if let Some(expires_at) = request.expires_at {
        model.expires_at = Some(expires_at);
    }

    let created = ApiKeyRepository::create(&state.db, &model)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create API key: {}", e)))?;

    let dto = ApiKeyDto {
        id: created.id,
        user_id: created.user_id,
        name: created.name,
        key_prefix: created.key_prefix,
        permissions: created.permissions,
        is_active: created.is_active,
        expires_at: created.expires_at,
        last_used_at: created.last_used_at,
        created_at: created.created_at,
        updated_at: created.updated_at,
    };

    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            api_key: dto,
            key: plaintext_key,
        }),
    ))
}

/// Update an API key
/// Users can only update their own keys unless they are admin
#[utoipa::path(
    put,
    path = "/api/v1/api-keys/{id}",
    params(
        ("id" = Uuid, Path, description = "API key ID")
    ),
    request_body = UpdateApiKeyRequest,
    responses(
        (status = 200, description = "API key updated", body = ApiKeyDto),
        (status = 404, description = "API key not found"),
        (status = 403, description = "Forbidden - Missing permission or not owner"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "api-keys"
)]
pub async fn update_api_key(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateApiKeyRequest>,
) -> Result<Json<ApiKeyDto>, ApiError> {
    auth.require_permission(&Permission::ApiKeysWrite)?;

    // Fetch existing API key
    let mut key = ApiKeyRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch API key: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("API key not found".to_string()))?;

    // Users can only update their own keys unless admin
    if !auth.is_admin && key.user_id != auth.user_id {
        return Err(ApiError::Forbidden(
            "You can only update your own API keys".to_string(),
        ));
    }

    // Convert to ActiveModel and update fields using Set()
    use sea_orm::ActiveValue::Set;
    let mut active_model: api_keys::ActiveModel = key.into();

    // Update fields
    if let Some(name) = request.name {
        active_model.name = Set(name);
    }

    if let Some(perm_strings) = request.permissions {
        // Parse provided permissions
        let mut perms = HashSet::new();
        for perm_str in perm_strings {
            // Normalize permission string: convert kebab-case to colon format
            // e.g., "libraries-read" -> "libraries:read"
            let normalized = perm_str.replace('-', ":");
            let perm = normalized.parse::<Permission>().map_err(|e| {
                ApiError::BadRequest(format!("Invalid permission: {} ({})", perm_str, e))
            })?;
            // Users can only grant permissions they have (unless admin)
            if !auth.is_admin && !auth.has_permission(&perm) {
                return Err(ApiError::Forbidden(format!(
                    "You don't have permission to grant: {}",
                    perm_str
                )));
            }
            perms.insert(perm);
        }
        let permissions_json = serialize_permissions(&perms);
        active_model.permissions =
            Set(serde_json::from_str(&permissions_json).unwrap_or_else(|_| serde_json::json!([])));
    }

    if let Some(is_active) = request.is_active {
        active_model.is_active = Set(is_active);
    }

    if let Some(expires_at) = request.expires_at {
        active_model.expires_at = Set(Some(expires_at));
    }

    active_model.updated_at = Set(Utc::now());

    // Update in database
    let updated_key = active_model
        .update(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update API key: {}", e)))?;

    let dto = ApiKeyDto {
        id: updated_key.id,
        user_id: updated_key.user_id,
        name: updated_key.name,
        key_prefix: updated_key.key_prefix,
        permissions: updated_key.permissions,
        is_active: updated_key.is_active,
        expires_at: updated_key.expires_at,
        last_used_at: updated_key.last_used_at,
        created_at: updated_key.created_at,
        updated_at: updated_key.updated_at,
    };

    Ok(Json(dto))
}

/// Delete an API key
/// Users can only delete their own keys unless they are admin
#[utoipa::path(
    delete,
    path = "/api/v1/api-keys/{id}",
    params(
        ("id" = Uuid, Path, description = "API key ID")
    ),
    responses(
        (status = 204, description = "API key deleted"),
        (status = 404, description = "API key not found"),
        (status = 403, description = "Forbidden - Missing permission or not owner"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "api-keys"
)]
pub async fn delete_api_key(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::ApiKeysDelete)?;

    // Fetch existing API key to check ownership
    let key = ApiKeyRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch API key: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("API key not found".to_string()))?;

    // Users can only delete their own keys unless admin
    if !auth.is_admin && key.user_id != auth.user_id {
        return Err(ApiError::Forbidden(
            "You can only delete your own API keys".to_string(),
        ));
    }

    ApiKeyRepository::delete(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete API key: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Generate an API key
/// Returns: (plaintext_key, api_key_model)
fn generate_api_key(
    user_id: Uuid,
    name: String,
    permissions: &HashSet<Permission>,
) -> Result<(String, api_keys::Model), anyhow::Error> {
    let mut rng = rand::thread_rng();

    // Generate random components
    let prefix_random: String = (0..16)
        .map(|_| format!("{:x}", rng.gen::<u8>() % 16))
        .collect();
    let suffix_random: String = (0..32)
        .map(|_| format!("{:x}", rng.gen::<u8>() % 16))
        .collect();

    // Construct full key
    let api_key = format!("codex_{}_{}", prefix_random, suffix_random);

    // Hash the full key for storage
    let key_hash = password::hash_password(&api_key)?;

    // Store prefix for lookup (must match auth extractor logic)
    let key_prefix = format!("codex_{}", prefix_random);

    let permissions_json = serialize_permissions(permissions);
    let api_key_model = api_keys::Model {
        id: Uuid::new_v4(),
        user_id,
        name,
        key_hash,
        key_prefix,
        permissions: serde_json::from_str(&permissions_json)
            .unwrap_or_else(|_| serde_json::json!([])),
        is_active: true,
        expires_at: None,
        last_used_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    Ok((api_key, api_key_model))
}
