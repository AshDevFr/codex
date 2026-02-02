use crate::api::error::ApiError;
use crate::api::permissions::{Permission, UserRole};
use crate::db::repositories::{ApiKeyRepository, UserRepository};
use crate::utils::{jwt::JwtService, password};
use axum::http::header::COOKIE;
use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use sea_orm::DatabaseConnection;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

/// Cache TTL for user authentication data (60 seconds)
/// This reduces DB load for repeated requests from the same user
const USER_CACHE_TTL_SECS: i64 = 60;

/// Cached user data for authentication
#[derive(Clone)]
struct CachedUser {
    username: String,
    email: String,
    role: UserRole,
    custom_permissions: Vec<Permission>,
    is_active: bool,
    cached_at: DateTime<Utc>,
}

/// User authentication cache to avoid hitting the database on every request
/// Entries expire after USER_CACHE_TTL_SECS seconds
#[derive(Default)]
pub struct UserAuthCache {
    cache: DashMap<Uuid, CachedUser>,
}

impl UserAuthCache {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    /// Get a cached user if it exists and hasn't expired
    fn get(&self, user_id: &Uuid) -> Option<CachedUser> {
        if let Some(entry) = self.cache.get(user_id) {
            let age = Utc::now().signed_duration_since(entry.cached_at);
            if age.num_seconds() < USER_CACHE_TTL_SECS {
                return Some(entry.clone());
            }
            // Entry expired, remove it
            drop(entry);
            self.cache.remove(user_id);
        }
        None
    }

    /// Cache a user's auth data
    fn insert(
        &self,
        user_id: Uuid,
        username: String,
        email: String,
        role: UserRole,
        permissions: Vec<Permission>,
        is_active: bool,
    ) {
        self.cache.insert(
            user_id,
            CachedUser {
                username,
                email,
                role,
                custom_permissions: permissions,
                is_active,
                cached_at: Utc::now(),
            },
        );
    }

    /// Invalidate a user's cached auth data (e.g., after permission changes)
    #[allow(dead_code)]
    pub fn invalidate(&self, user_id: &Uuid) {
        self.cache.remove(user_id);
    }

    /// Clear all cached entries
    #[allow(dead_code)]
    pub fn clear(&self) {
        self.cache.clear();
    }
}

/// Authentication context extracted from JWT or API key
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub username: String,
    pub email: String,
    /// User's role (Reader, Maintainer, Admin)
    pub role: UserRole,
    /// Custom permissions that extend the role's base permissions
    pub custom_permissions: Vec<Permission>,
    /// How the user was authenticated - used for audit logging
    #[allow(dead_code)]
    pub auth_method: AuthMethod,
    /// For API key auth: the token's permissions (subset of user's)
    /// This is used to constrain token permissions at request time
    pub token_permissions: Option<Vec<Permission>>,
}

#[derive(Debug, Clone)]
pub enum AuthMethod {
    Jwt,
    ApiKey,
    BasicAuth,
}

impl AuthContext {
    /// Get effective permissions (role permissions ∪ custom permissions)
    /// If authenticated via API token, permissions are intersected with token permissions
    pub fn effective_permissions(&self) -> HashSet<Permission> {
        // Start with role permissions
        let mut perms: HashSet<Permission> = self.role.permissions().clone();

        // Union with custom permissions
        perms.extend(self.custom_permissions.iter().cloned());

        // If authenticated via API token, intersect with token permissions
        if let Some(token_perms) = &self.token_permissions {
            let token_set: HashSet<_> = token_perms.iter().cloned().collect();
            perms = perms.intersection(&token_set).cloned().collect();
        }

        perms
    }

