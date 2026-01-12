use crate::api::{
    dto::{
        ConfigureSettingsRequest, ConfigureSettingsResponse, InitializeSetupRequest,
        InitializeSetupResponse, SetupStatusResponse, UserInfo,
    },
    error::ApiError,
    extractors::{AuthContext, AuthState},
    handlers::auth::build_auth_cookie,
};
use crate::db::{entities::users, repositories::UserRepository};
use crate::utils::password;
use axum::{
    extract::State,
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

/// Check if initial setup is required
///
/// Returns whether the application needs initial setup (no users exist)
#[utoipa::path(
    get,
    path = "/api/v1/setup/status",
    responses(
        (status = 200, description = "Setup status", body = SetupStatusResponse),
    ),
    tag = "setup"
)]
pub async fn setup_status(
    State(state): State<Arc<AuthState>>,
) -> Result<Json<SetupStatusResponse>, ApiError> {
    let has_users = UserRepository::has_any_users(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    Ok(Json(SetupStatusResponse {
        setup_required: !has_users,
        has_users,
    }))
}

/// Initialize application setup by creating the first admin user
///
/// Creates the first admin user with email verification bypassed and returns a JWT token
#[utoipa::path(
    post,
    path = "/api/v1/setup/initialize",
    request_body = InitializeSetupRequest,
    responses(
        (status = 200, description = "Setup initialized", body = InitializeSetupResponse),
        (status = 400, description = "Invalid request or setup already completed"),
        (status = 422, description = "Validation error"),
    ),
    tag = "setup"
)]
pub async fn initialize_setup(
    State(state): State<Arc<AuthState>>,
    Json(request): Json<InitializeSetupRequest>,
) -> Result<Response, ApiError> {
    // Check if setup is still needed
    let has_users = UserRepository::has_any_users(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    if has_users {
        return Err(ApiError::BadRequest(
            "Setup already completed. Users already exist in the database.".to_string(),
        ));
    }

    // Validate input
    if request.username.trim().is_empty() {
        return Err(ApiError::BadRequest("Username cannot be empty".to_string()));
    }
    if request.email.trim().is_empty() {
        return Err(ApiError::BadRequest("Email cannot be empty".to_string()));
    }

    // Validate email format (basic validation)
    if !request.email.contains('@') || !request.email.contains('.') {
        return Err(ApiError::BadRequest(
            "Invalid email address format".to_string(),
        ));
    }
    let parts: Vec<&str> = request.email.split('@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(ApiError::BadRequest(
            "Invalid email address format".to_string(),
        ));
    }
    if !parts[1].contains('.') {
        return Err(ApiError::BadRequest(
            "Invalid email address format".to_string(),
        ));
    }

    // Validate password complexity
    if request.password.len() < 8 {
        return Err(ApiError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }
    if !request.password.chars().any(|c| c.is_uppercase()) {
        return Err(ApiError::BadRequest(
            "Password must contain at least one uppercase letter".to_string(),
        ));
    }
    if !request.password.chars().any(|c| c.is_lowercase()) {
        return Err(ApiError::BadRequest(
            "Password must contain at least one lowercase letter".to_string(),
        ));
    }
    if !request.password.chars().any(|c| c.is_numeric()) {
        return Err(ApiError::BadRequest(
            "Password must contain at least one number".to_string(),
        ));
    }
    if !request
        .password
        .chars()
        .any(|c| "!@#$%^&*(),.?\":{}|<>".contains(c))
    {
        return Err(ApiError::BadRequest(
            "Password must contain at least one special character".to_string(),
        ));
    }

    // Hash password
    let password_hash = password::hash_password(&request.password)
        .map_err(|e| ApiError::Internal(format!("Password hashing error: {}", e)))?;

    // Create first admin user with full permissions
    use crate::api::permissions::{serialize_permissions, ADMIN_PERMISSIONS};
    let permissions_json = serialize_permissions(&ADMIN_PERMISSIONS);

    let new_user = users::Model {
        id: Uuid::new_v4(),
        username: request.username.clone(),
        email: request.email.clone(),
        password_hash,
        is_admin: true,       // First user is always admin
        is_active: true,      // Active by default for first user
        email_verified: true, // Bypass email verification for first user
        permissions: serde_json::from_str(&permissions_json)
            .unwrap_or_else(|_| serde_json::json!([])),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_login_at: None,
    };

    // Save user to database
    let created_user = UserRepository::create(&state.db, &new_user)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create user: {}", e)))?;

    // Generate JWT token for automatic login
    let access_token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .map_err(|e| ApiError::Internal(format!("Failed to generate token: {}", e)))?;

    // Update last login timestamp
    UserRepository::update_last_login(&state.db, created_user.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update last login: {}", e)))?;

    // Build response
    let response = InitializeSetupResponse {
        user: UserInfo {
            id: created_user.id,
            username: created_user.username,
            email: created_user.email,
            is_admin: created_user.is_admin,
            email_verified: created_user.email_verified,
        },
        access_token: access_token.clone(),
        token_type: "Bearer".to_string(),
        expires_in: 24 * 3600, // 24 hours in seconds
        message: "Setup completed successfully. Welcome to Codex!".to_string(),
    };

    // Create HTTP-only cookie for image authentication (same as login)
    let cookie = build_auth_cookie(&access_token, 24 * 3600);

    // Build response with cookie
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        cookie
            .parse()
            .map_err(|_| ApiError::Internal("Failed to create cookie header".to_string()))?,
    );

    Ok((headers, Json(response)).into_response())
}

/// Configure initial settings (optional step in setup wizard)
///
/// Allows the newly created admin to configure database settings
#[utoipa::path(
    patch,
    path = "/api/v1/setup/settings",
    request_body = ConfigureSettingsRequest,
    responses(
        (status = 200, description = "Settings configured", body = ConfigureSettingsResponse),
        (status = 403, description = "Forbidden - Admin only"),
    ),
    security(
        ("jwt_bearer" = [])
    ),
    tag = "setup"
)]
pub async fn configure_initial_settings(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<ConfigureSettingsRequest>,
) -> Result<Json<ConfigureSettingsResponse>, ApiError> {
    // Ensure user is admin
    if !auth.is_admin {
        return Err(ApiError::Forbidden(
            "Only administrators can configure settings".to_string(),
        ));
    }

    // If skipping configuration, return early
    if request.skip_configuration {
        return Ok(Json(ConfigureSettingsResponse {
            message: "Settings configuration skipped. Using default values.".to_string(),
            settings_configured: 0,
        }));
    }

    // Import SettingsRepository to update settings
    use crate::db::repositories::SettingsRepository;

    let mut configured_count = 0;

    // Update each setting
    for (key, value) in request.settings {
        match SettingsRepository::set(
            &state.db,
            &key,
            value,
            auth.user_id,
            Some("Initial setup configuration".to_string()),
            None, // IP address
        )
        .await
        {
            Ok(_) => configured_count += 1,
            Err(e) => {
                // Log error but continue with other settings
                eprintln!("Failed to set setting {}: {}", key, e);
            }
        }
    }

    Ok(Json(ConfigureSettingsResponse {
        message: format!("Successfully configured {} settings", configured_count),
        settings_configured: configured_count,
    }))
}
