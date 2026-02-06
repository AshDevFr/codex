//! User Plugin DTOs
//!
//! Request and response types for user plugin management endpoints.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// OAuth initiation response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthStartResponse {
    /// The URL to redirect the user to for OAuth authorization
    #[schema(
        example = "https://anilist.co/api/v2/oauth/authorize?response_type=code&client_id=..."
    )]
    pub redirect_url: String,
}

/// OAuth callback query parameters
#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    /// Authorization code from the OAuth provider
    pub code: String,
    /// State parameter for CSRF protection
    pub state: String,
}

/// User plugin instance status
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserPluginDto {
    /// User plugin instance ID
    pub id: Uuid,
    /// Plugin definition ID
    pub plugin_id: Uuid,
    /// Plugin display name
    pub plugin_name: String,
    /// Plugin display name for UI
    pub plugin_display_name: String,
    /// Plugin type: "system" or "user"
    pub plugin_type: String,
    /// Whether the user has enabled this plugin
    pub enabled: bool,
    /// Whether the plugin is connected (has valid credentials/OAuth)
    pub connected: bool,
    /// Health status of this user's plugin instance
    pub health_status: String,
    /// External service username (if connected via OAuth)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_username: Option<String>,
    /// External service avatar URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_avatar_url: Option<String>,
    /// Last sync timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_at: Option<DateTime<Utc>>,
    /// Last successful operation timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_success_at: Option<DateTime<Utc>>,
    /// Whether this plugin requires OAuth authentication
    pub requires_oauth: bool,
    /// User-facing description of the plugin
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Per-user configuration
    pub config: serde_json::Value,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

/// Available plugin (not yet enabled by user)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AvailablePluginDto {
    /// Plugin definition ID
    pub plugin_id: Uuid,
    /// Plugin name
    pub name: String,
    /// Plugin display name
    pub display_name: String,
    /// Plugin description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this plugin requires OAuth authentication
    pub requires_oauth: bool,
    /// Plugin capabilities
    pub capabilities: UserPluginCapabilitiesDto,
}

/// Plugin capabilities for display (user plugin context)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserPluginCapabilitiesDto {
    /// Can sync reading progress
    pub sync_provider: bool,
    /// Can provide recommendations
    pub recommendation_provider: bool,
}

/// User plugins list response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserPluginsListResponse {
    /// Plugins the user has enabled
    pub enabled: Vec<UserPluginDto>,
    /// Plugins available for the user to enable
    pub available: Vec<AvailablePluginDto>,
}
