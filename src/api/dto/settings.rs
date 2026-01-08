use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Setting response DTO
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct SettingDto {
    pub id: Uuid,
    pub key: String,
    pub value: String,
    pub value_type: String,
    pub category: String,
    pub description: String,
    pub is_sensitive: bool,
    pub default_value: String,
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<Uuid>,
    pub version: i32,
}

/// Update setting request
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSettingRequest {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_reason: Option<String>,
}

/// Bulk update settings request
#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkUpdateSettingsRequest {
    pub updates: Vec<BulkSettingUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_reason: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkSettingUpdate {
    pub key: String,
    pub value: String,
}

/// Setting history entry DTO
#[derive(Debug, Serialize, ToSchema)]
pub struct SettingHistoryDto {
    pub id: Uuid,
    pub setting_id: Uuid,
    pub key: String,
    pub old_value: String,
    pub new_value: String,
    pub changed_by: Uuid,
    pub changed_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
}

/// Query parameters for listing settings
#[derive(Debug, Deserialize, ToSchema)]
pub struct ListSettingsQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}
