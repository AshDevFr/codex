//! Plugin DTOs
//!
//! Data Transfer Objects for the Plugin API, enabling CRUD operations
//! for admin-configured external metadata provider plugins.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::entities::plugin_failures;
use crate::db::entities::plugins::{self, InternalPluginConfig, PluginPermission};
use crate::db::repositories::PluginsRepository;
use crate::services::plugin::protocol::{
    CredentialField, MetadataContentType, PluginCapabilities, PluginScope,
};

use super::common::deserialize_optional_nullable;

// =============================================================================
// Plugin Response DTOs
// =============================================================================

/// A plugin (credentials are never exposed)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginDto {
    /// Plugin ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// Unique identifier (e.g., "mangabaka")
    #[schema(example = "mangabaka")]
    pub name: String,

    /// Human-readable display name
    #[schema(example = "MangaBaka")]
    pub display_name: String,

    /// Description of the plugin
    #[schema(example = "Fetch manga metadata from MangaBaka (MangaUpdates)")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Plugin type: "system" (admin-configured) or "user" (per-user instances)
    #[schema(example = "system")]
    pub plugin_type: String,

    /// Command to spawn the plugin
    #[schema(example = "node")]
    pub command: String,

    /// Command arguments
    #[schema(example = json!(["/opt/codex/plugins/mangabaka/dist/index.js"]))]
    pub args: Vec<String>,

    /// Additional environment variables (non-sensitive only)
    #[schema(example = json!({"LOG_LEVEL": "info"}))]
    pub env: serde_json::Value,

    /// Working directory for the plugin process
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,

    /// RBAC permissions for metadata writes
    #[schema(example = json!(["metadata:write:summary", "metadata:write:genres"]))]
    pub permissions: Vec<String>,

    /// Scopes where plugin can be invoked
    #[schema(example = json!(["series:detail", "series:bulk"]))]
    pub scopes: Vec<String>,

    /// Library IDs this plugin applies to (empty = all libraries)
    #[schema(example = json!([]))]
    pub library_ids: Vec<Uuid>,

    /// Whether credentials have been set (actual credentials are never returned)
    #[schema(example = true)]
    pub has_credentials: bool,

    /// How credentials are delivered to the plugin
    #[schema(example = "env")]
    pub credential_delivery: String,

    /// Plugin-specific configuration
    #[schema(example = json!({"rate_limit": 60}))]
    pub config: serde_json::Value,

    /// Cached manifest from plugin (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<PluginManifestDto>,

    /// Whether the plugin is enabled
    #[schema(example = true)]
    pub enabled: bool,

    /// Health status: unknown, healthy, degraded, unhealthy, disabled
    #[schema(example = "healthy")]
    pub health_status: String,

    /// Number of consecutive failures
    #[schema(example = 0)]
    pub failure_count: i32,

    /// When the last failure occurred
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure_at: Option<DateTime<Utc>>,

    /// When the last successful operation occurred
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_success_at: Option<DateTime<Utc>>,

    /// Reason the plugin was disabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,

    /// Rate limit in requests per minute (None = no limit)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 60)]
    pub rate_limit_requests_per_minute: Option<i32>,

    /// When the plugin was created
    pub created_at: DateTime<Utc>,

    /// When the plugin was last updated
    pub updated_at: DateTime<Utc>,

    /// Handlebars template for customizing search queries
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_query_template: Option<String>,

    /// Preprocessing rules for search queries (JSON array of regex rules)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_preprocessing_rules: Option<serde_json::Value>,

    /// Auto-match conditions (JSON object with mode and rules)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_match_conditions: Option<serde_json::Value>,

    /// Whether to skip search when external ID exists for this plugin
    #[schema(example = true)]
    pub use_existing_external_id: bool,

    /// Metadata targets: which resource types this plugin auto-matches against
    /// null = auto-detect from plugin capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["series", "book"]))]
    pub metadata_targets: Option<Vec<String>>,

    /// Internal server-side configuration (not sent to plugin)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_config: Option<InternalPluginConfig>,

    /// Number of users who have enabled this plugin (only for user-type plugins)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 3)]
    pub user_count: Option<u64>,
}

