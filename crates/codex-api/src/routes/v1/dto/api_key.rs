use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

/// API key data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyDto {
    /// Unique API key identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// Owner user ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub user_id: Uuid,

    /// Human-readable name for the key
    #[schema(example = "Mobile App Key")]
    pub name: String,

    /// Prefix of the key for identification
    #[schema(example = "cdx_a1b2c3")]
    pub key_prefix: String,

    /// Permissions granted to this key
    pub permissions: Value,

    /// Whether the key is currently active
    #[schema(example = true)]
    pub is_active: bool,

    /// When the key expires (if set)
    #[schema(example = "2025-12-31T23:59:59Z")]
    pub expires_at: Option<DateTime<Utc>>,

    /// When the key was last used
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub last_used_at: Option<DateTime<Utc>>,

    /// When the key was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the key was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
}

/// API key creation response (includes plaintext key only on creation)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiKeyResponse {
    #[serde(flatten)]
    pub api_key: ApiKeyDto,

    /// The plaintext API key (only shown once on creation)
    #[schema(example = "cdx_a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6")]
    pub key: String,
}

/// Create API key request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiKeyRequest {
    /// Name/description for the API key
    #[schema(example = "Mobile App Key")]
    pub name: String,

    /// Permissions for the API key (array of permission strings)
    /// If not provided, uses the user's current permissions
    #[serde(default)]
    pub permissions: Option<Vec<String>>,

    /// Optional expiration date
    #[schema(example = "2025-12-31T23:59:59Z")]
    pub expires_at: Option<DateTime<Utc>>,
}

/// Update API key request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApiKeyRequest {
    /// Name/description for the API key
    #[schema(example = "Updated Key Name")]
    pub name: Option<String>,

    /// Permissions for the API key (array of permission strings)
    pub permissions: Option<Vec<String>>,

    /// Active status
    #[schema(example = true)]
    pub is_active: Option<bool>,

    /// Optional expiration date
    #[schema(example = "2025-12-31T23:59:59Z")]
    pub expires_at: Option<DateTime<Utc>>,
}
