use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::ScanningConfigDto;

/// Library data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDto {
    pub id: uuid::Uuid,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub scanning_config: Option<ScanningConfigDto>,
    pub last_scanned_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_count: Option<i64>,
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

    /// Scanning configuration
    pub scanning_config: Option<ScanningConfigDto>,
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

    /// Scanning configuration
    pub scanning_config: Option<ScanningConfigDto>,
}
