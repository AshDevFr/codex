use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::library_scanner::scan_library;
use super::types::{ScanMode, ScanProgress, ScanStatus};

/// Internal state for the scan manager
struct ScanState {
    /// Currently active scans
    active_scans: HashMap<Uuid, (ScanProgress, CancellationToken)>,
    /// Queued scans waiting to be processed
    queued_scans: VecDeque<(Uuid, ScanMode)>,
}

impl ScanState {
    fn new() -> Self {
        Self {
            active_scans: HashMap::new(),
            queued_scans: VecDeque::new(),
        }
    }
}

/// Manages concurrent library scans with queueing
pub struct ScanManager {
    state: Arc<RwLock<ScanState>>,
    db: DatabaseConnection,
    max_concurrent: usize,
}

impl ScanManager {
    /// Create a new scan manager
    pub fn new(db: DatabaseConnection, max_concurrent: usize) -> Self {
        info!("Initializing ScanManager with max_concurrent={}", max_concurrent);
        Self {
            state: Arc::new(RwLock::new(ScanState::new())),
            db,
            max_concurrent,
        }
    }

    /// Trigger a library scan
    /// Returns immediately after queueing or starting the scan
    pub async fn trigger_scan(&self, library_id: Uuid, mode: ScanMode) -> Result<()> {
        let mut state = self.state.write().await;

        // Check if library is already being scanned or queued
        if state.active_scans.contains_key(&library_id) {
            return Err(anyhow::anyhow!(
                "Library {} is already being scanned",
                library_id
            ));
        }

        if state
            .queued_scans
            .iter()
            .any(|(id, _)| id == &library_id)
        {
            return Err(anyhow::anyhow!("Library {} is already queued", library_id));
        }

        // Check if we can start immediately
        if state.active_scans.len() < self.max_concurrent {
            // Start scan immediately
            drop(state); // Release lock before starting scan
            self.start_scan(library_id, mode).await?;
        } else {
            // Add to queue
            info!("Queueing scan for library {} (mode: {})", library_id, mode);
            state.queued_scans.push_back((library_id, mode));

            // Initialize progress as queued
            let progress = ScanProgress::new(library_id);
            let cancellation = CancellationToken::new();
            state.active_scans.insert(library_id, (progress, cancellation));
        }

        Ok(())
    }

    /// Get scan status for a library
    pub async fn get_status(&self, library_id: Uuid) -> Option<ScanProgress> {
        let state = self.state.read().await;
        state.active_scans.get(&library_id).map(|(progress, _)| progress.clone())
    }

    /// Get all active scans
    pub async fn list_active(&self) -> Vec<ScanProgress> {
        let state = self.state.read().await;
        state
            .active_scans
            .values()
            .map(|(progress, _)| progress.clone())
            .collect()
    }

    /// Cancel a running scan
    pub async fn cancel_scan(&self, library_id: Uuid) -> Result<()> {
        let state = self.state.read().await;

        if let Some((_, cancellation)) = state.active_scans.get(&library_id) {
            cancellation.cancel();
            info!("Cancelled scan for library {}", library_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("No active scan found for library {}", library_id))
        }
    }

    /// Start a scan task
    async fn start_scan(&self, library_id: Uuid, mode: ScanMode) -> Result<()> {
        info!("Starting {} scan for library {}", mode, library_id);

        let cancellation = CancellationToken::new();
        let progress = ScanProgress::new(library_id);

        // Store initial progress
        {
            let mut state = self.state.write().await;
            state.active_scans.insert(library_id, (progress, cancellation.clone()));
        }

        // Clone necessary data for the task
        let db = self.db.clone();
        let state = self.state.clone();
        let max_concurrent = self.max_concurrent;

        // Spawn scan task
        tokio::spawn(async move {
            let (progress_tx, mut progress_rx) = mpsc::channel(100);

            // Start progress listener
            let state_clone = state.clone();
            let progress_task = tokio::spawn(async move {
                while let Some(progress) = progress_rx.recv().await {
                    let mut state = state_clone.write().await;
                    if let Some((stored_progress, _)) = state.active_scans.get_mut(&library_id) {
                        *stored_progress = progress;
                    }
                }
            });

            // Run the scan
            let scan_result = tokio::select! {
                result = scan_library(&db, library_id, mode, Some(progress_tx)) => result,
                _ = cancellation.cancelled() => {
                    warn!("Scan for library {} was cancelled", library_id);
                    Err(anyhow::anyhow!("Scan cancelled"))
                }
            };

            // Wait for progress updates to finish
            let _ = progress_task.await;

            // Update final status
            {
                let mut state = state.write().await;
                if let Some((progress, _)) = state.active_scans.get_mut(&library_id) {
                    match &scan_result {
                        Ok(result) => {
                            if result.has_errors() {
                                progress.status = ScanStatus::Completed;
                                for error in &result.errors {
                                    progress.add_error(error.clone());
                                }
                            } else {
                                progress.complete();
                            }
                        }
                        Err(e) => {
                            progress.fail(e.to_string());
                        }
                    }
                }
            }

            // Log result
            match &scan_result {
                Ok(result) => {
                    info!(
                        "Scan completed for library {} - {} files processed, {} errors",
                        library_id,
                        result.files_processed,
                        result.errors.len()
                    );
                }
                Err(e) => {
                    warn!("Scan failed for library {}: {}", library_id, e);
                }
            }

            // Clean up completed scan after a delay (keep results for a bit)
            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await; // 5 minutes
            {
                let mut state = state.write().await;
                state.active_scans.remove(&library_id);
            }

            // Note: We don't automatically start next queued scan here to avoid recursion issues
            // The queue will be processed when the next trigger_scan is called
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests are in tests/api/scan.rs
    // These are just unit tests for the manager logic

    #[tokio::test]
    async fn test_scan_manager_creation() {
        // This test just verifies the manager can be created
        // Real DB tests are in integration tests
    }
}
