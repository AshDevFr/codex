//! Plugin entity for external metadata provider processes
//!
//! Plugins are external processes that communicate with Codex via JSON-RPC over stdio.
//! This entity stores plugin configuration, RBAC permissions, scopes, and health status.
//!
//! ## Key Features
//!
//! - **Execution**: Command, args, env, working directory for spawning plugin process
//! - **RBAC Permissions**: Controls what metadata fields a plugin can write
//! - **Scopes**: Defines where the plugin can be invoked (series:detail, series:bulk, etc.)
//! - **Credentials**: Encrypted storage for API keys and tokens
//! - **Health Tracking**: Failure count, auto-disable on repeated failures
//!
//! TODO: Remove allow(dead_code) once plugin features are fully implemented

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "plugins")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// Unique identifier (e.g., "mangabaka")
    pub name: String,
    /// Display name for UI (e.g., "MangaBaka")
    pub display_name: String,
    /// Description of the plugin
    pub description: Option<String>,
    /// Plugin type: "system" (admin-configured) or "user" (per-user instances)
    pub plugin_type: String,

    // Execution
    /// Command to spawn the plugin (e.g., "node", "python", "/path/to/binary")
    pub command: String,
    /// Command arguments as JSON array (e.g., ["/opt/codex/plugins/mangabaka/dist/index.js"])
    pub args: serde_json::Value,
    /// Additional environment variables as JSON object
    pub env: serde_json::Value,
    /// Working directory for the plugin process
    pub working_directory: Option<String>,

    // Permissions & Scopes
    /// RBAC permissions as JSON array (e.g., ["metadata:write:summary", "metadata:write:genres"])
    pub permissions: serde_json::Value,
    /// Scopes where plugin can be invoked as JSON array (e.g., ["series:detail", "series:bulk"])
    pub scopes: serde_json::Value,

    // Library filtering
    /// Library IDs this plugin applies to as JSON array of UUIDs
    /// Empty array = all libraries, non-empty = only these specific libraries
    /// Use case: Different metadata providers for manga vs comics vs ebooks
    pub library_ids: serde_json::Value,

    // Credentials
    /// Encrypted credentials (API keys, tokens)
    #[serde(skip_serializing)] // Never serialize credentials
    pub credentials: Option<Vec<u8>>,
    /// How to deliver credentials to the plugin: "env", "init_message", or "both"
    pub credential_delivery: String,

    // Configuration
    /// Plugin-specific configuration as JSON object
    pub config: serde_json::Value,
    /// Cached manifest from plugin (populated after first connection)
    pub manifest: Option<serde_json::Value>,

    // State
    /// Whether the plugin is enabled
    pub enabled: bool,
    /// Current health status: "unknown", "healthy", "degraded", "unhealthy", "disabled"
    pub health_status: String,
    /// Number of consecutive failures
    pub failure_count: i32,
    /// When the last failure occurred
    pub last_failure_at: Option<DateTime<Utc>>,
    /// When the last successful operation occurred
    pub last_success_at: Option<DateTime<Utc>>,
    /// Reason the plugin was disabled (e.g., "Disabled after 3 consecutive failures")
    pub disabled_reason: Option<String>,

    // Rate Limiting
    /// Maximum requests per minute (default: 60, None = no limit)
    pub rate_limit_requests_per_minute: Option<i32>,

    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::CreatedBy",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    CreatedByUser,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UpdatedBy",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    UpdatedByUser,
    #[sea_orm(has_many = "super::plugin_failures::Entity")]
    Failures,
}

impl Related<super::plugin_failures::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Failures.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// =============================================================================
// Health Status Enum
// =============================================================================

/// Health status values for plugin health checks
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginHealthStatus {
    /// Initial state, not yet checked
    #[default]
    Unknown,
    /// Plugin is working correctly
    Healthy,
    /// Plugin has some issues but is operational
    Degraded,
    /// Plugin is not functioning
    Unhealthy,
    /// Plugin was disabled due to failures or by admin
    Disabled,
}

impl PluginHealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PluginHealthStatus::Unknown => "unknown",
            PluginHealthStatus::Healthy => "healthy",
            PluginHealthStatus::Degraded => "degraded",
            PluginHealthStatus::Unhealthy => "unhealthy",
            PluginHealthStatus::Disabled => "disabled",
        }
    }
}

impl FromStr for PluginHealthStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "unknown" => Ok(PluginHealthStatus::Unknown),
            "healthy" => Ok(PluginHealthStatus::Healthy),
            "degraded" => Ok(PluginHealthStatus::Degraded),
            "unhealthy" => Ok(PluginHealthStatus::Unhealthy),
            "disabled" => Ok(PluginHealthStatus::Disabled),
            _ => Err(format!("Unknown plugin health status: {}", s)),
        }
    }
}

