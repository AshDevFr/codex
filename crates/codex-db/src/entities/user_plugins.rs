//! User Plugin entity for per-user plugin instances
//!
//! This entity links users to plugins they've enabled, storing per-user
//! credentials (encrypted OAuth tokens, API keys), configuration overrides,
//! and external identity information.
//!
//! ## Key Features
//!
//! - **Per-user credentials**: Encrypted OAuth tokens or API keys per user
//! - **OAuth integration**: Access/refresh tokens, expiry tracking, external identity
//! - **Health tracking**: Per-user failure count and health status
//! - **Sync state**: Last sync timestamp for sync provider plugins
//!
//! ## Lifecycle
//!
//! 1. Admin installs a user-type plugin (in `plugins` table)
//! 2. User enables the plugin (creates `user_plugins` row)
//! 3. User connects via OAuth or provides API key
//! 4. User can disable/disconnect independently of other users

#![allow(dead_code)]

use chrono::{DateTime, Duration, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::plugins::PluginHealthStatus;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "user_plugins")]
pub struct Model {
    /// Unique identifier for this user-plugin instance
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    /// Reference to the plugin definition
    pub plugin_id: Uuid,

    /// The user who enabled this plugin
    pub user_id: Uuid,

    /// Encrypted per-user credentials (simple API keys/tokens)
    #[serde(skip_serializing)]
    pub credentials: Option<Vec<u8>>,

    /// Per-user configuration overrides (merged with plugin defaults)
    pub config: serde_json::Value,

    /// Encrypted OAuth access token
    #[serde(skip_serializing)]
    pub oauth_access_token: Option<Vec<u8>>,

    /// Encrypted OAuth refresh token
    #[serde(skip_serializing)]
    pub oauth_refresh_token: Option<Vec<u8>>,

    /// When the OAuth access token expires
    pub oauth_expires_at: Option<DateTime<Utc>>,

    /// OAuth scopes granted by the user
    pub oauth_scope: Option<String>,

    /// External user ID from the connected service (e.g., AniList user ID)
    pub external_user_id: Option<String>,

    /// External username for display (e.g., "@username" on AniList)
    pub external_username: Option<String>,

    /// External avatar URL for display
    pub external_avatar_url: Option<String>,

    /// Whether this user-plugin instance is enabled
    pub enabled: bool,

    /// Current health status: "unknown", "healthy", "degraded", "unhealthy", "disabled"
    pub health_status: String,

    /// Number of consecutive failures
    pub failure_count: i32,

    /// When the last failure occurred
    pub last_failure_at: Option<DateTime<Utc>>,

    /// When the last successful operation occurred
    pub last_success_at: Option<DateTime<Utc>>,

    /// When the last sync operation completed
    pub last_sync_at: Option<DateTime<Utc>>,

    /// When this user-plugin instance was created
    pub created_at: DateTime<Utc>,

    /// When this user-plugin instance was last updated
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::plugins::Entity",
        from = "Column::PluginId",
        to = "super::plugins::Column::Id",
        on_delete = "Cascade"
    )]
    Plugin,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id",
        on_delete = "Cascade"
    )]
    User,
    #[sea_orm(has_many = "super::user_plugin_data::Entity")]
    UserPluginData,
}

impl Related<super::plugins::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Plugin.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::user_plugin_data::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserPluginData.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// =============================================================================
// Helper Methods
// =============================================================================

impl Model {
    /// Check if the OAuth access token has expired
    pub fn is_oauth_expired(&self) -> bool {
        match self.oauth_expires_at {
            Some(expires_at) => Utc::now() >= expires_at,
            None => false, // No expiry means no OAuth or non-expiring token
        }
    }

    /// Check if the OAuth token needs refreshing (within 5 minutes of expiry)
    pub fn needs_token_refresh(&self) -> bool {
        match self.oauth_expires_at {
            Some(expires_at) => {
                let refresh_buffer = Duration::minutes(5);
                Utc::now() >= (expires_at - refresh_buffer)
            }
            None => false,
        }
    }

    /// Check if this instance has OAuth tokens configured
    pub fn has_oauth_tokens(&self) -> bool {
        self.oauth_access_token.is_some()
    }