impl From<plugins::Model> for PluginDto {
    fn from(model: plugins::Model) -> Self {
        let has_credentials = PluginsRepository::has_credentials(&model);
        let args = model.args_vec();
        let permissions: Vec<String> = model
            .permissions_vec()
            .into_iter()
            .map(|p| p.to_string())
            .collect();
        let scopes: Vec<String> = model
            .scopes_vec()
            .into_iter()
            .map(|s| scope_to_string(&s))
            .collect();
        let library_ids = model.library_ids_vec();

        // Parse manifest if available
        let manifest = model.cached_manifest().map(PluginManifestDto::from);

        Self {
            id: model.id,
            name: model.name,
            display_name: model.display_name,
            description: model.description,
            plugin_type: model.plugin_type,
            command: model.command,
            args,
            env: model.env,
            working_directory: model.working_directory,
            permissions,
            scopes,
            library_ids,
            has_credentials,
            credential_delivery: model.credential_delivery,
            config: model.config,
            manifest,
            enabled: model.enabled,
            health_status: model.health_status,
            failure_count: model.failure_count,
            last_failure_at: model.last_failure_at,
            last_success_at: model.last_success_at,
            disabled_reason: model.disabled_reason,
            rate_limit_requests_per_minute: model.rate_limit_requests_per_minute,
            created_at: model.created_at,
            updated_at: model.updated_at,
            search_query_template: model.search_query_template,
            search_preprocessing_rules: model
                .search_preprocessing_rules
                .and_then(|s| serde_json::from_str(&s).ok()),
            auto_match_conditions: model
                .auto_match_conditions
                .and_then(|s| serde_json::from_str(&s).ok()),
            use_existing_external_id: model.use_existing_external_id,
            metadata_targets: model
                .metadata_targets
                .and_then(|s| serde_json::from_str(&s).ok()),
            internal_config: model
                .internal_config
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok()),
            user_count: None,
        }
    }
}

/// Configuration field definition for documenting plugin config options
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFieldDto {
    /// Field name (key in JSON config)
    pub key: String,
    /// Human-readable label
    pub label: String,
    /// Description of what this field does
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Field type: "number", "string", or "boolean"
    #[serde(rename = "type")]
    pub field_type: String,
    /// Whether this field is required
    #[serde(default)]
    pub required: bool,
    /// Default value if not provided
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    /// Example value for documentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<serde_json::Value>,
}

/// Plugin configuration schema - documents available config options
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSchemaDto {
    /// Human-readable description of the configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// List of configuration fields
    pub fields: Vec<ConfigFieldDto>,
}

/// OAuth 2.0 configuration from plugin manifest
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfigDto {
    /// OAuth 2.0 authorization endpoint URL
    pub authorization_url: String,
    /// OAuth 2.0 token endpoint URL
    pub token_url: String,
    /// Required OAuth scopes
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Whether to use PKCE (Proof Key for Code Exchange)
    pub pkce: bool,
    /// Optional user info endpoint URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_info_url: Option<String>,
}

impl From<crate::services::plugin::protocol::OAuthConfig> for OAuthConfigDto {
    fn from(o: crate::services::plugin::protocol::OAuthConfig) -> Self {
        Self {
            authorization_url: o.authorization_url,
            token_url: o.token_url,
            scopes: o.scopes,
            pkce: o.pkce,
            user_info_url: o.user_info_url,
        }
    }
}

/// Plugin manifest from the plugin itself
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifestDto {
    /// Unique identifier
    pub name: String,
    /// Display name for UI
    pub display_name: String,
    /// Semantic version
    pub version: String,
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Author
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Homepage URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    /// Protocol version
    pub protocol_version: String,
    /// Plugin capabilities
    pub capabilities: PluginCapabilitiesDto,
    /// Supported content types
    pub content_types: Vec<String>,
    /// Required credentials
    #[serde(default)]
    pub required_credentials: Vec<CredentialFieldDto>,
    /// Supported scopes
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Configuration schema documenting available config options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<ConfigSchemaDto>,
    /// OAuth 2.0 configuration (if plugin supports OAuth)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OAuthConfigDto>,
    /// Admin-facing setup instructions (e.g., how to create OAuth app, set client ID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_setup_instructions: Option<String>,
    /// User-facing setup instructions (e.g., how to connect or get a personal token)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_setup_instructions: Option<String>,

    /// URI template for searching on the plugin's website
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_uri_template: Option<String>,
}

impl From<crate::services::plugin::protocol::PluginManifest> for PluginManifestDto {
    fn from(m: crate::services::plugin::protocol::PluginManifest) -> Self {
        // Derive content types from capabilities
        let content_types: Vec<String> = m
            .capabilities
            .metadata_provider
            .iter()
            .map(content_type_to_string)
            .collect();

        // Derive scopes from capabilities (series metadata provider gets series scopes)
        let scopes: Vec<String> = if m.capabilities.can_provide_series_metadata() {
            PluginScope::series_scopes()
                .into_iter()
                .map(|s| scope_to_string(&s))
                .collect()
        } else {
            vec![]
        };

        // Parse config_schema from JSON Value to typed ConfigSchemaDto
        let config_schema = m
            .config_schema
            .and_then(|v| serde_json::from_value::<ConfigSchemaDto>(v).ok());

        Self {
            name: m.name,
            display_name: m.display_name,
            version: m.version,
            description: m.description,
            author: m.author,
            homepage: m.homepage,
            protocol_version: m.protocol_version,
            capabilities: PluginCapabilitiesDto::from(m.capabilities),
            content_types,
            required_credentials: m
                .required_credentials
                .into_iter()
                .map(CredentialFieldDto::from)
                .collect(),
            scopes,
            config_schema,
            oauth: m.oauth.map(OAuthConfigDto::from),
            admin_setup_instructions: m.admin_setup_instructions,
            user_setup_instructions: m.user_setup_instructions,
            search_uri_template: m.search_uri_template,
        }
    }
}

