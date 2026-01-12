//! User Integrations DTOs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::entities::user_integrations;

/// A user integration (credentials are never exposed)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserIntegrationDto {
    /// Integration ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// Integration name (e.g., "anilist", "myanimelist")
    #[schema(example = "anilist")]
    pub integration_name: String,

    /// User-defined display name
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "My AniList Account")]
    pub display_name: Option<String>,

    /// Whether the integration is connected (has credentials)
    #[schema(example = true)]
    pub connected: bool,

    /// Whether the integration is enabled
    #[schema(example = true)]
    pub enabled: bool,

    /// User preferences for this integration
    #[schema(example = json!({"sync_progress": true, "sync_ratings": true}))]
    pub settings: serde_json::Value,

    /// When the integration last synced
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "2024-01-15T18:00:00Z")]
    pub last_sync_at: Option<DateTime<Utc>>,

    /// Current sync status: idle, syncing, error, rate_limited
    #[schema(example = "idle")]
    pub sync_status: String,

    /// Error message if sync failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,

    /// External user ID from the provider
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "123456")]
    pub external_user_id: Option<String>,

    /// External username from the provider
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "myusername")]
    pub external_username: Option<String>,

    /// When the OAuth token expires
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_expires_at: Option<DateTime<Utc>>,

    /// When the integration was connected
    pub created_at: DateTime<Utc>,

    /// When the integration was last updated
    pub updated_at: DateTime<Utc>,
}

impl From<user_integrations::Model> for UserIntegrationDto {
    fn from(model: user_integrations::Model) -> Self {
        Self {
            id: model.id,
            integration_name: model.integration_name,
            display_name: model.display_name,
            connected: true, // If we have a model, it's connected
            enabled: model.enabled,
            settings: model.settings,
            last_sync_at: model.last_sync_at,
            sync_status: model.sync_status,
            last_error: model.last_error,
            external_user_id: model.external_user_id,
            external_username: model.external_username,
            token_expires_at: model.token_expires_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

/// Response containing a list of user integrations
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserIntegrationsListResponse {
    /// Connected integrations
    pub integrations: Vec<UserIntegrationDto>,

    /// Available integrations that user can connect
    pub available: Vec<AvailableIntegrationDto>,
}

/// An available integration that can be connected
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AvailableIntegrationDto {
    /// Integration name (e.g., "anilist", "myanimelist")
    #[schema(example = "anilist")]
    pub name: String,

    /// Human-readable display name
    #[schema(example = "AniList")]
    pub display_name: String,

    /// Description of the integration
    #[schema(example = "Sync your reading progress and ratings with AniList")]
    pub description: String,

    /// Authentication type: oauth2, api_key, none
    #[schema(example = "oauth2")]
    pub auth_type: String,

    /// Features supported by this integration
    #[schema(example = json!(["sync_progress", "sync_ratings", "import_lists"]))]
    pub features: Vec<String>,

    /// Whether this integration is already connected by the user
    #[schema(example = false)]
    pub connected: bool,
}

/// Request to initiate connection to an integration
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectIntegrationRequest {
    /// Integration name to connect
    #[schema(example = "anilist")]
    pub integration_name: String,

    /// Redirect URI for OAuth callback (required for OAuth integrations)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "https://app.example.com/integrations/callback")]
    pub redirect_uri: Option<String>,

    /// API key (for api_key auth type integrations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// Response from initiating integration connection
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectIntegrationResponse {
    /// OAuth authorization URL (redirect user here)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "https://anilist.co/api/v2/oauth/authorize?client_id=...")]
    pub auth_url: Option<String>,

    /// Whether the integration is now connected (true for api_key auth)
    #[schema(example = false)]
    pub connected: bool,

    /// The integration details (if connected immediately)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integration: Option<UserIntegrationDto>,
}

/// OAuth callback request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthCallbackRequest {
    /// OAuth authorization code
    #[schema(example = "abc123")]
    pub code: String,

    /// State parameter for CSRF protection
    #[schema(example = "random-state-string")]
    pub state: String,

    /// Redirect URI used in the authorization request
    #[schema(example = "https://app.example.com/integrations/callback")]
    pub redirect_uri: String,
}

/// Request to update integration settings
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIntegrationSettingsRequest {
    /// Updated display name
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "My AniList")]
    pub display_name: Option<String>,

    /// Enable or disable the integration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    /// Updated settings
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!({"sync_progress": true, "sync_ratings": false}))]
    pub settings: Option<serde_json::Value>,
}

/// Response from triggering a sync
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncTriggerResponse {
    /// Whether the sync was started
    #[schema(example = true)]
    pub started: bool,

    /// Status message
    #[schema(example = "Sync started")]
    pub message: String,

    /// Updated integration state
    pub integration: UserIntegrationDto,
}

/// Integration sync status
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationSyncStatus {
    /// Current sync status
    #[schema(example = "syncing")]
    pub status: String,

    /// Progress percentage (0-100) if available
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 45)]
    pub progress: Option<u8>,

    /// Status message
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Syncing reading progress...")]
    pub message: Option<String>,

    /// Last sync timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_at: Option<DateTime<Utc>>,

    /// Last error message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}
