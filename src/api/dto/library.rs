use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::ScanningConfigDto;

/// Library data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDto {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: uuid::Uuid,
    #[schema(example = "Comics")]
    pub name: String,
    #[schema(example = "/media/comics")]
    pub path: String,
    #[schema(example = "My comic book collection")]
    pub description: Option<String>,
    #[schema(example = true)]
    pub is_active: bool,
    pub scanning_config: Option<ScanningConfigDto>,
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub last_scanned_at: Option<DateTime<Utc>>,
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 1250)]
    pub book_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 85)]
    pub series_count: Option<i64>,
    /// Allowed file formats (e.g., ["CBZ", "CBR", "EPUB"])
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["CBZ", "CBR", "PDF"]))]
    pub allowed_formats: Option<Vec<String>>,
    /// Excluded path patterns (newline-separated, e.g., ".DS_Store\nThumbs.db")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = ".DS_Store\nThumbs.db")]
    pub excluded_patterns: Option<String>,
}

/// Create library request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateLibraryRequest {
    /// Library name
    #[schema(example = "Comics")]
    pub name: String,

    /// Filesystem path to the library
    #[schema(example = "/media/comics")]
    pub path: String,

    /// Optional description
    #[schema(example = "My comic book collection")]
    pub description: Option<String>,

    /// Scanning configuration
    pub scanning_config: Option<ScanningConfigDto>,

    /// Scan immediately after creation (not stored in DB)
    #[serde(default, skip_serializing_if = "is_false")]
    #[schema(example = true)]
    pub scan_immediately: bool,

    /// Allowed file formats (e.g., ["CBZ", "CBR", "EPUB"])
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["CBZ", "CBR", "EPUB"]))]
    pub allowed_formats: Option<Vec<String>>,

    /// Excluded path patterns (newline-separated, e.g., ".DS_Store\nThumbs.db")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = ".DS_Store\nThumbs.db")]
    pub excluded_patterns: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !b
}

/// Update library request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLibraryRequest {
    /// Library name
    #[schema(example = "Comics Collection")]
    pub name: Option<String>,

    /// Filesystem path to the library
    #[schema(example = "/media/comics")]
    pub path: Option<String>,

    /// Optional description
    #[schema(example = "Updated comic book collection")]
    pub description: Option<String>,

    /// Active status
    #[schema(example = true)]
    pub is_active: Option<bool>,

    /// Scanning configuration
    pub scanning_config: Option<ScanningConfigDto>,

    /// Allowed file formats (e.g., ["CBZ", "CBR", "EPUB"])
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = json!(["CBZ", "CBR"]))]
    pub allowed_formats: Option<Vec<String>>,

    /// Excluded path patterns (newline-separated, e.g., ".DS_Store\nThumbs.db")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = ".DS_Store")]
    pub excluded_patterns: Option<String>,
}
