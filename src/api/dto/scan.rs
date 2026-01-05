use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::scanner::{ScanProgress, ScanStatus};

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
    /// Auto-scan when library is created
    pub auto_scan_on_create: bool,
    /// Whether scheduled scanning is enabled
    pub enabled: bool,
}

impl Default for ScanningConfigDto {
    fn default() -> Self {
        Self {
            cron_schedule: None,
            scan_mode: "normal".to_string(),
            auto_scan_on_create: false,
            enabled: true,
        }
    }
}
