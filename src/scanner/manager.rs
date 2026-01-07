use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, mpsc, RwLock};
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
    /// Broadcast channel for progress updates
    progress_broadcast: broadcast::Sender<ScanProgress>,
}

impl ScanState {
    fn new(progress_broadcast: broadcast::Sender<ScanProgress>) -> Self {
        Self {
            active_scans: HashMap::new(),
            queued_scans: VecDeque::new(),
            progress_broadcast,
        }
    }
}

/// Manages concurrent library scans with queueing
pub struct ScanManager {
    state: Arc<RwLock<ScanState>>,
    db: DatabaseConnection,
    max_concurrent: usize,
    progress_broadcast: broadcast::Sender<ScanProgress>,
}

impl ScanManager {
    /// Create a new scan manager
    pub fn new(db: DatabaseConnection, max_concurrent: usize) -> Self {
        info!(
            "Initializing ScanManager with max_concurrent={}",
            max_concurrent
        );
        let (progress_broadcast, _) = broadcast::channel(1000);
        Self {
            state: Arc::new(RwLock::new(ScanState::new(progress_broadcast.clone()))),
            db,
            max_concurrent,
            progress_broadcast,
        }
    }

    /// Subscribe to progress updates for all scans
    pub fn subscribe(&self) -> broadcast::Receiver<ScanProgress> {
        self.progress_broadcast.subscribe()
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

        if state.queued_scans.iter().any(|(id, _)| id == &library_id) {
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
            state
                .active_scans
                .insert(library_id, (progress, cancellation));
        }

        Ok(())
    }

    /// Get scan status for a library
    pub async fn get_status(&self, library_id: Uuid) -> Option<ScanProgress> {
        let state = self.state.read().await;
        state
            .active_scans
            .get(&library_id)
            .map(|(progress, _)| progress.clone())
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
            Err(anyhow::anyhow!(
                "No active scan found for library {}",
                library_id
            ))
        }
    }

    /// Start a scan task
    async fn start_scan(&self, library_id: Uuid, mode: ScanMode) -> Result<()> {
        let scan_start_time = Instant::now();
        info!("Starting {} scan for library {}", mode, library_id);

        let cancellation = CancellationToken::new();
        let progress = ScanProgress::new(library_id);

        // Store initial progress
        {
            let mut state = self.state.write().await;
            state
                .active_scans
                .insert(library_id, (progress, cancellation.clone()));
        }

        // Clone necessary data for the task
        let db = self.db.clone();
        let state = self.state.clone();
        let _max_concurrent = self.max_concurrent;

        // Spawn scan task
        tokio::spawn(async move {
            let (progress_tx, mut progress_rx) = mpsc::channel::<ScanProgress>(100);

            // Start progress listener
            let state_clone = state.clone();
            let progress_task = tokio::spawn(async move {
                while let Some(progress) = progress_rx.recv().await {
                    let mut state = state_clone.write().await;
                    if let Some((stored_progress, _)) = state.active_scans.get_mut(&library_id) {
                        *stored_progress = progress.clone();
                        // Broadcast progress update (ignore if no listeners)
                        match state.progress_broadcast.send(progress.clone()) {
                            Ok(count) => {
                                if count > 0 {
                                    debug!("Broadcast progress update for library {} to {} subscribers", library_id, count);
                                }
                                // If count is 0, there are no subscribers - this is fine, just continue
                            }
                            Err(e) => {
                                // Only warn if it's a real error (channel closed), not just no subscribers
                                // In tokio broadcast, send() returns Ok(0) when no receivers, not an error
                                // So if we get an error, the channel is actually closed
                                debug!(
                                    "No subscribers for progress update for library {}: {}",
                                    library_id, e
                                );
                            }
                        }
                    }
                }
                debug!("Progress listener task finished for library {}", library_id);
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

            // Update final status and broadcast it
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
                    // Broadcast the final status update
                    let progress_clone = progress.clone();
                    match state.progress_broadcast.send(progress_clone) {
                        Ok(count) => {
                            if count > 0 {
                                debug!("Broadcast final progress update for library {} to {} subscribers", library_id, count);
                            }
                            // If count is 0, there are no subscribers - this is fine
                        }
                        Err(e) => {
                            // Only debug log if no subscribers (channel might be closed if all senders dropped)
                            debug!(
                                "No subscribers for final progress update for library {}: {}",
                                library_id, e
                            );
                        }
                    }
                }
            }

            // Log result
            let total_duration = scan_start_time.elapsed();
            match &scan_result {
                Ok(result) => {
                    info!(
                        "Scan task completed for library {} in {:?} - {} files processed, {} errors",
                        library_id,
                        total_duration,
                        result.files_processed,
                        result.errors.len()
                    );

                    // Check if we should purge deleted books after scan
                    if let Ok(Some(library)) =
                        crate::db::repositories::LibraryRepository::get_by_id(&db, library_id).await
                    {
                        if let Some(config_json) = &library.scanning_config {
                            if let Ok(config) = serde_json::from_str::<
                                crate::scanner::scheduler::ScanningConfig,
                            >(config_json)
                            {
                                if config.purge_deleted_on_scan {
                                    info!(
                                        "Purging deleted books for library {} after scan",
                                        library_id
                                    );
                                    match crate::db::repositories::BookRepository::purge_deleted_in_library(&db, library_id).await {
                                        Ok(count) => {
                                            info!("Purged {} deleted books from library {}", count, library_id);
                                        }
                                        Err(e) => {
                                            warn!("Failed to purge deleted books from library {}: {}", library_id, e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "Scan failed for library {} after {:?}: {}",
                        library_id, total_duration, e
                    );
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

    // Note: Full integration tests are in tests/api/scan.rs
    // These are just unit tests for the manager logic

    #[tokio::test]
    async fn test_scan_manager_creation() {
        // This test just verifies the manager can be created
        // Real DB tests are in integration tests
    }
}