    /// Check if the user has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.effective_permissions().contains(permission)
    }

    /// Check if the user has any of the specified permissions
    #[allow(dead_code)] // Public API for permission checking
    pub fn has_any_permission(&self, permissions: &[Permission]) -> bool {
        let effective = self.effective_permissions();
        permissions.iter().any(|p| effective.contains(p))
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

    /// Require admin access (checks for SystemAdmin permission)
    pub fn require_admin(&self) -> Result<(), ApiError> {
        self.require_permission(&Permission::SystemAdmin)
    }
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub jwt_service: Arc<JwtService>,
    pub auth_config: Arc<crate::config::AuthConfig>,
    /// Database configuration - used for operation deadlines and pool settings
    pub database_config: Arc<crate::config::DatabaseConfig>,
    /// PDF configuration - used for rendering settings and cache config
    pub pdf_config: Arc<crate::config::PdfConfig>,
    pub email_service: Arc<crate::services::email::EmailService>,
    pub event_broadcaster: Arc<crate::events::EventBroadcaster>,
    /// Settings service - used for runtime configuration
    #[allow(dead_code)]
    pub settings_service: Arc<crate::services::SettingsService>,
    pub thumbnail_service: Arc<crate::services::ThumbnailService>,
    /// File cleanup service for managing orphaned files
    pub file_cleanup_service: Arc<crate::services::FileCleanupService>,
    /// Task metrics service for collecting task performance data
    /// None in test environments or when not needed
    pub task_metrics_service: Option<Arc<crate::services::TaskMetricsService>>,
    /// Scheduler for managing scheduled tasks (library scans, deduplication, etc.)
    /// None when workers are disabled (CODEX_DISABLE_WORKERS=true) or in test environments
    pub scheduler: Option<Arc<tokio::sync::Mutex<crate::scheduler::Scheduler>>>,
    /// Read progress batching service for efficient page view tracking
    /// Batches progress updates in memory and flushes periodically to reduce DB load
    pub read_progress_service: Arc<crate::services::ReadProgressService>,
    /// Auth tracking service for batched last_used/last_login timestamp updates
    /// Reduces DB load by batching API key usage and user login timestamps
    pub auth_tracking_service: Arc<crate::services::AuthTrackingService>,
    /// PDF page cache service for caching rendered PDF pages
    /// Reduces CPU load by caching expensive PDF page renders to disk
    pub pdf_page_cache: Arc<crate::services::PdfPageCache>,
    /// In-flight thumbnail request tracker to prevent thundering herd
    /// When multiple requests come in for the same uncached thumbnail,
    /// only the first generates it while others wait for the result
    pub inflight_thumbnails: Arc<crate::services::InflightThumbnailTracker>,
    /// User authentication cache to avoid hitting the database on every request
    /// Caches user permissions/role for 60 seconds to reduce DB load
    pub user_auth_cache: Arc<UserAuthCache>,
    /// Rate limiter service for API rate limiting
    /// None when rate limiting is disabled in config
    pub rate_limiter_service: Option<Arc<crate::services::RateLimiterService>>,
    /// Plugin manager for coordinating external plugin processes
    /// Manages plugin lifecycle, spawning, and request routing
    pub plugin_manager: Arc<crate::services::plugin::PluginManager>,
    /// Plugin metrics service for collecting plugin performance data
    /// Always available (in-memory only, no persistence)
    pub plugin_metrics_service: Arc<crate::services::PluginMetricsService>,
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

    // OPTIMIZATION: Check cache first to avoid DB query on every request
    // This significantly reduces DB load when many requests come in from the same user
    // (e.g., loading a page with 30+ thumbnail requests)
    if let Some(cached) = state.user_auth_cache.get(&user_id) {
        if !cached.is_active {
            return Err(ApiError::Unauthorized(
                "User account is inactive".to_string(),
            ));
        }

        return Ok(AuthContext {
            user_id,
            username: cached.username,
            email: cached.email,
            role: cached.role,
            custom_permissions: cached.custom_permissions,
            auth_method: AuthMethod::Jwt,
            token_permissions: None,
        });
    }

    // Cache miss - load user from database
    let user = UserRepository::get_by_id(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load user: {}", e)))?
        .ok_or_else(|| ApiError::Unauthorized("User not found".to_string()))?;

    // Parse custom permissions from JSON (clone the value before consuming)
    let custom_permissions: Vec<Permission> = serde_json::from_value(user.permissions.clone())
        .map_err(|e| ApiError::Internal(format!("Failed to parse permissions: {}", e)))?;

    // Get role from user model
    let role = user.get_role();

    // Cache the user data for future requests
    state.user_auth_cache.insert(
        user_id,
        user.username.clone(),
        user.email.clone(),
        role,
        custom_permissions.clone(),
        user.is_active,
    );

    // Check if user is active (after caching, so we remember inactive users too)
    if !user.is_active {
        return Err(ApiError::Unauthorized(
            "User account is inactive".to_string(),
        ));
    }

    Ok(AuthContext {
        user_id,
        username: user.username,
        email: user.email,
        role,
        custom_permissions,
        auth_method: AuthMethod::Jwt,
        token_permissions: None,
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

    // Parse custom permissions from user (stored as JSON)
    let custom_permissions: Vec<Permission> = serde_json::from_value(user.permissions.clone())
        .map_err(|e| ApiError::Internal(format!("Failed to parse user permissions: {}", e)))?;

    // Parse token permissions from API key (stored as JSON)
    // These will be used to constrain the effective permissions
    let token_permissions: Vec<Permission> = serde_json::from_value(api_key_model.permissions)
        .map_err(|e| ApiError::Internal(format!("Failed to parse token permissions: {}", e)))?;

    // Get role from user model
    let role = user.get_role();

    // Record API key usage (batched, non-blocking)
    state
        .auth_tracking_service
        .record_api_key_used(api_key_model.id);

    Ok(AuthContext {
        user_id: user.id,
        username: user.username,
        email: user.email,
        role,
        custom_permissions,
        auth_method: AuthMethod::ApiKey,
        token_permissions: Some(token_permissions),
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

    // Parse custom permissions from user model (clone before consuming)
    let custom_permissions: Vec<Permission> = serde_json::from_value(user.permissions.clone())
        .map_err(|e| ApiError::Internal(format!("Failed to parse permissions: {}", e)))?;

    // Get role from user model
    let role = user.get_role();

    // Record user login (batched, non-blocking)
    state.auth_tracking_service.record_user_login(user.id);

    Ok(AuthContext {
        user_id: user.id,
        username: user.username,
        email: user.email,
        role,
        custom_permissions,
        auth_method: AuthMethod::BasicAuth,
        token_permissions: None,
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
