use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Login request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    /// Username or email
    #[schema(example = "admin")]
    pub username: String,

    /// Password
    #[schema(example = "password123")]
    pub password: String,
}

/// Login response with JWT token
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    /// JWT access token
    #[schema(
        example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ"
    )]
    pub access_token: String,

    /// Token type (always "Bearer")
    #[schema(example = "Bearer")]
    pub token_type: String,

    /// Token expiry in seconds
    #[schema(example = 86400)]
    pub expires_in: u64,

    /// User information
    pub user: UserInfo,
}

/// User information in login response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    /// User unique identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: uuid::Uuid,

    /// Username
    #[schema(example = "admin")]
    pub username: String,

    /// Email address
    #[schema(example = "admin@example.com")]
    pub email: String,

    /// Whether user has admin privileges
    #[schema(example = true)]
    pub is_admin: bool,

    /// Whether email has been verified
    #[schema(example = true)]
    pub email_verified: bool,
}

/// Token response (for refresh tokens in future)
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenResponse {
    /// JWT access token
    #[schema(
        example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ"
    )]
    pub access_token: String,

    /// Token type (always "Bearer")
    #[schema(example = "Bearer")]
    pub token_type: String,

    /// Token expiry in seconds
    #[schema(example = 86400)]
    pub expires_in: u64,
}

/// Register request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    /// Username
    #[schema(example = "johndoe")]
    pub username: String,

    /// Email address
    #[schema(example = "john@example.com")]
    pub email: String,

    /// Password
    #[schema(example = "securePassword123!")]
    pub password: String,
}

/// Register response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisterResponse {
    /// JWT access token (if email confirmation not required)
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub access_token: Option<String>,

    /// Token type (always "Bearer")
    #[schema(example = "Bearer")]
    pub token_type: Option<String>,

    /// Token expiry in seconds
    #[schema(example = 86400)]
    pub expires_in: Option<u64>,

    /// User information
    pub user: UserInfo,

    /// Message about email verification if required
    #[schema(example = "Please check your email to verify your account")]
    pub message: Option<String>,
}

/// Verify email request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerifyEmailRequest {
    /// Verification token from email
    #[schema(example = "abc123def456")]
    pub token: String,
}

/// Verify email response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerifyEmailResponse {
    /// Success message
    #[schema(example = "Email verified successfully")]
    pub message: String,

    /// JWT access token (user can now login)
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub access_token: String,

    /// Token type (always "Bearer")
    #[schema(example = "Bearer")]
    pub token_type: String,

    /// Token expiry in seconds
    #[schema(example = 86400)]
    pub expires_in: u64,

    /// User information
    pub user: UserInfo,
}

/// Resend verification email request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResendVerificationRequest {
    /// Email address
    #[schema(example = "john@example.com")]
    pub email: String,
}

/// Resend verification email response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResendVerificationResponse {
    /// Success message
    #[schema(example = "Verification email sent")]
    pub message: String,
}