    /// Check if this instance has simple credentials configured
    pub fn has_credentials(&self) -> bool {
        self.credentials.is_some()
    }

    /// Check if the instance has any form of authentication configured
    pub fn is_authenticated(&self) -> bool {
        self.has_oauth_tokens() || self.has_credentials()
    }

    /// Parse health status
    pub fn health_status_type(&self) -> PluginHealthStatus {
        self.health_status
            .parse()
            .unwrap_or(PluginHealthStatus::Unknown)
    }

    /// Check if the instance is in a healthy state
    pub fn is_healthy(&self) -> bool {
        self.enabled
            && matches!(
                self.health_status_type(),
                PluginHealthStatus::Healthy | PluginHealthStatus::Unknown
            )
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_model() -> Model {
        Model {
            id: Uuid::new_v4(),
            plugin_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            credentials: None,
            config: serde_json::json!({}),
            oauth_access_token: None,
            oauth_refresh_token: None,
            oauth_expires_at: None,
            oauth_scope: None,
            external_user_id: None,
            external_username: None,
            external_avatar_url: None,
            enabled: true,
            health_status: "unknown".to_string(),
            failure_count: 0,
            last_failure_at: None,
            last_success_at: None,
            last_sync_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_is_oauth_expired_no_expiry() {
        let model = test_model();
        assert!(!model.is_oauth_expired());
    }

    #[test]
    fn test_is_oauth_expired_future() {
        let mut model = test_model();
        model.oauth_expires_at = Some(Utc::now() + Duration::hours(1));
        assert!(!model.is_oauth_expired());
    }

    #[test]
    fn test_is_oauth_expired_past() {
        let mut model = test_model();
        model.oauth_expires_at = Some(Utc::now() - Duration::hours(1));
        assert!(model.is_oauth_expired());
    }

    #[test]
    fn test_needs_token_refresh_no_expiry() {
        let model = test_model();
        assert!(!model.needs_token_refresh());
    }

    #[test]
    fn test_needs_token_refresh_far_future() {
        let mut model = test_model();
        model.oauth_expires_at = Some(Utc::now() + Duration::hours(1));
        assert!(!model.needs_token_refresh());
    }

    #[test]
    fn test_needs_token_refresh_within_buffer() {
        let mut model = test_model();
        model.oauth_expires_at = Some(Utc::now() + Duration::minutes(3));
        assert!(model.needs_token_refresh());
    }

    #[test]
    fn test_has_oauth_tokens() {
        let mut model = test_model();
        assert!(!model.has_oauth_tokens());

        model.oauth_access_token = Some(vec![1, 2, 3]);
        assert!(model.has_oauth_tokens());
    }

    #[test]
    fn test_has_credentials() {
        let mut model = test_model();
        assert!(!model.has_credentials());

        model.credentials = Some(vec![1, 2, 3]);
        assert!(model.has_credentials());
    }

    #[test]
    fn test_is_authenticated() {
        let mut model = test_model();
        assert!(!model.is_authenticated());

        model.credentials = Some(vec![1, 2, 3]);
        assert!(model.is_authenticated());

        model.credentials = None;
        model.oauth_access_token = Some(vec![4, 5, 6]);
        assert!(model.is_authenticated());
    }

    #[test]
    fn test_health_status_type() {
        let mut model = test_model();
        assert_eq!(model.health_status_type(), PluginHealthStatus::Unknown);

        model.health_status = "healthy".to_string();
        assert_eq!(model.health_status_type(), PluginHealthStatus::Healthy);

        model.health_status = "unhealthy".to_string();
        assert_eq!(model.health_status_type(), PluginHealthStatus::Unhealthy);
    }

    #[test]
    fn test_is_healthy() {
        let mut model = test_model();
        assert!(model.is_healthy()); // enabled + unknown = healthy

        model.health_status = "healthy".to_string();
        assert!(model.is_healthy());

        model.health_status = "unhealthy".to_string();
        assert!(!model.is_healthy());

        model.health_status = "healthy".to_string();
        model.enabled = false;
        assert!(!model.is_healthy());
    }
}
