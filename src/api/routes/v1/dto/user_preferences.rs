//! User Preferences DTOs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::db::entities::user_preferences;
use crate::db::repositories::UserPreferencesRepository;

/// A single user preference
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferenceDto {
    /// The preference key (e.g., "ui.theme", "reader.zoom")
    #[schema(example = "ui.theme")]
    pub key: String,

    /// The preference value
    #[schema(example = json!("dark"))]
    pub value: serde_json::Value,

    /// The value type: string, integer, float, boolean, or json
    #[schema(example = "string")]
    pub value_type: String,

    /// When the preference was last updated
    #[schema(example = "2024-01-15T18:45:00Z")]
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<user_preferences::Model> for UserPreferenceDto {
    type Error = anyhow::Error;

    fn try_from(model: user_preferences::Model) -> Result<Self, Self::Error> {
        let value = UserPreferencesRepository::to_json_value(&model)?;
        Ok(Self {
            key: model.key,
            value,
            value_type: model.value_type,
            updated_at: model.updated_at,
        })
    }
}

/// Response containing all user preferences
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferencesResponse {
    /// List of all user preferences
    pub preferences: Vec<UserPreferenceDto>,
}

/// Request to set a single preference value
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetPreferenceRequest {
    /// The value to set
    #[schema(example = json!("dark"))]
    pub value: serde_json::Value,
}

/// Request to set multiple preferences at once
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkSetPreferencesRequest {
    /// Map of preference keys to values
    #[schema(example = json!({"ui.theme": "dark", "reader.zoom": 150}))]
    pub preferences: HashMap<String, serde_json::Value>,
}

/// Response after setting preferences
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetPreferencesResponse {
    /// Number of preferences that were updated
    #[schema(example = 3)]
    pub updated: usize,

    /// The updated preferences
    pub preferences: Vec<UserPreferenceDto>,
}

/// Response after deleting a preference
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeletePreferenceResponse {
    /// Whether a preference was deleted
    #[schema(example = true)]
    pub deleted: bool,

    /// Message describing the result
    #[schema(example = "Preference 'ui.theme' was reset to default")]
    pub message: String,
}
