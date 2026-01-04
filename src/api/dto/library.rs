use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Library data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDto {
    pub id: uuid::Uuid,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub last_scanned_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Create library request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateLibraryRequest {
    /// Library name
    pub name: String,

    /// Filesystem path to the library
    pub path: String,

    /// Optional description
    pub description: Option<String>,
}

/// Update library request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLibraryRequest {
    /// Library name
    pub name: Option<String>,

    /// Filesystem path to the library
    pub path: Option<String>,

    /// Optional description
    pub description: Option<String>,

    /// Active status
    pub is_active: Option<bool>,
}