/// Plugin capabilities
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginCapabilitiesDto {
    /// Content types this plugin can provide metadata for (e.g., ["series", "book"])
    #[serde(default)]
    pub metadata_provider: Vec<String>,
    /// Can sync user reading progress
    #[serde(default)]
    pub user_read_sync: bool,
    /// External ID source for matching sync entries to series (e.g., "api:anilist")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id_source: Option<String>,
    /// Can provide personalized recommendations
    #[serde(default)]
    pub user_recommendation_provider: bool,
}

impl From<PluginCapabilities> for PluginCapabilitiesDto {
    fn from(c: PluginCapabilities) -> Self {
        Self {
            metadata_provider: c
                .metadata_provider
                .iter()
                .map(content_type_to_string)
                .collect(),
            user_read_sync: c.user_read_sync,
            external_id_source: c.external_id_source,
            user_recommendation_provider: c.user_recommendation_provider,
        }
    }
}

/// Credential field definition
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CredentialFieldDto {
    /// Credential key (e.g., "api_key")
    pub key: String,
    /// Display label (e.g., "API Key")
    pub label: String,
    /// Description for the user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this credential is required
    #[serde(default)]
    pub required: bool,
    /// Whether to mask the value in UI
    #[serde(default)]
    pub sensitive: bool,
    /// Input type for UI
    pub credential_type: String,
}

impl From<CredentialField> for CredentialFieldDto {
    fn from(f: CredentialField) -> Self {
        let credential_type = match f.credential_type {
            crate::services::plugin::protocol::CredentialType::String => "string",
            crate::services::plugin::protocol::CredentialType::Password => "password",
            crate::services::plugin::protocol::CredentialType::OAuth => "oauth",
        };
        Self {
            key: f.key,
            label: f.label,
            description: f.description,
            required: f.required,
            sensitive: f.sensitive,
            credential_type: credential_type.to_string(),
        }
    }
}

/// Response containing a list of plugins
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginsListResponse {
    /// List of plugins
    pub plugins: Vec<PluginDto>,
    /// Total count
    pub total: usize,
}

// =============================================================================
// Plugin Request DTOs
// =============================================================================

/// Request to create a new plugin
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatePluginRequest {
    /// Unique identifier (alphanumeric with underscores)
    #[schema(example = "mangabaka")]
    pub name: String,

    /// Human-readable display name
    #[schema(example = "MangaBaka")]
    pub display_name: String,

    /// Description of the plugin
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Fetch manga metadata from MangaBaka (MangaUpdates)")]
    pub description: Option<String>,

    /// Plugin type: "system" (default) or "user"
    #[serde(default = "default_plugin_type")]
    #[schema(example = "system")]
    pub plugin_type: String,

    /// Command to spawn the plugin
    #[schema(example = "node")]
    pub command: String,

    /// Command arguments
    #[serde(default)]
    #[schema(example = json!(["/opt/codex/plugins/mangabaka/dist/index.js"]))]
    pub args: Vec<String>,

    /// Additional environment variables
    #[serde(default)]
    #[schema(example = json!({"LOG_LEVEL": "info"}))]
    pub env: Vec<EnvVarDto>,

    /// Working directory for the plugin process
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,

    /// RBAC permissions for metadata writes
    #[serde(default)]
    #[schema(example = json!(["metadata:write:summary", "metadata:write:genres"]))]
    pub permissions: Vec<String>,

    /// Scopes where plugin can be invoked
    #[serde(default)]
    #[schema(example = json!(["series:detail", "series:bulk"]))]
    pub scopes: Vec<String>,

    /// Library IDs this plugin applies to (empty = all libraries)
    #[serde(default)]
    #[schema(example = json!([]))]
    pub library_ids: Vec<Uuid>,

    /// Credentials (will be encrypted before storage)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!({"api_key": "your-api-key"}))]
    pub credentials: Option<serde_json::Value>,

    /// How credentials are delivered to the plugin: "env", "init_message", or "both"
    #[serde(default = "default_credential_delivery")]
    #[schema(example = "env")]
    pub credential_delivery: String,

    /// Plugin-specific configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!({"rate_limit": 60}))]
    pub config: Option<serde_json::Value>,

    /// Whether to enable immediately
    #[serde(default)]
    #[schema(example = false)]
    pub enabled: bool,

    /// Rate limit in requests per minute (default: 60, None = no limit)
    #[serde(default = "default_rate_limit")]
    #[schema(example = 60)]
    pub rate_limit_requests_per_minute: Option<i32>,

    /// Handlebars template for customizing search queries
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_query_template: Option<String>,

    /// Preprocessing rules for search queries (JSON array of regex rules)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_preprocessing_rules: Option<serde_json::Value>,

    /// Auto-match conditions (JSON object with mode and rules)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_match_conditions: Option<serde_json::Value>,

    /// Whether to skip search when external ID exists for this plugin
    #[serde(default = "default_use_existing_external_id")]
    #[schema(example = true)]
    pub use_existing_external_id: bool,

    /// Metadata targets: which resource types this plugin auto-matches against
    /// null = auto-detect from plugin capabilities
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["book"]))]
    pub metadata_targets: Option<Vec<String>>,
}

