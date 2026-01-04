use crate::api::{
    dto::{LoginRequest, LoginResponse, UserInfo},
    error::ApiError,
    extractors::{AuthContext, AuthState},
};
use crate::db::repositories::UserRepository;
use crate::utils::password;
use axum::{extract::State, Json};
use std::sync::Arc;

/// Login handler
///
/// Authenticates a user with username/email and password, returns JWT token
#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = LoginResponse),
        (status = 401, description = "Invalid credentials"),
    ),
    tag = "auth"
)]
pub async fn login(
    State(state): State<Arc<AuthState>>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    // Try to find user by username first
    let user = match UserRepository::get_by_username(&state.db, &request.username).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            // If not found by username, try by email
            UserRepository::get_by_email(&state.db, &request.username)
                .await
                .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
                .ok_or_else(|| ApiError::Unauthorized("Invalid credentials".to_string()))?
        }
        Err(e) => return Err(ApiError::Internal(format!("Database error: {}", e))),
    };

    // Check if user is active
    if !user.is_active {
        return Err(ApiError::Unauthorized(
            "Account is inactive".to_string(),
        ));
    }

    // Verify password
    let password_valid = password::verify_password(&request.password, &user.password_hash)
        .map_err(|e| ApiError::Internal(format!("Password verification error: {}", e)))?;

    if !password_valid {
        return Err(ApiError::Unauthorized("Invalid credentials".to_string()));
    }

    // Generate JWT token
    let access_token = state
        .jwt_service
        .generate_token(user.id, user.username.clone(), user.is_admin)
        .map_err(|e| ApiError::Internal(format!("Failed to generate token: {}", e)))?;

    // Update last login timestamp
    UserRepository::update_last_login(&state.db, user.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update last login: {}", e)))?;

    // Build response
    let response = LoginResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: 24 * 3600, // 24 hours in seconds
        user: UserInfo {
            id: user.id,
            username: user.username,
            email: user.email,
            is_admin: user.is_admin,
        },
    };

    Ok(Json(response))
}

/// Logout handler
///
/// No-op for stateless JWT - client should discard token
#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    responses(
        (status = 200, description = "Logout successful"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "auth"
)]
pub async fn logout(_auth: AuthContext) -> Result<Json<serde_json::Value>, ApiError> {
    // For stateless JWT, logout is handled client-side by discarding the token
    // In the future, we could implement token blacklisting or refresh token revocation
    Ok(Json(serde_json::json!({
        "message": "Logged out successfully"
    })))
}
