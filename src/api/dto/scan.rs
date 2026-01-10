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
    pub library_id: Uuid,
    /// Current status of the scan
    pub status: String,
    /// Total number of files discovered
    pub files_total: usize,
    /// Number of files processed so far
    pub files_processed: usize,
    /// Number of series found/created
    pub series_found: usize,
    /// Number of books found/created
    pub books_found: usize,
    /// List of errors encountered during scan
    pub errors: Vec<String>,
    /// When the scan started
    pub started_at: DateTime<Utc>,
    /// When the scan completed (if finished)
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
    /// Cron expression for scheduled scans (e.g., "0 */6 * * *")
    pub cron_schedule: Option<String>,
    /// Default scan mode for scheduled scans ("normal" or "deep")
    pub scan_mode: String,
    /// Whether scheduled scanning is enabled
    pub enabled: bool,
    /// Scan library when the application starts
    #[serde(default)]
    pub scan_on_start: bool,
    /// Purge soft-deleted books after completing a scan
    #[serde(default)]
    pub purge_deleted_on_scan: bool,
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