fn default_use_existing_external_id() -> bool {
    true
}

fn default_rate_limit() -> Option<i32> {
    Some(60)
}

/// Environment variable key-value pair
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EnvVarDto {
    pub key: String,
    pub value: String,
}

fn default_plugin_type() -> String {
    "system".to_string()
}

fn default_credential_delivery() -> String {
    "init_message".to_string()
}

/// Request to update a plugin
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePluginRequest {
    /// Updated display name
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "MangaBaka v2")]
    pub display_name: Option<String>,

    /// Updated description
    #[serde(default)]
    pub description: Option<Option<String>>,

    /// Updated command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Updated command arguments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,

    /// Updated environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<EnvVarDto>>,

    /// Updated working directory
    #[serde(default)]
    pub working_directory: Option<Option<String>>,

    /// Updated permissions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,

    /// Updated scopes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,

    /// Updated library IDs (empty = all libraries)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_ids: Option<Vec<Uuid>>,

    /// Updated credentials (set to null to clear)
    #[serde(default)]
    pub credentials: Option<serde_json::Value>,

    /// Updated credential delivery method
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_delivery: Option<String>,

    /// Updated configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,

    /// Updated rate limit in requests per minute (Some(None) = remove limit)
    #[serde(default)]
    #[schema(example = 60)]
    pub rate_limit_requests_per_minute: Option<Option<i32>>,

    /// Handlebars template for customizing search queries (null = clear template)
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_nullable"
    )]
    pub search_query_template: Option<serde_json::Value>,

    /// Preprocessing rules for search queries (JSON array of regex rules)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_preprocessing_rules: Option<serde_json::Value>,

    /// Auto-match conditions (JSON object with mode and rules)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_match_conditions: Option<serde_json::Value>,

    /// Whether to skip search when external ID exists for this plugin
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_existing_external_id: Option<bool>,

    /// Metadata targets: which resource types this plugin auto-matches against
    /// null = clear to auto-detect from plugin capabilities
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_nullable"
    )]
    pub metadata_targets: Option<serde_json::Value>,

    /// Internal server-side configuration (not sent to plugin)
    /// Validated as InternalPluginConfig on the server
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_config: Option<serde_json::Value>,
}

// =============================================================================
// Plugin Action Response DTOs
// =============================================================================

/// Response from testing a plugin connection
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginTestResult {
    /// Whether the test was successful
    #[schema(example = true)]
    pub success: bool,

    /// Test result message
    #[schema(example = "Successfully connected to plugin")]
    pub message: String,

    /// Response latency in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 150)]
    pub latency_ms: Option<u64>,

    /// Plugin manifest (if connection succeeded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<PluginManifestDto>,
}

/// Response after enabling or disabling a plugin
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginStatusResponse {
    /// The updated plugin
    pub plugin: PluginDto,

    /// Status change message
    #[schema(example = "Plugin enabled successfully")]
    pub message: String,

    /// Whether a health check was performed
    #[serde(default)]
    #[schema(example = true)]
    pub health_check_performed: bool,

    /// Health check passed (None if not performed)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = true)]
    pub health_check_passed: Option<bool>,

    /// Health check latency in milliseconds (None if not performed)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 150)]
    pub health_check_latency_ms: Option<u64>,

    /// Health check error message (None if passed or not performed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check_error: Option<String>,
}

/// Plugin health information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginHealthDto {
    /// Plugin ID
    pub plugin_id: Uuid,

    /// Plugin name
    pub name: String,

    /// Current health status
    pub health_status: String,

    /// Whether the plugin is enabled
    pub enabled: bool,

    /// Number of consecutive failures
    pub failure_count: i32,

    /// When the last failure occurred
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure_at: Option<DateTime<Utc>>,

    /// When the last successful operation occurred
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_success_at: Option<DateTime<Utc>>,

    /// Reason the plugin was disabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
}

impl From<plugins::Model> for PluginHealthDto {
    fn from(model: plugins::Model) -> Self {
        Self {
            plugin_id: model.id,
            name: model.name,
            health_status: model.health_status,
            enabled: model.enabled,
            failure_count: model.failure_count,
            last_failure_at: model.last_failure_at,
            last_success_at: model.last_success_at,
            disabled_reason: model.disabled_reason,
        }
    }
}

/// Response containing plugin health history/summary
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginHealthResponse {
    /// Plugin health information
    pub health: PluginHealthDto,
}

