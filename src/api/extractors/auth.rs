use crate::api::error::ApiError;
use crate::api::permissions::Permission;
use crate::db::repositories::{ApiKeyRepository, UserRepository};
use crate::utils::{jwt::JwtService, password};
use axum::http::header::COOKIE;
use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

/// Authentication context extracted from JWT or API key
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub username: String,
    pub is_admin: bool,
    pub permissions: Vec<Permission>,
    pub auth_method: AuthMethod,
}

#[derive(Debug, Clone)]
pub enum AuthMethod {
    Jwt,
    ApiKey,
    BasicAuth,
}

impl AuthContext {
    /// Check if the user has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.is_admin || self.permissions.contains(permission)
    }

    /// Check if the user has any of the specified permissions
    pub fn has_any_permission(&self, permissions: &[Permission]) -> bool {
        if self.is_admin {
            return true;
        }
        permissions.iter().any(|p| self.permissions.contains(p))
    }

    /// Require a specific permission (returns error if missing)
    pub fn require_permission(&self, permission: &Permission) -> Result<(), ApiError> {
        if self.has_permission(permission) {
            Ok(())
        } else {
            Err(ApiError::Forbidden(format!(
                "Missing required permission: {:?}",
                permission
            )))
        }
    }

    /// Require admin access
    pub fn require_admin(&self) -> Result<(), ApiError> {
        if self.is_admin {
            Ok(())
        } else {
            Err(ApiError::Forbidden("Admin access required".to_string()))
        }
    }
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub jwt_service: Arc<JwtService>,
    pub auth_config: Arc<crate::config::AuthConfig>,
    pub email_service: Arc<crate::services::email::EmailService>,
    pub event_broadcaster: Arc<crate::events::EventBroadcaster>,
    pub settings_service: Arc<crate::services::SettingsService>,
    pub thumbnail_service: Arc<crate::services::ThumbnailService>,
    /// Task metrics service for collecting task performance data
    /// None in test environments or when not needed
    pub task_metrics_service: Option<Arc<crate::services::TaskMetricsService>>,
    /// Scheduler for managing scheduled tasks (library scans, deduplication, etc.)
    /// None when workers are disabled (CODEX_DISABLE_WORKERS=true) or in test environments
    pub scheduler: Option<Arc<tokio::sync::Mutex<crate::scheduler::Scheduler>>>,
}

// Legacy alias for backwards compatibility during transition
pub type AuthState = AppState;

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthContext {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        // Try to extract from Authorization header
        if let Some(auth_header) = parts.headers.get("authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                // Try JWT Bearer token
                if let Some(token) = auth_str.strip_prefix("Bearer ") {
                    return extract_from_jwt(token, state).await;
                }
                // Try HTTP Basic authentication
                if let Some(credentials) = auth_str.strip_prefix("Basic ") {
                    return extract_from_basic_auth(credentials, state).await;
                }
            }
        }

        // Try to extract from X-API-Key header
        if let Some(api_key_header) = parts.headers.get("x-api-key") {
            if let Ok(api_key) = api_key_header.to_str() {
                return extract_from_api_key(api_key, state).await;
            }
        }

        Err(ApiError::Unauthorized(
            "Missing or invalid authentication credentials".to_string(),
        ))
    }
}

/// Extract auth context from JWT token
async fn extract_from_jwt(token: &str, state: &AppState) -> Result<AuthContext, ApiError> {
    // Verify and decode JWT
    let claims = state
        .jwt_service
        .verify_token(token)
        .map_err(|e| ApiError::Unauthorized(format!("Invalid JWT token: {}", e)))?;

    // Parse user ID
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::Unauthorized("Invalid user ID in token".to_string()))?;

    // Load user from database to get current permissions
    let user = UserRepository::get_by_id(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load user: {}", e)))?
        .ok_or_else(|| ApiError::Unauthorized("User not found".to_string()))?;

    // Check if user is active
    if !user.is_active {
        return Err(ApiError::Unauthorized(
            "User account is inactive".to_string(),
        ));
    }

    // Parse permissions from JSON
    let permissions: Vec<Permission> = serde_json::from_value(user.permissions)
        .map_err(|e| ApiError::Internal(format!("Failed to parse permissions: {}", e)))?;

    Ok(AuthContext {
        user_id,
        username: claims.username,
        is_admin: claims.is_admin,
        permissions,
        auth_method: AuthMethod::Jwt,
    })
}