impl std::fmt::Display for PluginHealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// Credential Delivery Enum
// =============================================================================

/// How credentials are delivered to the plugin
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialDelivery {
    /// Pass credentials as environment variables
    #[default]
    Env,
    /// Pass credentials in the initialize message
    InitMessage,
    /// Pass credentials both ways
    Both,
}

impl CredentialDelivery {
    pub fn as_str(&self) -> &'static str {
        match self {
            CredentialDelivery::Env => "env",
            CredentialDelivery::InitMessage => "init_message",
            CredentialDelivery::Both => "both",
        }
    }
}

impl FromStr for CredentialDelivery {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "env" => Ok(CredentialDelivery::Env),
            "init_message" => Ok(CredentialDelivery::InitMessage),
            "both" => Ok(CredentialDelivery::Both),
            _ => Err(format!("Unknown credential delivery: {}", s)),
        }
    }
}

impl std::fmt::Display for CredentialDelivery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// Plugin Type Enum
// =============================================================================

/// Type of plugin determining who manages it
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginType {
    /// Admin-configured plugin for metadata fetching (shared across all users)
    #[default]
    System,
    /// User-configured plugin for sync/recommendations (per-user instances)
    User,
}

impl PluginType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PluginType::System => "system",
            PluginType::User => "user",
        }
    }
}

impl FromStr for PluginType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "system" => Ok(PluginType::System),
            "user" => Ok(PluginType::User),
            _ => Err(format!("Unknown plugin type: {}", s)),
        }
    }
}

impl std::fmt::Display for PluginType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// RBAC Permission Enum
// =============================================================================

/// RBAC permissions for plugin metadata writes
///
/// These permissions control what metadata fields a plugin can write.
/// Configured by admin when setting up the plugin.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginPermission {
    /// Read series/book metadata
    ///
    /// Includes: title, summary, genres, tags, year, status, authors, artists,
    /// publisher, external ratings (from providers), and user average rating.
    /// Does NOT include individual user's personal ratings or notes - those
    /// require user-level permissions (`user:ratings:read`, `user:notes:read`).
    #[serde(rename = "metadata:read")]
    MetadataRead,
    /// Update series/book titles
    #[serde(rename = "metadata:write:title")]
    MetadataWriteTitle,
    /// Update summaries/descriptions
    #[serde(rename = "metadata:write:summary")]
    MetadataWriteSummary,
    /// Update genres
    #[serde(rename = "metadata:write:genres")]
    MetadataWriteGenres,
    /// Update tags
    #[serde(rename = "metadata:write:tags")]
    MetadataWriteTags,
    /// Update cover images
    #[serde(rename = "metadata:write:covers")]
    MetadataWriteCovers,
    /// Write external ratings
    #[serde(rename = "metadata:write:ratings")]
    MetadataWriteRatings,
    /// Add external links
    #[serde(rename = "metadata:write:links")]
    MetadataWriteLinks,
    /// Update publication year
    #[serde(rename = "metadata:write:year")]
    MetadataWriteYear,
    /// Update publication status
    #[serde(rename = "metadata:write:status")]
    MetadataWriteStatus,
    /// Update publisher
    #[serde(rename = "metadata:write:publisher")]
    MetadataWritePublisher,
    /// Update age rating
    #[serde(rename = "metadata:write:age_rating")]
    MetadataWriteAgeRating,
    /// Update language
    #[serde(rename = "metadata:write:language")]
    MetadataWriteLanguage,
    /// Update reading direction
    #[serde(rename = "metadata:write:reading_direction")]
    MetadataWriteReadingDirection,
    /// Update total book count
    #[serde(rename = "metadata:write:total_book_count")]
    MetadataWriteTotalBookCount,
    /// All metadata write permissions
    #[serde(rename = "metadata:write:*")]
    MetadataWriteAll,
    /// Read library structure
    #[serde(rename = "library:read")]
    LibraryRead,
}