// =============================================================================
// Plugin Failure DTOs
// =============================================================================

/// A single plugin failure event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginFailureDto {
    /// Failure ID
    pub id: Uuid,

    /// Human-readable error message
    #[schema(example = "Connection timeout after 30s")]
    pub error_message: String,

    /// Error code for categorization
    #[schema(example = "TIMEOUT")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,

    /// Which method failed
    #[schema(example = "metadata/search")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,

    /// Additional context (parameters, stack trace, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,

    /// Sanitized summary of request parameters (sensitive fields redacted)
    #[schema(example = "query: \"One Piece\", limit: 10")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_summary: Option<String>,

    /// When the failure occurred
    pub occurred_at: DateTime<Utc>,
}

impl From<plugin_failures::Model> for PluginFailureDto {
    fn from(model: plugin_failures::Model) -> Self {
        Self {
            id: model.id,
            error_message: model.error_message,
            error_code: model.error_code,
            method: model.method,
            context: model.context,
            request_summary: model.request_summary,
            occurred_at: model.occurred_at,
        }
    }
}

/// Response containing plugin failure history
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginFailuresResponse {
    /// List of failure events
    pub failures: Vec<PluginFailureDto>,

    /// Total number of failures (for pagination)
    pub total: u64,

    /// Number of failures within the current time window
    pub window_failures: u64,

    /// Time window size in seconds
    #[schema(example = 3600)]
    pub window_seconds: i64,

    /// Threshold for auto-disable
    #[schema(example = 3)]
    pub threshold: u32,
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Convert PluginScope to string
fn scope_to_string(scope: &PluginScope) -> String {
    match scope {
        PluginScope::SeriesDetail => "series:detail".to_string(),
        PluginScope::SeriesBulk => "series:bulk".to_string(),
        PluginScope::BookDetail => "book:detail".to_string(),
        PluginScope::BookBulk => "book:bulk".to_string(),
        PluginScope::LibraryDetail => "library:detail".to_string(),
        PluginScope::LibraryScan => "library:scan".to_string(),
    }
}

/// Convert MetadataContentType to string
fn content_type_to_string(ct: &MetadataContentType) -> String {
    match ct {
        MetadataContentType::Series => "series".to_string(),
        MetadataContentType::Book => "book".to_string(),
    }
}

/// Parse string to PluginScope
pub fn parse_scope(s: &str) -> Option<PluginScope> {
    match s {
        "series:detail" => Some(PluginScope::SeriesDetail),
        "series:bulk" => Some(PluginScope::SeriesBulk),
        "book:detail" => Some(PluginScope::BookDetail),
        "book:bulk" => Some(PluginScope::BookBulk),
        "library:detail" => Some(PluginScope::LibraryDetail),
        "library:scan" => Some(PluginScope::LibraryScan),
        _ => None,
    }
}

/// Parse string to PluginPermission
pub fn parse_permission(s: &str) -> Option<PluginPermission> {
    std::str::FromStr::from_str(s).ok()
}

/// Available plugin permissions for documentation/validation
pub fn available_permissions() -> Vec<&'static str> {
    vec![
        // Read permissions
        "metadata:read",
        // Common write permissions (series + books)
        "metadata:write:title",
        "metadata:write:summary",
        "metadata:write:genres",
        "metadata:write:tags",
        "metadata:write:covers",
        "metadata:write:ratings",
        "metadata:write:links",
        "metadata:write:year",
        "metadata:write:status",
        "metadata:write:publisher",
        "metadata:write:age_rating",
        "metadata:write:language",
        "metadata:write:reading_direction",
        "metadata:write:total_volume_count",
        "metadata:write:total_chapter_count",
        // Book-specific write permissions
        "metadata:write:book_type",
        "metadata:write:subtitle",
        "metadata:write:authors",
        "metadata:write:translator",
        "metadata:write:edition",
        "metadata:write:original_title",
        "metadata:write:original_year",
        "metadata:write:series_position",
        "metadata:write:subjects",
        "metadata:write:awards",
        "metadata:write:custom_metadata",
        "metadata:write:isbn",
        // Wildcard
        "metadata:write:*",
        // Library
        "library:read",
    ]
}

/// Available plugin scopes for documentation/validation
pub fn available_scopes() -> Vec<&'static str> {
    vec![
        "series:detail",
        "series:bulk",
        "book:detail",
        "book:bulk",
        "library:detail",
        "library:scan",
    ]
}

/// Available credential delivery methods
pub fn available_credential_delivery_methods() -> Vec<&'static str> {
    vec!["env", "init_message", "both"]
}

// =============================================================================
// Plugin Actions DTOs (Phase 4)
// =============================================================================

