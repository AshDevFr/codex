use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::patch::PatchValue;
use super::ScanningConfigDto;
use crate::models::{BookStrategy, NumberStrategy, SeriesStrategy};

/// Library data transfer object
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDto {
    /// Library unique identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: uuid::Uuid,

    /// Library name
    #[schema(example = "Comics")]
    pub name: String,

    /// Filesystem path to the library root
    #[schema(example = "/media/comics")]
    pub path: String,

    /// Optional description
    #[schema(example = "My comic book collection")]
    pub description: Option<String>,

    /// Whether the library is active
    #[schema(example = true)]
    pub is_active: bool,

    /// Series detection strategy (series_volume, series_volume_chapter, flat, etc.)
    #[schema(example = "series_volume")]
    pub series_strategy: SeriesStrategy,

    /// Strategy-specific configuration (JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_config: Option<serde_json::Value>,

    /// Book naming strategy (filename, metadata_first, smart, series_name)
    #[schema(example = "filename")]
    pub book_strategy: BookStrategy,

    /// Book strategy-specific configuration (JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_config: Option<serde_json::Value>,

    /// Book number strategy (file_order, metadata, filename, smart)
    #[schema(example = "file_order")]
    pub number_strategy: NumberStrategy,

    /// Number strategy-specific configuration (JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_config: Option<serde_json::Value>,

    /// Scanning configuration for scheduled scans
    pub scanning_config: Option<ScanningConfigDto>,

    /// When the library was last scanned
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub last_scanned_at: Option<DateTime<Utc>>,

    /// When the library was created
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// When the library was last updated
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub updated_at: DateTime<Utc>,

    /// Total number of books in this library
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 1250)]
    pub book_count: Option<i64>,

    /// Total number of series in this library
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

    /// Default reading direction for books in this library (ltr, rtl, ttb or webtoon)
    #[schema(example = "ltr")]
    pub default_reading_direction: String,

    /// Title preprocessing rules (JSON array of regex rules)
    /// Applied during scan to clean series titles before metadata search
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_preprocessing_rules: Option<serde_json::Value>,

    /// Auto-match conditions (JSON object with mode and rules)
    /// Controls when auto-matching runs for this library
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_match_conditions: Option<serde_json::Value>,
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

    /// Series detection strategy (immutable after creation)
    /// Options: series_volume, series_volume_chapter, flat, publisher_hierarchy, calibre, custom
    #[serde(default)]
    #[schema(example = "series_volume")]
    pub series_strategy: Option<SeriesStrategy>,

    /// Strategy-specific configuration (JSON, immutable after creation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_config: Option<serde_json::Value>,

    /// Book naming strategy (mutable after creation)
    /// Options: filename, metadata_first, smart, series_name
    #[serde(default)]
    #[schema(example = "filename")]
    pub book_strategy: Option<BookStrategy>,

    /// Book strategy-specific configuration (JSON, mutable after creation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_config: Option<serde_json::Value>,

    /// Book number strategy (mutable after creation)
    /// Options: file_order, metadata, filename, smart
    #[serde(default)]
    #[schema(example = "file_order")]
    pub number_strategy: Option<NumberStrategy>,

    /// Number strategy-specific configuration (JSON, mutable after creation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_config: Option<serde_json::Value>,

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

    /// Default reading direction for books in this library (ltr, rtl, ttb or webtoon)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "ltr")]
    pub default_reading_direction: Option<String>,

    /// Title preprocessing rules (JSON array of regex rules)
    /// Applied during scan to clean series titles before metadata search
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_preprocessing_rules: Option<serde_json::Value>,

    /// Auto-match conditions (JSON object with mode and rules)
    /// Controls when auto-matching runs for this library
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_match_conditions: Option<serde_json::Value>,
}

fn is_false(b: &bool) -> bool {
    !b
}

/// Update library request
///
/// Note: series_strategy and series_config are immutable after library creation.
/// book_strategy, book_config, number_strategy, and number_config can be updated.
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

    /// Book naming strategy (mutable)
    /// Options: filename, metadata_first, smart, series_name
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "filename")]
    pub book_strategy: Option<BookStrategy>,

    /// Book strategy-specific configuration (JSON, mutable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_config: Option<serde_json::Value>,

    /// Book number strategy (mutable)
    /// Options: file_order, metadata, filename, smart
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "file_order")]
    pub number_strategy: Option<NumberStrategy>,

    /// Number strategy-specific configuration (JSON, mutable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_config: Option<serde_json::Value>,

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

    /// Default reading direction for books in this library (ltr, rtl, ttb or webtoon)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "rtl")]
    pub default_reading_direction: Option<String>,

    /// Title preprocessing rules (JSON array of regex rules)
    /// Applied during scan to clean series titles before metadata search.
    /// - Omit field: keep existing value
    /// - Set to null: clear the rules
    /// - Set to value: update the rules
    #[serde(default)]
    #[schema(value_type = Option<Object>, nullable = true)]
    pub title_preprocessing_rules: PatchValue<serde_json::Value>,

    /// Auto-match conditions (JSON object with mode and rules)
    /// Controls when auto-matching runs for this library.
    /// - Omit field: keep existing value
    /// - Set to null: clear the conditions
    /// - Set to value: update the conditions
    #[serde(default)]
    #[schema(value_type = Option<Object>, nullable = true)]
    pub auto_match_conditions: PatchValue<serde_json::Value>,
}

/// Preview scan request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreviewScanRequest {
    /// Filesystem path to scan
    #[schema(example = "/media/comics")]
    pub path: String,

    /// Series detection strategy to use
    #[serde(default)]
    #[schema(example = "series_volume")]
    pub series_strategy: Option<SeriesStrategy>,

    /// Strategy-specific configuration (JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_config: Option<serde_json::Value>,
}

/// Preview scan response showing detected series
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreviewScanResponse {
    /// List of detected series
    pub detected_series: Vec<DetectedSeriesDto>,

    /// Total number of files found
    pub total_files: usize,
}

/// Detected series information for preview
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectedSeriesDto {
    /// Series name as detected
    pub name: String,

    /// Path relative to library root
    pub path: Option<String>,

    /// Number of books detected
    pub book_count: usize,

    /// Sample book filenames (first 5)
    pub sample_books: Vec<String>,

    /// Metadata extracted during detection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<DetectedSeriesMetadataDto>,
}

/// Metadata extracted during series detection
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectedSeriesMetadataDto {
    /// Publisher (for publisher_hierarchy strategy)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,

    /// Author (for calibre strategy)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}
