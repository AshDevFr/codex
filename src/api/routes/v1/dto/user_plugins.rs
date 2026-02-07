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

/// Request to update user plugin configuration
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserPluginConfigRequest {
    /// Configuration overrides for this plugin
    pub config: serde_json::Value,
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

/// Response from triggering a sync operation
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncTriggerResponse {
    /// Task ID for tracking the sync operation
    pub task_id: Uuid,
    /// Human-readable status message
    pub message: String,
}

/// Query parameters for sync status endpoint
#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatusQuery {
    /// If true, spawn the plugin process and query live sync state
    /// (external count, pending push/pull, conflicts).
    /// Default: false (returns database-stored metadata only).
    #[serde(default)]
    pub live: bool,
}

/// Sync status response for a user plugin
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatusDto {
    /// Plugin ID
    pub plugin_id: Uuid,
    /// Plugin name
    pub plugin_name: String,
    /// Whether the plugin is connected and ready to sync
    pub connected: bool,
    /// Last successful sync timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_at: Option<DateTime<Utc>>,
    /// Last successful operation timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_success_at: Option<DateTime<Utc>>,
    /// Last failure timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure_at: Option<DateTime<Utc>>,
    /// Health status
    pub health_status: String,
    /// Number of consecutive failures
    pub failure_count: i32,
    /// Whether the plugin is currently enabled
    pub enabled: bool,
    /// Number of entries tracked on the external service (only with `?live=true`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_count: Option<u32>,
    /// Number of local entries that need to be pushed (only with `?live=true`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_push: Option<u32>,
    /// Number of external entries that need to be pulled (only with `?live=true`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_pull: Option<u32>,
    /// Number of entries with conflicts on both sides (only with `?live=true`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflicts: Option<u32>,
    /// Error message if `?live=true` was requested but the plugin could not be queried
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_status_dto_omits_live_fields_when_none() {
        let dto = SyncStatusDto {
            plugin_id: Uuid::new_v4(),
            plugin_name: "AniList".to_string(),
            connected: true,
            last_sync_at: None,
            last_success_at: None,
            last_failure_at: None,
            health_status: "healthy".to_string(),
            failure_count: 0,
            enabled: true,
            external_count: None,
            pending_push: None,
            pending_pull: None,
            conflicts: None,
            live_error: None,
        };
        let json = serde_json::to_value(&dto).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("externalCount"));
        assert!(!obj.contains_key("pendingPush"));
        assert!(!obj.contains_key("pendingPull"));
        assert!(!obj.contains_key("conflicts"));
        assert!(!obj.contains_key("liveError"));
    }

    #[test]
    fn test_sync_status_dto_includes_live_fields_when_present() {
        let dto = SyncStatusDto {
            plugin_id: Uuid::new_v4(),
            plugin_name: "AniList".to_string(),
            connected: true,
            last_sync_at: None,
            last_success_at: None,
            last_failure_at: None,
            health_status: "healthy".to_string(),
            failure_count: 0,
            enabled: true,
            external_count: Some(150),
            pending_push: Some(5),
            pending_pull: Some(3),
            conflicts: Some(1),
            live_error: None,
        };
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["externalCount"], 150);
        assert_eq!(json["pendingPush"], 5);
        assert_eq!(json["pendingPull"], 3);
        assert_eq!(json["conflicts"], 1);
        assert!(!json.as_object().unwrap().contains_key("liveError"));
    }

    #[test]
    fn test_sync_status_dto_includes_live_error() {
        let dto = SyncStatusDto {
            plugin_id: Uuid::new_v4(),
            plugin_name: "AniList".to_string(),
            connected: false,
            last_sync_at: None,
            last_success_at: None,
            last_failure_at: None,
            health_status: "unknown".to_string(),
            failure_count: 0,
            enabled: true,
            external_count: None,
            pending_push: None,
            pending_pull: None,
            conflicts: None,
            live_error: Some("Plugin unavailable: not found".to_string()),
        };
        let json = serde_json::to_value(&dto).unwrap();
        assert!(
            json["liveError"]
                .as_str()
                .unwrap()
                .contains("Plugin unavailable")
        );
        assert!(!json.as_object().unwrap().contains_key("externalCount"));
    }

    #[test]
    fn test_sync_status_query_defaults_to_false() {
        let query: SyncStatusQuery = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(!query.live);
    }

    #[test]
    fn test_sync_status_query_live_true() {
        let query: SyncStatusQuery =
            serde_json::from_value(serde_json::json!({"live": true})).unwrap();
        assert!(query.live);
    }
}
