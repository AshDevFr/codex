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
