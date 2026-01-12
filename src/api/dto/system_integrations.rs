//! System Integrations DTOs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::entities::system_integrations;
use crate::db::repositories::SystemIntegrationsRepository;

/// A system integration (credentials are never exposed)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SystemIntegrationDto {
    /// Integration ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// Unique integration name (e.g., "mangaupdates", "anilist")
    #[schema(example = "mangaupdates")]
    pub name: String,

    /// Human-readable display name
    #[schema(example = "MangaUpdates")]
    pub display_name: String,

    /// Integration type: metadata_provider, notification, storage, sync
    #[schema(example = "metadata_provider")]
    pub integration_type: String,

    /// Non-sensitive configuration
    #[schema(example = json!({"rate_limit_per_minute": 60}))]
    pub config: serde_json::Value,

    /// Whether credentials have been set (actual credentials are never returned)
    #[schema(example = true)]
    pub has_credentials: bool,

    /// Whether the integration is enabled
    #[schema(example = true)]
    pub enabled: bool,

    /// Health status: unknown, healthy, degraded, unhealthy, disabled
    #[schema(example = "healthy")]
    pub health_status: String,

    /// When the last health check was performed
    #[schema(example = "2024-01-15T18:45:00Z")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_health_check_at: Option<DateTime<Utc>>,

    /// When the integration last synced data
    #[schema(example = "2024-01-15T18:00:00Z")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_at: Option<DateTime<Utc>>,

    /// Error message if health check failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,

    /// When the integration was created
    pub created_at: DateTime<Utc>,

    /// When the integration was last updated
    pub updated_at: DateTime<Utc>,
}

impl From<system_integrations::Model> for SystemIntegrationDto {
    fn from(model: system_integrations::Model) -> Self {
        let has_credentials = SystemIntegrationsRepository::has_credentials(&model);
        Self {
            id: model.id,
            name: model.name,
            display_name: model.display_name,
            integration_type: model.integration_type,
            config: model.config,
            has_credentials,
            enabled: model.enabled,
            health_status: model.health_status,
            last_health_check_at: model.last_health_check_at,
            last_sync_at: model.last_sync_at,
            error_message: model.error_message,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

/// Response containing a list of system integrations
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SystemIntegrationsListResponse {
    /// List of system integrations
    pub integrations: Vec<SystemIntegrationDto>,

    /// Total count
    pub total: usize,
}

/// Request to create a new system integration
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSystemIntegrationRequest {
    /// Unique integration name (alphanumeric with underscores)
    #[schema(example = "mangaupdates")]
    pub name: String,

    /// Human-readable display name
    #[schema(example = "MangaUpdates")]
    pub display_name: String,

    /// Integration type
    #[schema(example = "metadata_provider")]
    pub integration_type: String,

    /// Credentials (will be encrypted before storage)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!({"api_key": "your-api-key"}))]
    pub credentials: Option<serde_json::Value>,

    /// Non-sensitive configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!({"rate_limit_per_minute": 60}))]
    pub config: Option<serde_json::Value>,

    /// Whether to enable immediately
    #[serde(default)]
    #[schema(example = false)]
    pub enabled: bool,
}

/// Request to update a system integration
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSystemIntegrationRequest {
    /// Updated display name
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "MangaUpdates API")]
    pub display_name: Option<String>,

    /// Updated credentials (will be encrypted before storage)
    /// Set to null to clear credentials, omit to keep unchanged
    #[serde(default)]
    #[schema(example = json!({"api_key": "new-api-key"}))]
    pub credentials: Option<serde_json::Value>,

    /// Updated configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!({"rate_limit_per_minute": 30}))]
    pub config: Option<serde_json::Value>,
}

/// Response from testing an integration connection
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationTestResult {
    /// Whether the test was successful
    #[schema(example = true)]
    pub success: bool,

    /// Test result message
    #[schema(example = "Successfully connected to MangaUpdates API")]
    pub message: String,

    /// Response latency in milliseconds (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 150)]
    pub latency_ms: Option<u64>,
}

/// Response after enabling or disabling an integration
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationStatusResponse {
    /// The updated integration
    pub integration: SystemIntegrationDto,

    /// Status change message
    #[schema(example = "Integration enabled successfully")]
    pub message: String,
}
