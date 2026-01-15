use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Setting response DTO
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct SettingDto {
    /// Setting unique identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// Setting key name
    #[schema(example = "scan.concurrent_jobs")]
    pub key: String,

    /// Current setting value
    #[schema(example = "4")]
    pub value: String,

    /// Data type of the value (string, integer, boolean, etc.)
    #[schema(example = "integer")]
    pub value_type: String,

    /// Category for grouping settings
    #[schema(example = "scanning")]
    pub category: String,

    /// Human-readable description
    #[schema(example = "Number of concurrent scanning jobs")]
    pub description: String,

    /// Whether value should be masked in responses
    #[schema(example = false)]
    pub is_sensitive: bool,

    /// Default value for this setting
    #[schema(example = "2")]
    pub default_value: String,

    /// Minimum allowed value (for numeric settings)
    #[schema(example = 1)]
    pub min_value: Option<i64>,

    /// Maximum allowed value (for numeric settings)
    #[schema(example = 16)]
    pub max_value: Option<i64>,

    /// When the setting was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,

    /// User who last updated the setting
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<Uuid>,

    /// Version number for optimistic locking
    #[schema(example = 1)]
    pub version: i32,
}

/// Update setting request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateSettingRequest {
    /// New value for the setting
    #[schema(example = "8")]
    pub value: String,

    /// Optional reason for the change (for audit log)
    #[schema(example = "Increased concurrency for faster scanning")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_reason: Option<String>,
}

/// Bulk update settings request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BulkUpdateSettingsRequest {
    /// List of settings to update
    pub updates: Vec<BulkSettingUpdate>,

    /// Optional reason for the changes (for audit log)
    #[schema(example = "Batch configuration update for production")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_reason: Option<String>,
}

/// Single setting update in a bulk operation
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BulkSettingUpdate {
    /// Setting key to update
    #[schema(example = "scan.concurrent_jobs")]
    pub key: String,

    /// New value for the setting
    #[schema(example = "4")]
    pub value: String,
}

/// Setting history entry DTO
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SettingHistoryDto {
    /// History entry ID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440001")]
    pub id: Uuid,

    /// ID of the setting that was changed
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub setting_id: Uuid,

    /// Setting key that was changed
    #[schema(example = "scan.concurrent_jobs")]
    pub key: String,

    /// Previous value before the change
    #[schema(example = "2")]
    pub old_value: String,

    /// New value after the change
    #[schema(example = "4")]
    pub new_value: String,

    /// User who made the change
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub changed_by: Uuid,

    /// When the change was made
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub changed_at: DateTime<Utc>,

    /// Reason provided for the change
    #[schema(example = "Performance optimization")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_reason: Option<String>,

    /// IP address of the user who made the change
    #[schema(example = "192.168.1.100")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
}

/// Query parameters for listing settings
#[derive(Debug, Deserialize, ToSchema)]
pub struct ListSettingsQuery {
    /// Filter settings by category
    #[schema(example = "scanning")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

/// Query parameters for setting history
#[derive(Debug, Deserialize, ToSchema)]
pub struct HistoryQuery {
    /// Maximum number of history entries to return
    #[schema(example = 50)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
}

/// Public setting DTO (for non-admin users)
///
/// A simplified setting DTO that only includes the key and value,
/// used for public display settings accessible to all authenticated users.
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct PublicSettingDto {
    /// Setting key name
    #[schema(example = "display.custom_metadata_template")]
    pub key: String,

    /// Current setting value
    #[schema(
        example = "{{#if custom_metadata}}## Additional Information\n{{#each custom_metadata}}- **{{@key}}**: {{this}}\n{{/each}}{{/if}}"
    )]
    pub value: String,
}
