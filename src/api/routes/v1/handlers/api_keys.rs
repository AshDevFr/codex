use super::super::dto::{
    ApiKeyDto, CreateApiKeyRequest, CreateApiKeyResponse, UpdateApiKeyRequest,
    common::{
        DEFAULT_PAGE, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE, PaginatedResponse, PaginationLinkBuilder,
    },
};
use super::paginated_response;
use crate::api::{
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::{Permission, serialize_permissions},
};
use crate::db::entities::api_keys;
use crate::db::repositories::ApiKeyRepository;
use crate::utils::password;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
};
use chrono::Utc;
use rand::RngExt;
use sea_orm::ActiveModelTrait;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

/// Query parameters for listing API keys
#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct ApiKeyListParams {
    /// Page number (1-indexed, default 1)
    #[serde(default = "default_page")]
    pub page: u64,

    /// Number of items per page (default 50, max 500)
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_page_size() -> u64 {
    DEFAULT_PAGE_SIZE
}

/// List API keys for the authenticated user
/// Users can only see their own keys unless they are admin
#[utoipa::path(
    get,
    path = "/api/v1/api-keys",
    params(ApiKeyListParams),
    responses(
        (status = 200, description = "List of API keys", body = PaginatedResponse<ApiKeyDto>),
        (status = 403, description = "Forbidden - Missing permission"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "API Keys"
)]
pub async fn list_api_keys(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(params): Query<ApiKeyListParams>,
) -> Result<Response, ApiError> {
    auth.require_permission(&Permission::ApiKeysRead)?;

    // Validate and clamp pagination params
    let page = params.page.max(1);
    let page_size = params.page_size.clamp(1, MAX_PAGE_SIZE);

    // Users can only see their own keys
    // Admins with UsersRead permission could theoretically see all keys,
    // but for now we show only the user's own keys
    let user_id = auth.user_id;

    let keys = ApiKeyRepository::list_by_user(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch API keys: {}", e)))?;

    let total = keys.len() as u64;
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };

    // Apply in-memory pagination
    let offset = (page - 1) * page_size;
    let paginated_keys: Vec<_> = keys
        .into_iter()
        .skip(offset as usize)
        .take(page_size as usize)
        .collect();

    let dtos: Vec<ApiKeyDto> = paginated_keys
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

    // Build pagination links
    let link_builder = PaginationLinkBuilder::new("/api/v1/api-keys", page, page_size, total_pages);

    let response = PaginatedResponse::with_builder(dtos, page, page_size, total, &link_builder);

    Ok(paginated_response(response, &link_builder))
}

/// Get API key by ID
/// Users can only get their own keys unless they are admin
#[utoipa::path(
    get,
    path = "/api/v1/api-keys/{api_key_id}",
    params(
        ("api_key_id" = Uuid, Path, description = "API key ID")
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
    tag = "API Keys"
)]
pub async fn get_api_key(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(api_key_id): Path<Uuid>,
) -> Result<Json<ApiKeyDto>, ApiError> {
    auth.require_permission(&Permission::ApiKeysRead)?;

    let key = ApiKeyRepository::get_by_id(&state.db, api_key_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch API key: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("API key not found".to_string()))?;

    // Users can only access their own keys unless they have UsersRead permission
    let can_access_others = auth.has_permission(&Permission::UsersRead);
    if !can_access_others && key.user_id != auth.user_id {
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
    tag = "API Keys"
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
            // Users can only grant permissions they have
            // (Admins have all permissions via their role, so no special case needed)
            if !auth.has_permission(&perm) {
                return Err(ApiError::Forbidden(format!(
                    "You don't have permission to grant: {}",
                    perm_str
                )));
            }
            perms.insert(perm);
        }
        perms
    } else {
        // Use user's current effective permissions (role + custom)
        auth.effective_permissions()
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

/// Update an API key (partial update)
/// Users can only update their own keys unless they are admin
#[utoipa::path(
    patch,
    path = "/api/v1/api-keys/{api_key_id}",
    params(
        ("api_key_id" = Uuid, Path, description = "API key ID")
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
    tag = "API Keys"
)]
pub async fn update_api_key(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(api_key_id): Path<Uuid>,
    Json(request): Json<UpdateApiKeyRequest>,
) -> Result<Json<ApiKeyDto>, ApiError> {
    auth.require_permission(&Permission::ApiKeysWrite)?;

    // Fetch existing API key
    let key = ApiKeyRepository::get_by_id(&state.db, api_key_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch API key: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("API key not found".to_string()))?;

    // Users can only update their own keys unless they have UsersWrite permission
    let can_modify_others = auth.has_permission(&Permission::UsersWrite);
    if !can_modify_others && key.user_id != auth.user_id {
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
            // Users can only grant permissions they have
            // (Admins have all permissions via their role, so no special case needed)
            if !auth.has_permission(&perm) {
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
    path = "/api/v1/api-keys/{api_key_id}",
    params(
        ("api_key_id" = Uuid, Path, description = "API key ID")
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
    tag = "API Keys"
)]
pub async fn delete_api_key(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(api_key_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    auth.require_permission(&Permission::ApiKeysDelete)?;

    // Fetch existing API key to check ownership
    let key = ApiKeyRepository::get_by_id(&state.db, api_key_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch API key: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("API key not found".to_string()))?;

    // Users can only delete their own keys unless they have UsersDelete permission
    let can_delete_others = auth.has_permission(&Permission::UsersDelete);
    if !can_delete_others && key.user_id != auth.user_id {
        return Err(ApiError::Forbidden(
            "You can only delete your own API keys".to_string(),
        ));
    }

    ApiKeyRepository::delete(&state.db, api_key_id)
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
    let mut rng = rand::rng();

    // Generate random components
    let prefix_random: String = (0..16)
        .map(|_| format!("{:x}", rng.random::<u8>() % 16))
        .collect();
    let suffix_random: String = (0..32)
        .map(|_| format!("{:x}", rng.random::<u8>() % 16))
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
