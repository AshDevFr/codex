use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// User data transfer object
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserDto {
    pub id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub is_admin: bool,
    pub is_active: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Create user request
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserRequest {
    /// Username
    pub username: String,

    /// Email address
    pub email: String,

    /// Password
    pub password: String,

    /// Admin flag
    #[serde(default)]
    pub is_admin: bool,
}

/// Update user request
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserRequest {
    /// Username
    pub username: Option<String>,

    /// Email address
    pub email: Option<String>,

    /// New password
    pub password: Option<String>,

    /// Admin flag
    pub is_admin: Option<bool>,

    /// Active status
    pub is_active: Option<bool>,
}
