use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Scan mode determines how the scanner processes files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanMode {
    /// Only process new or changed files (by timestamp/hash)
    Normal,
    /// Re-process all files regardless of changes
    Deep,
}

impl fmt::Display for ScanMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanMode::Normal => write!(f, "normal"),
            ScanMode::Deep => write!(f, "deep"),
        }
    }
}

impl ScanMode {
    /// Parse scan mode from string
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "normal" => Ok(ScanMode::Normal),
            "deep" => Ok(ScanMode::Deep),
            _ => Err(format!("Invalid scan mode: {}", s)),
        }
    }
}

/// Current status of a scan operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanStatus {
    /// Scan is pending but not yet started
    Pending,
    /// Scan is currently running
    Running,
    /// Scan completed successfully
    Completed,
    /// Scan failed with errors
    Failed,
    /// Scan was cancelled by user
    Cancelled,
}

impl fmt::Display for ScanStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanStatus::Pending => write!(f, "pending"),
            ScanStatus::Running => write!(f, "running"),
            ScanStatus::Completed => write!(f, "completed"),
            ScanStatus::Failed => write!(f, "failed"),
            ScanStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Progress information for an ongoing or completed scan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    /// ID of the library being scanned
    pub library_id: Uuid,
    /// Current status of the scan
    pub status: ScanStatus,
    /// Total number of files discovered
    pub files_total: usize,
    /// Number of files processed so far
    pub files_processed: usize,
    /// Number of series found/created
    pub series_found: usize,
    /// Number of books found/created
    pub books_found: usize,
    /// List of error messages encountered during scan
    pub errors: Vec<String>,
    /// When the scan started
    pub started_at: DateTime<Utc>,
    /// When the scan completed (if finished)
    pub completed_at: Option<DateTime<Utc>>,
}

impl ScanProgress {
    /// Create new scan progress in pending state
    pub fn new(library_id: Uuid) -> Self {
        Self {
            library_id,
            status: ScanStatus::Pending,
            files_total: 0,
            files_processed: 0,
            series_found: 0,
            books_found: 0,
            errors: Vec::new(),
            started_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Mark scan as running
    pub fn start(&mut self) {
        self.status = ScanStatus::Running;
        self.started_at = Utc::now();
    }

    /// Mark scan as completed
    pub fn complete(&mut self) {
        self.status = ScanStatus::Completed;
        self.completed_at = Some(Utc::now());
    }

    /// Mark scan as failed
    pub fn fail(&mut self, error: String) {
        self.status = ScanStatus::Failed;
        self.errors.push(error);
        self.completed_at = Some(Utc::now());
    }

    /// Mark scan as cancelled
    pub fn cancel(&mut self) {
        self.status = ScanStatus::Cancelled;
        self.completed_at = Some(Utc::now());
    }

    /// Add an error without failing the scan
    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }

    /// Update progress counts
    pub fn update_progress(&mut self, files_processed: usize, files_total: usize) {
        self.files_processed = files_processed;
        self.files_total = files_total;
    }

    /// Increment series count
    pub fn increment_series(&mut self) {
        self.series_found += 1;
    }

    /// Increment books count
    pub fn increment_books(&mut self) {
        self.books_found += 1;
    }
}

/// Final result of a completed scan operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    /// Number of files processed during scan
    pub files_processed: usize,
    /// Number of series created
    pub series_created: usize,
    /// Number of books created
    pub books_created: usize,
    /// Number of books updated
    pub books_updated: usize,
    /// Number of books marked as deleted (missing from filesystem)
    pub books_deleted: usize,
    /// Number of books restored (deleted books that reappeared)
    pub books_restored: usize,
    /// List of errors encountered
    pub errors: Vec<String>,
}

impl ScanResult {
    /// Create new empty scan result
    pub fn new() -> Self {
        Self {
            files_processed: 0,
            series_created: 0,
            books_created: 0,
            books_updated: 0,
            books_deleted: 0,
            books_restored: 0,
            errors: Vec::new(),
        }
    }

