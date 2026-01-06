use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Login request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    /// Username or email
    pub username: String,

    /// Password
    pub password: String,
}

/// Login response with JWT token
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    /// JWT access token
    pub access_token: String,

    /// Token type (always "Bearer")
    pub token_type: String,

    /// Token expiry in seconds
    pub expires_in: u64,

    /// User information
    pub user: UserInfo,
}

/// User information in login response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub is_admin: bool,
    pub email_verified: bool,
}

/// Token response (for refresh tokens in future)
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenResponse {
    /// JWT access token
    pub access_token: String,

    /// Token type (always "Bearer")
    pub token_type: String,

    /// Token expiry in seconds
    pub expires_in: u64,
}

/// Register request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    /// Username
    pub username: String,

    /// Email address
    pub email: String,

    /// Password
    pub password: String,
}

/// Register response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisterResponse {
    /// JWT access token (if email confirmation not required)
    pub access_token: Option<String>,

    /// Token type (always "Bearer")
    pub token_type: Option<String>,

    /// Token expiry in seconds
    pub expires_in: Option<u64>,

    /// User information
    pub user: UserInfo,

    /// Message about email verification if required
    pub message: Option<String>,
}

/// Verify email request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerifyEmailRequest {
    /// Verification token from email
    pub token: String,
}

/// Verify email response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerifyEmailResponse {
    /// Success message
    pub message: String,

    /// JWT access token (user can now login)
    pub access_token: String,

    /// Token type (always "Bearer")
    pub token_type: String,

    /// Token expiry in seconds
    pub expires_in: u64,

    /// User information
    pub user: UserInfo,
}

/// Resend verification email request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResendVerificationRequest {
    /// Email address
    pub email: String,
}

/// Resend verification email response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResendVerificationResponse {
    /// Success message
    pub message: String,
}