/// A plugin action available for a specific scope
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginActionDto {
    /// Plugin ID
    pub plugin_id: Uuid,

    /// Plugin name
    pub plugin_name: String,

    /// Plugin display name
    pub plugin_display_name: String,

    /// Action type (e.g., "metadata_search", "metadata_get")
    pub action_type: String,

    /// Human-readable label for the action
    pub label: String,

    /// Description of the action
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Icon hint for UI (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,

    /// Library IDs this plugin applies to (empty means all libraries)
    /// Used by frontend to filter which plugins show up for each library
    #[serde(default)]
    pub library_ids: Vec<Uuid>,

    /// URI template for searching on the plugin's website (from manifest)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_uri_template: Option<String>,
}

/// Response containing available plugin actions for a scope
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginActionsResponse {
    /// Available actions grouped by plugin
    pub actions: Vec<PluginActionDto>,

    /// The scope these actions are for
    pub scope: String,
}

/// Action for metadata plugins
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetadataAction {
    /// Search for metadata by query
    Search,
    /// Get full metadata by external ID
    Get,
    /// Find best match for a title (auto-match)
    Match,
}

/// Plugin action request - tagged by plugin type
///
/// Each plugin type has its own set of valid actions.
/// This ensures type safety - you can't call a metadata action on a sync plugin.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PluginActionRequest {
    /// Metadata plugin actions (search, get, match)
    Metadata {
        /// The metadata action to perform
        action: MetadataAction,
        /// Content type (series or book)
        #[serde(rename = "contentType")]
        content_type: MetadataContentType,
        /// Action-specific parameters
        #[serde(default)]
        params: serde_json::Value,
    },
    /// Health check (works for any plugin type)
    Ping,
    // Future: Sync { action: SyncAction, params: serde_json::Value },
}

/// Request to execute a plugin action
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecutePluginRequest {
    /// The action to execute, tagged by plugin type
    pub action: PluginActionRequest,
}

/// Response from executing a plugin method
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecutePluginResponse {
    /// Whether the execution succeeded
    pub success: bool,

    /// Result data (varies by method)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,

    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Execution time in milliseconds
    pub latency_ms: u64,
}

/// Search result from a plugin
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginSearchResultDto {
    /// External ID from the provider
    pub external_id: String,

    /// Primary title
    pub title: String,

    /// Alternative titles
    #[serde(default)]
    pub alternate_titles: Vec<String>,

    /// Year of publication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,

    /// Cover image URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,

    /// Relevance score (0.0-1.0). Optional - if not provided, result order indicates relevance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relevance_score: Option<f64>,

    /// Preview data for search results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<SearchResultPreviewDto>,
}