    /// Check if scan had any errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

impl Default for ScanResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Scanning configuration stored in library's scanning_config JSON field
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanningConfig {
    /// Cron expression for scheduled scans (e.g., "0 */6 * * *")
    pub cron_schedule: Option<String>,
    /// Default scan mode for scheduled scans ("normal" or "deep")
    #[serde(default = "default_scan_mode")]
    pub scan_mode: String,
    /// Auto-scan when library is created
    #[serde(default)]
    pub auto_scan_on_create: bool,
    /// Whether scheduled scanning is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Scan library when the application starts
    #[serde(default)]
    pub scan_on_start: bool,
    /// Purge soft-deleted books after completing a scan
    #[serde(default)]
    pub purge_deleted_on_scan: bool,
}

fn default_scan_mode() -> String {
    "normal".to_string()
}

fn default_enabled() -> bool {
    true
}

impl ScanningConfig {
    /// Parse scan mode from config
    pub fn get_scan_mode(&self) -> Result<ScanMode> {
        ScanMode::from_str(&self.scan_mode).map_err(|e| anyhow::anyhow!(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_mode_from_str() {
        assert_eq!(ScanMode::from_str("normal").unwrap(), ScanMode::Normal);
        assert_eq!(ScanMode::from_str("deep").unwrap(), ScanMode::Deep);
        assert_eq!(ScanMode::from_str("NORMAL").unwrap(), ScanMode::Normal);
        assert_eq!(ScanMode::from_str("DEEP").unwrap(), ScanMode::Deep);
        assert!(ScanMode::from_str("invalid").is_err());
    }

    #[test]
    fn test_scan_mode_display() {
        assert_eq!(ScanMode::Normal.to_string(), "normal");
        assert_eq!(ScanMode::Deep.to_string(), "deep");
    }

    #[test]
    fn test_scan_status_display() {
        assert_eq!(ScanStatus::Pending.to_string(), "pending");
        assert_eq!(ScanStatus::Running.to_string(), "running");
        assert_eq!(ScanStatus::Completed.to_string(), "completed");
        assert_eq!(ScanStatus::Failed.to_string(), "failed");
        assert_eq!(ScanStatus::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn test_scan_progress_lifecycle() {
        let library_id = Uuid::new_v4();
        let mut progress = ScanProgress::new(library_id);

        assert_eq!(progress.status, ScanStatus::Pending);
        assert_eq!(progress.library_id, library_id);

        progress.start();
        assert_eq!(progress.status, ScanStatus::Running);

        progress.update_progress(5, 10);
        assert_eq!(progress.files_processed, 5);
        assert_eq!(progress.files_total, 10);

        progress.increment_series();
        progress.increment_books();
        assert_eq!(progress.series_found, 1);
        assert_eq!(progress.books_found, 1);

        progress.complete();
        assert_eq!(progress.status, ScanStatus::Completed);
        assert!(progress.completed_at.is_some());
    }

    #[test]
    fn test_scan_progress_error_handling() {
        let mut progress = ScanProgress::new(Uuid::new_v4());

        progress.add_error("Test error".to_string());
        assert_eq!(progress.errors.len(), 1);
        assert_eq!(progress.status, ScanStatus::Pending); // Still pending

        progress.fail("Fatal error".to_string());
        assert_eq!(progress.status, ScanStatus::Failed);
        assert_eq!(progress.errors.len(), 2);
        assert!(progress.completed_at.is_some());
    }

    #[test]
    fn test_scan_result_default() {
        let result = ScanResult::default();
        assert_eq!(result.files_processed, 0);
        assert!(!result.has_errors());
    }

    #[test]
    fn test_scanning_config_parsing() {
        // Test with camelCase (as stored in database)
        let json = r#"{
            "cronSchedule": "0 */6 * * *",
            "scanMode": "normal",
            "autoScanOnCreate": true,
            "enabled": true,
            "scanOnStart": true,
            "purgeDeletedOnScan": true
        }"#;

        let config: ScanningConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.cron_schedule, Some("0 */6 * * *".to_string()));
        assert_eq!(config.scan_mode, "normal");
        assert!(config.auto_scan_on_create);
        assert!(config.enabled);
        assert!(config.scan_on_start);
        assert!(config.purge_deleted_on_scan);
        assert_eq!(config.get_scan_mode().unwrap(), ScanMode::Normal);
    }

    #[test]
    fn test_scanning_config_defaults() {
        let json = r#"{}"#;

        let config: ScanningConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.scan_mode, "normal");
        assert!(!config.auto_scan_on_create);
        assert!(config.enabled);
        assert!(!config.scan_on_start);
        assert!(!config.purge_deleted_on_scan);
    }

    #[test]
    fn test_scanning_config_deep_mode() {
        let json = r#"{
            "scanMode": "deep"
        }"#;

        let config: ScanningConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.get_scan_mode().unwrap(), ScanMode::Deep);
    }

    #[test]
    fn test_scanning_config_database_format() {
        // Test with exact format as stored in database (from user report)
        let json = r#"{
            "cronSchedule": null,
            "scanMode": "normal",
            "autoScanOnCreate": true,
            "enabled": false,
            "scanOnStart": false,
            "purgeDeletedOnScan": true
        }"#;

        let config: ScanningConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.cron_schedule, None);
        assert_eq!(config.scan_mode, "normal");
        assert!(config.auto_scan_on_create);
        assert!(!config.enabled);
        assert!(!config.scan_on_start);
        assert!(
            config.purge_deleted_on_scan,
            "purgeDeletedOnScan should be true"
        );
    }
}