impl PluginPermission {
    pub fn as_str(&self) -> &'static str {
        match self {
            PluginPermission::MetadataRead => "metadata:read",
            PluginPermission::MetadataWriteTitle => "metadata:write:title",
            PluginPermission::MetadataWriteSummary => "metadata:write:summary",
            PluginPermission::MetadataWriteGenres => "metadata:write:genres",
            PluginPermission::MetadataWriteTags => "metadata:write:tags",
            PluginPermission::MetadataWriteCovers => "metadata:write:covers",
            PluginPermission::MetadataWriteRatings => "metadata:write:ratings",
            PluginPermission::MetadataWriteLinks => "metadata:write:links",
            PluginPermission::MetadataWriteYear => "metadata:write:year",
            PluginPermission::MetadataWriteStatus => "metadata:write:status",
            PluginPermission::MetadataWritePublisher => "metadata:write:publisher",
            PluginPermission::MetadataWriteAgeRating => "metadata:write:age_rating",
            PluginPermission::MetadataWriteLanguage => "metadata:write:language",
            PluginPermission::MetadataWriteReadingDirection => "metadata:write:reading_direction",
            PluginPermission::MetadataWriteTotalBookCount => "metadata:write:total_book_count",
            PluginPermission::MetadataWriteAll => "metadata:write:*",
            PluginPermission::LibraryRead => "library:read",
        }
    }

    /// Get all individual write permissions that "metadata:write:*" expands to
    pub fn all_write_permissions() -> Vec<PluginPermission> {
        vec![
            PluginPermission::MetadataWriteTitle,
            PluginPermission::MetadataWriteSummary,
            PluginPermission::MetadataWriteGenres,
            PluginPermission::MetadataWriteTags,
            PluginPermission::MetadataWriteCovers,
            PluginPermission::MetadataWriteRatings,
            PluginPermission::MetadataWriteLinks,
            PluginPermission::MetadataWriteYear,
            PluginPermission::MetadataWriteStatus,
            PluginPermission::MetadataWritePublisher,
            PluginPermission::MetadataWriteAgeRating,
            PluginPermission::MetadataWriteLanguage,
            PluginPermission::MetadataWriteReadingDirection,
            PluginPermission::MetadataWriteTotalBookCount,
        ]
    }
}

impl FromStr for PluginPermission {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "metadata:read" => Ok(PluginPermission::MetadataRead),
            "metadata:write:title" => Ok(PluginPermission::MetadataWriteTitle),
            "metadata:write:summary" => Ok(PluginPermission::MetadataWriteSummary),
            "metadata:write:genres" => Ok(PluginPermission::MetadataWriteGenres),
            "metadata:write:tags" => Ok(PluginPermission::MetadataWriteTags),
            "metadata:write:covers" => Ok(PluginPermission::MetadataWriteCovers),
            "metadata:write:ratings" => Ok(PluginPermission::MetadataWriteRatings),
            "metadata:write:links" => Ok(PluginPermission::MetadataWriteLinks),
            "metadata:write:year" => Ok(PluginPermission::MetadataWriteYear),
            "metadata:write:status" => Ok(PluginPermission::MetadataWriteStatus),
            "metadata:write:publisher" => Ok(PluginPermission::MetadataWritePublisher),
            "metadata:write:age_rating" => Ok(PluginPermission::MetadataWriteAgeRating),
            "metadata:write:language" => Ok(PluginPermission::MetadataWriteLanguage),
            "metadata:write:reading_direction" => {
                Ok(PluginPermission::MetadataWriteReadingDirection)
            }
            "metadata:write:total_book_count" => Ok(PluginPermission::MetadataWriteTotalBookCount),
            "metadata:write:*" => Ok(PluginPermission::MetadataWriteAll),
            "library:read" => Ok(PluginPermission::LibraryRead),
            _ => Err(format!("Unknown plugin permission: {}", s)),
        }
    }
}

impl std::fmt::Display for PluginPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// Helper Methods
// =============================================================================

