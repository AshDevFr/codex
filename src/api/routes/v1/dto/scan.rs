use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::scanner::ScanProgress;

/// Scan status response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScanStatusDto {
    /// Library ID being scanned
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub library_id: Uuid,

    /// Current status of the scan (scanning, completed, failed)
    #[schema(example = "scanning")]
    pub status: String,

    /// Total number of files discovered
    #[schema(example = 1500)]
    pub files_total: usize,

    /// Number of files processed so far
    #[schema(example = 750)]
    pub files_processed: usize,

    /// Number of series found/created
    #[schema(example = 45)]
    pub series_found: usize,

    /// Number of books found/created
    #[schema(example = 750)]
    pub books_found: usize,

    /// List of errors encountered during scan
    pub errors: Vec<String>,

    /// When the scan started
    #[schema(example = "2024-01-15T10:30:00Z")]
    pub started_at: DateTime<Utc>,

    /// When the scan completed (if finished)
    #[schema(example = "2024-01-15T10:45:00Z")]
    pub completed_at: Option<DateTime<Utc>>,
}

impl From<ScanProgress> for ScanStatusDto {
    fn from(progress: ScanProgress) -> Self {
        Self {
            library_id: progress.library_id,
            status: progress.status.to_string(),
            files_total: progress.files_total,
            files_processed: progress.files_processed,
            series_found: progress.series_found,
            books_found: progress.books_found,
            errors: progress.errors,
            started_at: progress.started_at,
            completed_at: progress.completed_at,
        }
    }
}

/// Query parameters for triggering a scan
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TriggerScanQuery {
    /// Scan mode: "normal" or "deep" (defaults to "normal")
    #[schema(example = "normal")]
    #[serde(default = "default_scan_mode")]
    pub mode: String,
}

fn default_scan_mode() -> String {
    "normal".to_string()
}

/// Scanning configuration for a library
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScanningConfigDto {
    /// Cron expression for scheduled scans.
    /// Accepts standard 5-part Unix cron (e.g., "0 */6 * * *") or 6-part with seconds
    /// (e.g., "0 0 */6 * * *"). Stored as provided; normalization happens at scheduler level.
    #[schema(example = "0 */6 * * *")]
    pub cron_schedule: Option<String>,

    /// Default scan mode for scheduled scans ("normal" or "deep")
    #[schema(example = "normal")]
    pub scan_mode: String,

    /// Whether scheduled scanning is enabled
    #[schema(example = true)]
    pub enabled: bool,

    /// Scan library when the application starts
    #[serde(default)]
    #[schema(example = false)]
    pub scan_on_start: bool,

    /// Purge soft-deleted books after completing a scan
    #[serde(default)]
    #[schema(example = false)]
    pub purge_deleted_on_scan: bool,
}

impl ScanningConfigDto {
    /// Validate the cron expression without modifying it.
    ///
    /// Accepts both standard 5-part Unix cron (e.g., "0 */6 * * *") and 6-part
    /// with seconds (e.g., "0 0 */6 * * *"). The original expression is preserved
    /// as-is for storage; normalization to 6-part happens at scheduler level.
    pub fn validated(self) -> Result<Self, String> {
        if let Some(cron) = &self.cron_schedule {
            crate::utils::cron::validate_cron_expression(cron).map_err(|e| e.to_string())?;
        }
        Ok(self)
    }
}

impl Default for ScanningConfigDto {
    fn default() -> Self {
        Self {
            cron_schedule: None,
            scan_mode: "normal".to_string(),
            enabled: true,
            scan_on_start: false,
            purge_deleted_on_scan: false,
        }
    }
}

/// Analysis result response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResult {
    /// Number of books successfully analyzed
    #[schema(example = 150)]
    pub books_analyzed: usize,

    /// List of errors encountered during analysis
    pub errors: Vec<String>,
}

impl From<crate::scanner::AnalysisResult> for AnalysisResult {
    fn from(result: crate::scanner::AnalysisResult) -> Self {
        Self {
            books_analyzed: result.books_analyzed,
            errors: result.errors,
        }
    }
}

/// Request for bulk renumber operations on multiple series
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkRenumberSeriesRequest {
    /// List of series IDs to renumber
    #[schema(example = json!(["550e8400-e29b-41d4-a716-446655440001", "550e8400-e29b-41d4-a716-446655440002"]))]
    pub series_ids: Vec<Uuid>,
}
