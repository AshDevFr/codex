use crate::api::{
    dto::{
        LoginRequest, LoginResponse, RegisterRequest, RegisterResponse, ResendVerificationRequest,
        ResendVerificationResponse, UserInfo, VerifyEmailRequest, VerifyEmailResponse,
    },
    error::ApiError,
    extractors::{AuthContext, AuthState},
};
use crate::db::{
    entities::users,
    repositories::{EmailVerificationTokenRepository, UserRepository},
};
use crate::utils::password;
use axum::{extract::State, Json};
use chrono::Utc;
use sea_orm::ActiveModelTrait;
use sea_orm::Set;
use std::sync::Arc;
use uuid::Uuid;

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
        return Err(ApiError::Unauthorized("Account is inactive".to_string()));
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
            email_verified: user.email_verified,
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

/// Register handler
///
/// Creates a new user account with username, email, and password
#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered successfully", body = RegisterResponse),
        (status = 400, description = "Invalid request or user already exists"),
        (status = 422, description = "Validation error"),
    ),
    tag = "auth"
)]
pub async fn register(
    State(state): State<Arc<AuthState>>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, ApiError> {
    // Validate input
    if request.username.trim().is_empty() {
        return Err(ApiError::BadRequest("Username cannot be empty".to_string()));
    }
    if request.email.trim().is_empty() {
        return Err(ApiError::BadRequest("Email cannot be empty".to_string()));
    }
    if request.password.len() < 8 {
        return Err(ApiError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    // Check if username already exists
    if let Some(_existing) = UserRepository::get_by_username(&state.db, &request.username)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
    {
        return Err(ApiError::BadRequest("Username already exists".to_string()));
    }

    // Check if email already exists
    if let Some(_existing) = UserRepository::get_by_email(&state.db, &request.email)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
    {
        return Err(ApiError::BadRequest("Email already exists".to_string()));
    }

    // Hash password
    let password_hash = password::hash_password(&request.password)
        .map_err(|e| ApiError::Internal(format!("Password hashing error: {}", e)))?;

    // Determine if user should be active and email verified based on config
    let email_confirmation_required = state.auth_config.email_confirmation_required;
    let is_active = !email_confirmation_required;
    let email_verified = !email_confirmation_required;

    // Create user with reader permissions by default
    use crate::api::permissions::{serialize_permissions, READER_PERMISSIONS};
    let permissions_json = serialize_permissions(&READER_PERMISSIONS);

    let new_user = users::Model {
        id: Uuid::new_v4(),
        username: request.username.clone(),
        email: request.email.clone(),
        password_hash,
        is_admin: false,
        is_active,
        email_verified,
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

    // Build response based on email confirmation requirement
    let response = if email_confirmation_required {
        // Create verification token
        let expiry_hours = state.email_service.config.verification_token_expiry_hours as i64;
        let token = EmailVerificationTokenRepository::create(&state.db, created_user.id, expiry_hours)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to create verification token: {}", e)))?;

        // Send verification email
        state
            .email_service
            .send_verification_email(&created_user.email, &created_user.username, &token.token)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to send verification email: {}", e)))?;

        // Email confirmation required - don't generate token yet
        RegisterResponse {
            access_token: None,
            token_type: None,
            expires_in: None,
            user: UserInfo {
                id: created_user.id,
                username: created_user.username,
                email: created_user.email,
                is_admin: created_user.is_admin,
                email_verified: created_user.email_verified,
            },
            message: Some(
                "Registration successful. Please verify your email to activate your account."
                    .to_string(),
            ),
        }
    } else {
        // No email confirmation required - generate token immediately
        let access_token = state
            .jwt_service
            .generate_token(
                created_user.id,
                created_user.username.clone(),
                created_user.is_admin,
            )
            .map_err(|e| ApiError::Internal(format!("Failed to generate token: {}", e)))?;

        RegisterResponse {
            access_token: Some(access_token),
            token_type: Some("Bearer".to_string()),
            expires_in: Some(24 * 3600), // 24 hours in seconds
            user: UserInfo {
                id: created_user.id,
                username: created_user.username,
                email: created_user.email,
                is_admin: created_user.is_admin,
                email_verified: created_user.email_verified,
            },
            message: Some("Registration successful. You are now logged in.".to_string()),
        }
    };

    Ok(Json(response))
}

/// Verify email handler
///
/// Verifies a user's email address using the token sent via email
#[utoipa::path(
    post,
    path = "/api/v1/auth/verify-email",
    request_body = VerifyEmailRequest,
    responses(
        (status = 200, description = "Email verified successfully", body = VerifyEmailResponse),
        (status = 400, description = "Invalid or expired token"),
    ),
    tag = "auth"
)]
pub async fn verify_email(
    State(state): State<Arc<AuthState>>,
    Json(request): Json<VerifyEmailRequest>,
) -> Result<Json<VerifyEmailResponse>, ApiError> {
    // Get the token from database
    let token_model = EmailVerificationTokenRepository::get_by_token(&state.db, &request.token)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::BadRequest("Invalid verification token".to_string()))?;

    // Check if token is expired
    if token_model.expires_at < Utc::now() {
        return Err(ApiError::BadRequest(
            "Verification token has expired".to_string(),
        ));
    }

    // Get the user
    let user = UserRepository::get_by_id(&state.db, token_model.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::BadRequest("User not found".to_string()))?;

    // Check if email is already verified
    if user.email_verified {
        return Err(ApiError::BadRequest(
            "Email already verified".to_string(),
        ));
    }

    // Update user: mark email as verified and activate account
    let mut active_user: users::ActiveModel = user.clone().into();
    active_user.email_verified = Set(true);
    active_user.is_active = Set(true);
    active_user.updated_at = Set(Utc::now());
    let updated_user = active_user
        .update(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update user: {}", e)))?;

    // Delete the used token
    EmailVerificationTokenRepository::delete_by_token(&state.db, &request.token)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete token: {}", e)))?;

    // Generate JWT token for the user
    let access_token = state
        .jwt_service
        .generate_token(updated_user.id, updated_user.username.clone(), updated_user.is_admin)
        .map_err(|e| ApiError::Internal(format!("Failed to generate token: {}", e)))?;

    // Build response
    let response = VerifyEmailResponse {
        message: "Email verified successfully. Your account is now active.".to_string(),
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: 24 * 3600, // 24 hours in seconds
        user: UserInfo {
            id: updated_user.id,
            username: updated_user.username,
            email: updated_user.email,
            is_admin: updated_user.is_admin,
            email_verified: updated_user.email_verified,
        },
    };

    Ok(Json(response))
}

/// Resend verification email handler
///
/// Resends the verification email to a user
#[utoipa::path(
    post,
    path = "/api/v1/auth/resend-verification",
    request_body = ResendVerificationRequest,
    responses(
        (status = 200, description = "Verification email sent", body = ResendVerificationResponse),
        (status = 400, description = "Invalid request or email already verified"),
    ),
    tag = "auth"
)]
pub async fn resend_verification(
    State(state): State<Arc<AuthState>>,
    Json(request): Json<ResendVerificationRequest>,
) -> Result<Json<ResendVerificationResponse>, ApiError> {
    // Get user by email
    let user = UserRepository::get_by_email(&state.db, &request.email)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| {
            ApiError::BadRequest("No account found with that email address".to_string())
        })?;

    // Check if email is already verified
    if user.email_verified {
        return Err(ApiError::BadRequest(
            "Email is already verified".to_string(),
        ));
    }

    // Delete any existing verification tokens for this user
    EmailVerificationTokenRepository::delete_by_user_id(&state.db, user.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete old tokens: {}", e)))?;

    // Create new verification token
    let expiry_hours = state.email_service.config.verification_token_expiry_hours as i64;
    let token = EmailVerificationTokenRepository::create(&state.db, user.id, expiry_hours)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create verification token: {}", e)))?;

    // Send verification email
    state
        .email_service
        .send_verification_email(&user.email, &user.username, &token.token)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to send verification email: {}", e)))?;

    let response = ResendVerificationResponse {
        message: "Verification email has been sent. Please check your inbox.".to_string(),
    };

    Ok(Json(response))
}