impl Model {
    /// Parse the args JSON array into a Vec<String>
    pub fn args_vec(&self) -> Vec<String> {
        self.args
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Parse the env JSON object into a Vec<(String, String)>
    pub fn env_vec(&self) -> Vec<(String, String)> {
        self.env
            .as_object()
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Parse the permissions JSON array into a Vec<PluginPermission>
    pub fn permissions_vec(&self) -> Vec<PluginPermission> {
        self.permissions
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().and_then(|s| PluginPermission::from_str(s).ok()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if the plugin has a specific permission
    pub fn has_permission(&self, permission: &PluginPermission) -> bool {
        let permissions = self.permissions_vec();

        // Check for wildcard permission
        if permissions.contains(&PluginPermission::MetadataWriteAll) {
            // Wildcard grants all write permissions
            if matches!(
                permission,
                PluginPermission::MetadataWriteTitle
                    | PluginPermission::MetadataWriteSummary
                    | PluginPermission::MetadataWriteGenres
                    | PluginPermission::MetadataWriteTags
                    | PluginPermission::MetadataWriteCovers
                    | PluginPermission::MetadataWriteRatings
                    | PluginPermission::MetadataWriteLinks
                    | PluginPermission::MetadataWriteYear
                    | PluginPermission::MetadataWriteStatus
                    | PluginPermission::MetadataWritePublisher
                    | PluginPermission::MetadataWriteAgeRating
                    | PluginPermission::MetadataWriteLanguage
                    | PluginPermission::MetadataWriteReadingDirection
                    | PluginPermission::MetadataWriteTotalBookCount
            ) {
                return true;
            }
        }

        permissions.contains(permission)
    }

    /// Parse the scopes JSON array into a Vec<PluginScope>
    pub fn scopes_vec(&self) -> Vec<crate::services::plugin::protocol::PluginScope> {
        use crate::services::plugin::protocol::PluginScope;

        self.scopes
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value::<PluginScope>(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if the plugin supports a specific scope
    pub fn has_scope(&self, scope: &crate::services::plugin::protocol::PluginScope) -> bool {
        self.scopes_vec().contains(scope)
    }

    /// Parse the library_ids JSON array into a Vec<Uuid>
    pub fn library_ids_vec(&self) -> Vec<Uuid> {
        self.library_ids
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if the plugin applies to a specific library
    /// Returns true if library_ids is empty (applies to all) or contains the given library_id
    pub fn applies_to_library(&self, library_id: Uuid) -> bool {
        let library_ids = self.library_ids_vec();
        library_ids.is_empty() || library_ids.contains(&library_id)
    }

    /// Check if the plugin applies to all libraries (no restrictions)
    pub fn applies_to_all_libraries(&self) -> bool {
        self.library_ids_vec().is_empty()
    }

    /// Parse plugin type
    pub fn plugin_type_enum(&self) -> PluginType {
        PluginType::from_str(&self.plugin_type).unwrap_or_default()
    }

    /// Check if this is a system plugin (admin-configured)
    pub fn is_system_plugin(&self) -> bool {
        self.plugin_type_enum() == PluginType::System
    }

    /// Check if this is a user plugin (per-user instances)
    pub fn is_user_plugin(&self) -> bool {
        self.plugin_type_enum() == PluginType::User
    }

    /// Parse credential delivery type
    pub fn credential_delivery_type(&self) -> CredentialDelivery {
        CredentialDelivery::from_str(&self.credential_delivery).unwrap_or_default()
    }

    /// Parse health status
    pub fn health_status_type(&self) -> PluginHealthStatus {
        PluginHealthStatus::from_str(&self.health_status).unwrap_or_default()
    }

    /// Check if the plugin has credentials configured
    pub fn has_credentials(&self) -> bool {
        self.credentials.is_some()
    }

    /// Check if the plugin is in a healthy state (enabled and healthy)
    pub fn is_healthy(&self) -> bool {
        self.enabled
            && matches!(
                self.health_status_type(),
                PluginHealthStatus::Healthy | PluginHealthStatus::Unknown
            )
    }

    /// Get the cached manifest if available
    pub fn cached_manifest(&self) -> Option<crate::services::plugin::protocol::PluginManifest> {
        self.manifest
            .as_ref()
            .and_then(|m| serde_json::from_value(m.clone()).ok())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_health_status_as_str() {
        assert_eq!(PluginHealthStatus::Unknown.as_str(), "unknown");
        assert_eq!(PluginHealthStatus::Healthy.as_str(), "healthy");
        assert_eq!(PluginHealthStatus::Disabled.as_str(), "disabled");
    }

    #[test]
    fn test_plugin_health_status_from_str() {
        assert_eq!(
            PluginHealthStatus::from_str("healthy").unwrap(),
            PluginHealthStatus::Healthy
        );
        assert_eq!(
            PluginHealthStatus::from_str("disabled").unwrap(),
            PluginHealthStatus::Disabled
        );
        assert!(PluginHealthStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_credential_delivery_as_str() {
        assert_eq!(CredentialDelivery::Env.as_str(), "env");
        assert_eq!(CredentialDelivery::InitMessage.as_str(), "init_message");
        assert_eq!(CredentialDelivery::Both.as_str(), "both");
    }

    #[test]
    fn test_credential_delivery_from_str() {
        assert_eq!(
            CredentialDelivery::from_str("env").unwrap(),
            CredentialDelivery::Env
        );
        assert_eq!(
            CredentialDelivery::from_str("init_message").unwrap(),
            CredentialDelivery::InitMessage
        );
        assert!(CredentialDelivery::from_str("invalid").is_err());
    }

    #[test]
    fn test_plugin_type_as_str() {
        assert_eq!(PluginType::System.as_str(), "system");
        assert_eq!(PluginType::User.as_str(), "user");
    }

    #[test]
    fn test_plugin_type_from_str() {
        assert_eq!(PluginType::from_str("system").unwrap(), PluginType::System);
        assert_eq!(PluginType::from_str("user").unwrap(), PluginType::User);
        assert!(PluginType::from_str("invalid").is_err());
    }

    #[test]
    fn test_plugin_type_default() {
        assert_eq!(PluginType::default(), PluginType::System);
    }

    #[test]
    fn test_plugin_permission_as_str() {
        assert_eq!(PluginPermission::MetadataRead.as_str(), "metadata:read");
        assert_eq!(
            PluginPermission::MetadataWriteTitle.as_str(),
            "metadata:write:title"
        );
        assert_eq!(
            PluginPermission::MetadataWriteAll.as_str(),
            "metadata:write:*"
        );
    }

    #[test]
    fn test_plugin_permission_from_str() {
        assert_eq!(
            PluginPermission::from_str("metadata:read").unwrap(),
            PluginPermission::MetadataRead
        );
        assert_eq!(
            PluginPermission::from_str("metadata:write:summary").unwrap(),
            PluginPermission::MetadataWriteSummary
        );
        assert_eq!(
            PluginPermission::from_str("metadata:write:*").unwrap(),
            PluginPermission::MetadataWriteAll
        );
        assert!(PluginPermission::from_str("invalid").is_err());
    }

    #[test]
    fn test_plugin_permission_serialization() {
        let perm = PluginPermission::MetadataWriteTitle;
        let json = serde_json::to_string(&perm).unwrap();
        assert_eq!(json, "\"metadata:write:title\"");

        let perm: PluginPermission = serde_json::from_str("\"metadata:write:genres\"").unwrap();
        assert_eq!(perm, PluginPermission::MetadataWriteGenres);
    }

    #[test]
    fn test_all_write_permissions() {
        let perms = PluginPermission::all_write_permissions();
        assert!(perms.contains(&PluginPermission::MetadataWriteTitle));
        assert!(perms.contains(&PluginPermission::MetadataWriteSummary));
        assert!(!perms.contains(&PluginPermission::MetadataWriteAll));
        assert!(!perms.contains(&PluginPermission::MetadataRead));
    }

    #[test]
    fn test_library_ids_vec_empty() {
        use chrono::Utc;
        let model = Model {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            display_name: "Test".to_string(),
            description: None,
            plugin_type: "system".to_string(),
            command: "node".to_string(),
            args: serde_json::json!([]),
            env: serde_json::json!({}),
            working_directory: None,
            permissions: serde_json::json!([]),
            scopes: serde_json::json!([]),
            library_ids: serde_json::json!([]),
            credentials: None,
            credential_delivery: "env".to_string(),
            config: serde_json::json!({}),
            manifest: None,
            enabled: true,
            health_status: "healthy".to_string(),
            failure_count: 0,
            last_failure_at: None,
            last_success_at: None,
            disabled_reason: None,
            rate_limit_requests_per_minute: Some(60),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: None,
            updated_by: None,
        };

        assert!(model.library_ids_vec().is_empty());
        assert!(model.applies_to_all_libraries());
        // Empty library_ids means applies to all libraries
        assert!(model.applies_to_library(Uuid::new_v4()));
    }

    #[test]
    fn test_library_ids_vec_with_values() {
        use chrono::Utc;
        let lib1 = Uuid::new_v4();
        let lib2 = Uuid::new_v4();
        let lib3 = Uuid::new_v4();

        let model = Model {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            display_name: "Test".to_string(),
            description: None,
            plugin_type: "system".to_string(),
            command: "node".to_string(),
            args: serde_json::json!([]),
            env: serde_json::json!({}),
            working_directory: None,
            permissions: serde_json::json!([]),
            scopes: serde_json::json!([]),
            library_ids: serde_json::json!([lib1.to_string(), lib2.to_string()]),
            credentials: None,
            credential_delivery: "env".to_string(),
            config: serde_json::json!({}),
            manifest: None,
            enabled: true,
            health_status: "healthy".to_string(),
            failure_count: 0,
            last_failure_at: None,
            last_success_at: None,
            disabled_reason: None,
            rate_limit_requests_per_minute: Some(60),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: None,
            updated_by: None,
        };

        let library_ids = model.library_ids_vec();
        assert_eq!(library_ids.len(), 2);
        assert!(library_ids.contains(&lib1));
        assert!(library_ids.contains(&lib2));

        assert!(!model.applies_to_all_libraries());
        assert!(model.applies_to_library(lib1));
        assert!(model.applies_to_library(lib2));
        assert!(!model.applies_to_library(lib3)); // Not in the list
    }
}