/// Preview data for search results
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultPreviewDto {
    /// Status string (series search results)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    /// Genres
    #[serde(default)]
    pub genres: Vec<String>,

    /// Rating
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<f64>,

    /// Short description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Number of books in the series (if known by the provider)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_count: Option<i32>,

    /// Author names (book search results)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,

    /// Content format discriminator (e.g. `manga`, `novel`, `light_novel`,
    /// `manhwa`, `manhua`, `comic`, `webtoon`, `one_shot`).
    ///
    /// Free-form string at the protocol layer; the UI maps known values to
    /// colored badges and falls back to a neutral badge for anything else.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

/// Response containing search results from a plugin
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginSearchResponse {
    /// Search results
    pub results: Vec<PluginSearchResultDto>,

    /// Cursor for next page (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,

    /// Plugin that provided the results
    pub plugin_id: Uuid,

    /// Plugin name
    pub plugin_name: String,
}

// =============================================================================
// Metadata Preview/Apply DTOs (Phase 4)
// =============================================================================

/// Status of a field during metadata preview
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FieldApplyStatus {
    /// Field will be applied (different value, no lock, has permission)
    WillApply,
    /// Field is locked by user
    Locked,
    /// No permission to write this field
    NoPermission,
    /// Value is unchanged
    Unchanged,
    /// Field is not provided by plugin
    NotProvided,
}

/// A single field in the metadata preview
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetadataFieldPreview {
    /// Field name
    pub field: String,

    /// Current value in database
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_value: Option<serde_json::Value>,

    /// Proposed value from plugin
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposed_value: Option<serde_json::Value>,

    /// Apply status
    pub status: FieldApplyStatus,

    /// Human-readable reason for status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Request to preview metadata from a plugin
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetadataPreviewRequest {
    /// Plugin ID to fetch metadata from
    pub plugin_id: Uuid,

    /// External ID from the plugin's search results
    pub external_id: String,
}

/// Response containing metadata preview
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetadataPreviewResponse {
    /// Field-by-field preview
    pub fields: Vec<MetadataFieldPreview>,

    /// Summary counts
    pub summary: PreviewSummary,

    /// Plugin that provided the metadata
    pub plugin_id: Uuid,

    /// Plugin name
    pub plugin_name: String,

    /// External ID used
    pub external_id: String,

    /// External URL (link to provider's page)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_url: Option<String>,
}

/// Summary of preview results
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSummary {
    /// Number of fields that will be applied
    pub will_apply: usize,

    /// Number of fields that are locked
    pub locked: usize,

    /// Number of fields with no permission
    pub no_permission: usize,

    /// Number of fields that are unchanged
    pub unchanged: usize,

    /// Number of fields not provided
    pub not_provided: usize,
}

/// Request to apply metadata from a plugin
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetadataApplyRequest {
    /// Plugin ID to fetch metadata from
    pub plugin_id: Uuid,

    /// External ID from the plugin's search results
    pub external_id: String,

    /// Optional list of fields to apply (default: all applicable fields)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<String>>,

    /// When `true`, the call simulates the apply without writing to the
    /// database. Returns the same `appliedFields`/`skippedFields` plus an
    /// extra `dryRunReport` showing every would-be change. Default `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub dry_run: bool,
}

/// One would-be field change recorded during a dry-run apply.
///
/// Mirrors `services::metadata::apply::FieldChange`, kept as a distinct DTO
/// to keep the wire-format frozen even if internal types evolve.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FieldChangeDto {
    pub field: String,
    /// Current value, where cheaply available. `null` for fields backed by
    /// joined tables (genres, tags, alternate titles, ratings, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<serde_json::Value>,
    pub after: serde_json::Value,
}

/// Dry-run preview attached to [`MetadataApplyResponse`] when the request
/// set `dryRun = true`. Absent on real applies.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DryRunReportDto {
    pub changes: Vec<FieldChangeDto>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Response after applying metadata
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetadataApplyResponse {
    /// Whether the operation succeeded
    pub success: bool,

    /// Fields that were applied
    pub applied_fields: Vec<String>,

    /// Fields that were skipped (with reasons)
    pub skipped_fields: Vec<SkippedField>,

    /// Message
    pub message: String,

    /// Populated only when the request set `dryRun = true`. Each entry is a
    /// field that *would* have been written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dry_run_report: Option<DryRunReportDto>,
}

/// A field that was skipped during apply
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkippedField {
    /// Field name
    pub field: String,

    /// Reason for skipping
    pub reason: String,
}

/// Response containing the preprocessed search title for a series
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchTitleResponse {
    /// Original title before preprocessing
    pub original_title: String,

    /// Title after preprocessing rules were applied
    pub search_title: String,

    /// Number of preprocessing rules that were applied
    pub rules_applied: usize,
}

/// Request to auto-match and apply metadata from a plugin
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetadataAutoMatchRequest {
    /// Plugin ID to use for matching
    pub plugin_id: Uuid,

    /// Optional query to use for matching (defaults to series title)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
}

/// Response after auto-matching metadata
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetadataAutoMatchResponse {
    /// Whether the operation succeeded
    pub success: bool,

    /// The search result that was matched
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_result: Option<PluginSearchResultDto>,

    /// Fields that were applied
    pub applied_fields: Vec<String>,

    /// Fields that were skipped (with reasons)
    pub skipped_fields: Vec<SkippedField>,

    /// Message
    pub message: String,

    /// External URL (link to matched item on provider)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_url: Option<String>,
}

/// Request to enqueue plugin auto-match task for a single series
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueAutoMatchRequest {
    /// Plugin ID to use for matching
    pub plugin_id: Uuid,
}

/// Request to enqueue plugin auto-match tasks for multiple series (bulk)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueBulkAutoMatchRequest {
    /// Plugin ID to use for matching
    pub plugin_id: Uuid,

    /// Series IDs to auto-match
    pub series_ids: Vec<Uuid>,
}

/// Request to enqueue plugin auto-match tasks for all series in a library
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueLibraryAutoMatchRequest {
    /// Plugin ID to use for matching
    pub plugin_id: Uuid,
}

/// Response after enqueuing auto-match task(s)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueAutoMatchResponse {
    /// Whether the operation succeeded
    pub success: bool,

    /// Number of tasks enqueued
    pub tasks_enqueued: usize,

    /// Task IDs that were created
    pub task_ids: Vec<Uuid>,

    /// Message
    pub message: String,
}

// =============================================================================
// Conversions from Protocol Types
// =============================================================================

impl From<crate::services::plugin::protocol::SearchResult> for PluginSearchResultDto {
    fn from(r: crate::services::plugin::protocol::SearchResult) -> Self {
        Self {
            external_id: r.external_id,
            title: r.title,
            alternate_titles: r.alternate_titles,
            year: r.year,
            cover_url: r.cover_url,
            relevance_score: r.relevance_score,
            preview: r.preview.map(SearchResultPreviewDto::from),
        }
    }
}

