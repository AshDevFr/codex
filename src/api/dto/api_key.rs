use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

/// API key data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub permissions: Value,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// API key creation response (includes plaintext key only on creation)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiKeyResponse {
    #[serde(flatten)]
    pub api_key: ApiKeyDto,
    /// The plaintext API key (only shown once on creation)
    pub key: String,
}

/// Create API key request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiKeyRequest {
    /// Name/description for the API key
    pub name: String,

    /// Permissions for the API key (array of permission strings)
    /// If not provided, uses the user's current permissions
    #[serde(default)]
    pub permissions: Option<Vec<String>>,

    /// Optional expiration date
    pub expires_at: Option<DateTime<Utc>>,
}

/// Update API key request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApiKeyRequest {
    /// Name/description for the API key
    pub name: Option<String>,

    /// Permissions for the API key (array of permission strings)
    pub permissions: Option<Vec<String>>,

    /// Active status
    pub is_active: Option<bool>,

    /// Optional expiration date
    pub expires_at: Option<DateTime<Utc>>,
}