/// Extract auth context from API key
async fn extract_from_api_key(api_key: &str, state: &AppState) -> Result<AuthContext, ApiError> {
    // Extract the prefix from the API key (format: codex_<prefix>_<secret>)
    // We use the first underscore-delimited part as the prefix for lookup
    let key_prefix = api_key.split('_').take(2).collect::<Vec<&str>>().join("_");

    // Look up all API keys with this prefix
    let candidate_keys = ApiKeyRepository::get_by_prefix(&state.db, &key_prefix)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load API keys: {}", e)))?;

    // Try to verify the API key against each candidate
    let mut api_key_model = None;
    for candidate in candidate_keys {
        if password::verify_password(api_key, &candidate.key_hash).unwrap_or(false) {
            api_key_model = Some(candidate);
            break;
        }
    }

    let api_key_model =
        api_key_model.ok_or_else(|| ApiError::Unauthorized("Invalid API key".to_string()))?;

    // Check if API key is expired
    if let Some(expires_at) = api_key_model.expires_at {
        if expires_at < chrono::Utc::now() {
            return Err(ApiError::Unauthorized("API key has expired".to_string()));
        }
    }

    // Load user associated with API key
    let user = UserRepository::get_by_id(&state.db, api_key_model.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load user: {}", e)))?
        .ok_or_else(|| ApiError::Unauthorized("User not found".to_string()))?;

    // Check if user is active
    if !user.is_active {
        return Err(ApiError::Unauthorized(
            "User account is inactive".to_string(),
        ));
    }

    // Parse permissions from API key (stored as JSON string)
    let permissions: Vec<Permission> = serde_json::from_value(api_key_model.permissions)
        .map_err(|e| ApiError::Internal(format!("Failed to parse permissions: {}", e)))?;

    // Update last used timestamp (fire and forget - don't block on this)
    let db = state.db.clone();
    let key_id = api_key_model.id;
    tokio::spawn(async move {
        let _ = ApiKeyRepository::update_last_used(&db, key_id).await;
    });

    Ok(AuthContext {
        user_id: user.id,
        username: user.username,
        is_admin: user.is_admin,
        permissions,
        auth_method: AuthMethod::ApiKey,
    })
}

/// Extract auth context from HTTP Basic authentication
async fn extract_from_basic_auth(
    credentials: &str,
    state: &AppState,
) -> Result<AuthContext, ApiError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    // Decode base64 credentials
    let decoded = STANDARD
        .decode(credentials)
        .map_err(|_| ApiError::Unauthorized("Invalid Basic auth encoding".to_string()))?;

    let credentials_str = String::from_utf8(decoded)
        .map_err(|_| ApiError::Unauthorized("Invalid Basic auth credentials".to_string()))?;

    // Split into username and password (format: "username:password")
    let parts: Vec<&str> = credentials_str.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(ApiError::Unauthorized(
            "Invalid Basic auth format".to_string(),
        ));
    }

    let username = parts[0];
    let password = parts[1];

    // Look up user by username
    let user = UserRepository::get_by_username(&state.db, username)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load user: {}", e)))?
        .ok_or_else(|| ApiError::Unauthorized("Invalid username or password".to_string()))?;

    // Verify password
    let password_valid = password::verify_password(password, &user.password_hash)
        .map_err(|e| ApiError::Internal(format!("Failed to verify password: {}", e)))?;

    if !password_valid {
        return Err(ApiError::Unauthorized(
            "Invalid username or password".to_string(),
        ));
    }

    // Check if user is active
    if !user.is_active {
        return Err(ApiError::Unauthorized(
            "User account is inactive".to_string(),
        ));
    }

    // Parse permissions from user model
    let permissions: Vec<Permission> = serde_json::from_value(user.permissions)
        .map_err(|e| ApiError::Internal(format!("Failed to parse permissions: {}", e)))?;

    // Update last login timestamp (fire and forget - don't block on this)
    let db = state.db.clone();
    let user_id = user.id;
    tokio::spawn(async move {
        let _ = UserRepository::update_last_login(&db, user_id).await;
    });

    Ok(AuthContext {
        user_id: user.id,
        username: user.username,
        is_admin: user.is_admin,
        permissions,
        auth_method: AuthMethod::BasicAuth,
    })
}

/// Flexible authentication context that accepts both Bearer tokens and cookies
/// Used primarily for thumbnail endpoints to allow browser image tags to work
#[derive(Debug, Clone)]
pub struct FlexibleAuthContext(pub AuthContext);

#[async_trait]
impl FromRequestParts<Arc<AppState>> for FlexibleAuthContext {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        // Try Authorization header first (Bearer token, Basic auth, API key)
        if let Some(auth_header) = parts.headers.get("authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                // Try JWT Bearer token
                if let Some(token) = auth_str.strip_prefix("Bearer ") {
                    return extract_from_jwt(token, state)
                        .await
                        .map(FlexibleAuthContext);
                }
                // Try HTTP Basic authentication
                if let Some(credentials) = auth_str.strip_prefix("Basic ") {
                    return extract_from_basic_auth(credentials, state)
                        .await
                        .map(FlexibleAuthContext);
                }
            }
        }

        // Try X-API-Key header
        if let Some(api_key_header) = parts.headers.get("x-api-key") {
            if let Ok(api_key) = api_key_header.to_str() {
                return extract_from_api_key(api_key, state)
                    .await
                    .map(FlexibleAuthContext);
            }
        }

        // Try cookie as fallback
        if let Some(cookie_header) = parts.headers.get(COOKIE) {
            if let Ok(cookie_str) = cookie_header.to_str() {
                // Parse cookies to find auth_token
                if let Some(token) = extract_token_from_cookies(cookie_str) {
                    return extract_from_jwt(&token, state)
                        .await
                        .map(FlexibleAuthContext);
                }
            }
        }

        Err(ApiError::Unauthorized(
            "Missing or invalid authentication credentials".to_string(),
        ))
    }
}

/// Extract auth_token value from cookie header string
fn extract_token_from_cookies(cookie_str: &str) -> Option<String> {
    for cookie in cookie_str.split(';') {
        let cookie = cookie.trim();
        if let Some(value) = cookie.strip_prefix("auth_token=") {
            return Some(value.to_string());
        }
    }
    None
}