impl From<crate::services::plugin::protocol::SearchResultPreview> for SearchResultPreviewDto {
    fn from(p: crate::services::plugin::protocol::SearchResultPreview) -> Self {
        Self {
            status: p.status,
            genres: p.genres,
            rating: p.rating,
            description: p.description,
            book_count: p.book_count,
            authors: p.authors,
            format: p.format,
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_create_plugin_request_defaults() {
        let json = json!({
            "name": "test",
            "displayName": "Test",
            "command": "node"
        });

        let request: CreatePluginRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "test");
        assert_eq!(request.plugin_type, "system");
        assert_eq!(request.credential_delivery, "init_message");
        assert!(request.args.is_empty());
        assert!(request.permissions.is_empty());
        assert!(request.scopes.is_empty());
        assert!(!request.enabled);
    }

    #[test]
    fn test_create_plugin_request_full() {
        let json = json!({
            "name": "mangabaka",
            "displayName": "MangaBaka",
            "description": "Manga metadata provider",
            "pluginType": "system",
            "command": "node",
            "args": ["/opt/plugins/mangabaka/dist/index.js"],
            "env": [{"key": "LOG_LEVEL", "value": "debug"}],
            "permissions": ["metadata:write:summary", "metadata:write:genres"],
            "scopes": ["series:detail"],
            "credentials": {"api_key": "secret"},
            "credentialDelivery": "both",
            "config": {"rate_limit": 60},
            "enabled": true
        });

        let request: CreatePluginRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "mangabaka");
        assert_eq!(request.args.len(), 1);
        assert_eq!(request.env.len(), 1);
        assert_eq!(request.env[0].key, "LOG_LEVEL");
        assert_eq!(request.permissions.len(), 2);
        assert_eq!(request.scopes.len(), 1);
        assert!(request.credentials.is_some());
        assert_eq!(request.credential_delivery, "both");
        assert!(request.enabled);
    }

    #[test]
    fn test_update_plugin_request_partial() {
        let json = json!({
            "displayName": "Updated Name",
            "permissions": ["metadata:write:*"]
        });

        let request: UpdatePluginRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.display_name, Some("Updated Name".to_string()));
        assert!(request.description.is_none());
        assert!(request.command.is_none());
        assert_eq!(
            request.permissions,
            Some(vec!["metadata:write:*".to_string()])
        );
    }

    #[test]
    fn test_parse_scope() {
        assert_eq!(
            parse_scope("series:detail"),
            Some(PluginScope::SeriesDetail)
        );
        assert_eq!(parse_scope("series:bulk"), Some(PluginScope::SeriesBulk));
        assert_eq!(parse_scope("invalid"), None);
    }

    #[test]
    fn test_parse_permission() {
        assert_eq!(
            parse_permission("metadata:write:summary"),
            Some(PluginPermission::MetadataWriteSummary)
        );
        assert_eq!(
            parse_permission("metadata:write:*"),
            Some(PluginPermission::MetadataWriteAll)
        );
        assert_eq!(parse_permission("invalid"), None);
    }

    #[test]
    fn test_available_permissions() {
        let perms = available_permissions();
        assert!(perms.contains(&"metadata:read"));
        assert!(perms.contains(&"metadata:write:*"));
        assert!(perms.contains(&"library:read"));
    }

    #[test]
    fn test_available_scopes() {
        let scopes = available_scopes();
        assert!(scopes.contains(&"series:detail"));
        assert!(scopes.contains(&"series:bulk"));
        assert!(scopes.contains(&"library:scan"));
    }

    #[test]
    fn test_plugin_test_result_serialization() {
        let result = PluginTestResult {
            success: true,
            message: "Connected successfully".to_string(),
            latency_ms: Some(150),
            manifest: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["latencyMs"], 150);
    }

    #[test]
    fn test_update_plugin_request_search_template_deserialization() {
        // Test 1: null value should deserialize as Some(Value::Null)
        let json1 = r#"{"searchQueryTemplate": null}"#;
        let parsed1: UpdatePluginRequest = serde_json::from_str(json1).unwrap();
        assert!(
            parsed1.search_query_template.is_some(),
            "null should deserialize as Some(Value::Null)"
        );
        assert!(
            parsed1.search_query_template.as_ref().unwrap().is_null(),
            "inner value should be null"
        );

        // Test 2: missing field should deserialize as None
        let json2 = r#"{}"#;
        let parsed2: UpdatePluginRequest = serde_json::from_str(json2).unwrap();
        assert!(
            parsed2.search_query_template.is_none(),
            "missing field should deserialize as None"
        );

        // Test 3: string value should deserialize as Some(Value::String)
        let json3 = r#"{"searchQueryTemplate": "{{title}}"}"#;
        let parsed3: UpdatePluginRequest = serde_json::from_str(json3).unwrap();
        assert!(parsed3.search_query_template.is_some());
        assert_eq!(
            parsed3.search_query_template.as_ref().unwrap().as_str(),
            Some("{{title}}")
        );

        // Test 4: empty string should deserialize as Some(Value::String(""))
        let json4 = r#"{"searchQueryTemplate": ""}"#;
        let parsed4: UpdatePluginRequest = serde_json::from_str(json4).unwrap();
        assert!(parsed4.search_query_template.is_some());
        assert_eq!(
            parsed4.search_query_template.as_ref().unwrap().as_str(),
            Some("")
        );
    }
}
