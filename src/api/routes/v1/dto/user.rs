use super::common::{DEFAULT_PAGE, DEFAULT_PAGE_SIZE};
use super::sharing_tag::UserSharingTagGrantDto;
use crate::api::permissions::UserRole;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// User data transfer object
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserDto {
    /// Unique user identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: uuid::Uuid,

    /// Username for login
    #[schema(example = "johndoe")]
    pub username: String,

    /// User email address
    #[schema(example = "john.doe@example.com")]
    pub email: String,

    /// User role (reader, maintainer, admin)
    #[schema(example = "reader")]
    pub role: UserRole,

    /// Custom permissions that extend the role's base permissions
    pub permissions: Vec<String>,

    /// Whether the account is active
    #[schema(example = true)]
    pub is_active: bool,

    /// Timestamp of last login
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub last_login_at: Option<DateTime<Utc>>,

    /// Account creation timestamp
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// Last account update timestamp
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// User detail DTO with sharing tag grants (for single user view)
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserDetailDto {
    /// Unique user identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: uuid::Uuid,

    /// Username for login
    #[schema(example = "johndoe")]
    pub username: String,

    /// User email address
    #[schema(example = "john.doe@example.com")]
    pub email: String,

    /// User role (reader, maintainer, admin)
    #[schema(example = "reader")]
    pub role: UserRole,

    /// Custom permissions that extend the role's base permissions
    pub permissions: Vec<String>,

    /// Whether the account is active
    #[schema(example = true)]
    pub is_active: bool,

    /// Timestamp of last login
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub last_login_at: Option<DateTime<Utc>>,

    /// Account creation timestamp
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// Last account update timestamp
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,

    /// Sharing tag grants for this user
    pub sharing_tags: Vec<UserSharingTagGrantDto>,
}

/// Create user request
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserRequest {
    /// Username for the new account
    #[schema(example = "newuser")]
    pub username: String,

    /// Email address for the new account
    #[schema(example = "newuser@example.com")]
    pub email: String,

    /// Password for the new account
    #[schema(example = "securePassword123!")]
    pub password: String,

    /// User role (reader, maintainer, admin). Defaults to reader if not specified.
    #[schema(example = "reader")]
    #[serde(default)]
    pub role: Option<UserRole>,
}

/// Update user request
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserRequest {
    /// New username
    #[schema(example = "updateduser")]
    pub username: Option<String>,

    /// New email address
    #[schema(example = "updated@example.com")]
    pub email: Option<String>,

    /// New password
    #[schema(example = "newSecurePassword123!")]
    pub password: Option<String>,

    /// Update user role (reader, maintainer, admin)
    #[schema(example = "reader")]
    pub role: Option<UserRole>,

    /// Update active status
    #[schema(example = true)]
    pub is_active: Option<bool>,

    /// Custom permissions that extend the role's base permissions (admin only)
    /// These permissions are unioned with the role's permissions
    #[serde(default)]
    pub permissions: Option<Vec<String>>,
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_page_size() -> u64 {
    DEFAULT_PAGE_SIZE
}

/// Query parameters for listing users with filtering and pagination
#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct UserListParams {
    /// Filter by role
    #[serde(default)]
    pub role: Option<UserRole>,

    /// Filter by sharing tag name (users who have a grant for this tag)
    #[serde(default)]
    pub sharing_tag: Option<String>,

    /// Filter by sharing tag access mode (allow/deny) - only used with sharing_tag
    #[serde(default)]
    pub sharing_tag_mode: Option<String>,

    /// Page number (1-indexed, default 1)
    #[serde(default = "default_page")]
    pub page: u64,

    /// Number of items per page (max 100, default 50)
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

impl Default for UserListParams {
    fn default() -> Self {
        Self {
            role: None,
            sharing_tag: None,
            sharing_tag_mode: None,
            page: DEFAULT_PAGE,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }
}

impl UserListParams {
    /// Validate and clamp pagination parameters (1-indexed)
    pub fn validate(mut self, max_page_size: u64) -> Self {
        // Treat page 0 as page 1 for backward compatibility
        if self.page == 0 {
            self.page = 1;
        }
        if self.page_size == 0 {
            self.page_size = DEFAULT_PAGE_SIZE;
        }
        if self.page_size > max_page_size {
            self.page_size = max_page_size;
        }
        self
    }

    /// Calculate offset for database queries (converts 1-indexed page to 0-indexed offset)
    pub fn offset(&self) -> u64 {
        self.page.saturating_sub(1) * self.page_size
    }

    /// Get limit for database queries
    pub fn limit(&self) -> u64 {
        self.page_size
    }
}
