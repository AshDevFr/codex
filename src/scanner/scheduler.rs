use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::repositories::{LibraryRepository, TaskRepository};
use crate::tasks::types::TaskType;

use super::types::ScanMode;

/// Scanning configuration stored in library's scanning_config JSON field
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Manages scheduled scans using cron expressions
pub struct ScanScheduler {
    scheduler: JobScheduler,
    db: DatabaseConnection,
}

impl ScanScheduler {
    /// Create a new scan scheduler
    pub async fn new(db: DatabaseConnection) -> Result<Self> {
        let scheduler = JobScheduler::new()
            .await
            .context("Failed to create job scheduler")?;

        Ok(Self { scheduler, db })
    }

    /// Start the scheduler and load all library schedules
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting scan scheduler");

        // Load all libraries
        let libraries = LibraryRepository::list_all(&self.db).await?;

        for library in libraries {
            // Add scheduled scans
            if let Err(e) = self.add_library_schedule(library.id).await {
                warn!("Failed to add schedule for library {}: {}", library.name, e);
            }

            // Trigger scan-on-start if configured
            if let Some(config_json) = &library.scanning_config {
                if let Ok(config) = serde_json::from_str::<ScanningConfig>(config_json) {
                    if config.scan_on_start {
                        info!("Triggering scan-on-start for library {}", library.name);
                        let scan_mode = config.get_scan_mode().unwrap_or(ScanMode::Normal);

                        let task_type = TaskType::ScanLibrary {
                            library_id: library.id,
                            mode: scan_mode.to_string(),
                        };

                        if let Err(e) = TaskRepository::enqueue(&self.db, task_type, 0, None).await
                        {
                            warn!(
                                "Failed to trigger scan-on-start for library {}: {}",
                                library.name, e
                            );
                        }
                    }
                }
            }
        }

        // Start the scheduler
        self.scheduler
            .start()
            .await
            .context("Failed to start scheduler")?;

        let job_count = if self.scheduler.time_till_next_job().await.is_ok() {
            1
        } else {
            0
        };
        info!("Scan scheduler started with {} jobs", job_count);

        Ok(())
    }

    /// Reload all schedules (useful when libraries are updated)
    pub async fn reload_schedules(&mut self) -> Result<()> {
        info!("Reloading all schedules");

        // Remove all existing jobs
        self.scheduler.shutdown().await?;
        self.scheduler = JobScheduler::new().await?;

        // Reload
        self.start().await
    }

    /// Add or update a library's schedule
    pub async fn add_library_schedule(&mut self, library_id: Uuid) -> Result<()> {
        // Load library from database
        let library = LibraryRepository::get_by_id(&self.db, library_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Library not found: {}", library_id))?;

        // Parse scanning config
        let config: Option<ScanningConfig> = library
            .scanning_config
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok());

        // Skip if no config or not enabled
        let config = match config {
            Some(c) if c.enabled => c,
            _ => {
                debug!("Skipping library {} - scanning not enabled", library.name);
                return Ok(());
            }
        };

        // Skip if no cron schedule
        let cron_schedule = match &config.cron_schedule {
            Some(schedule) => schedule.clone(),
            None => {
                debug!("Skipping library {} - no cron schedule", library.name);
                return Ok(());
            }
        };

        // Parse scan mode
        let scan_mode = config.get_scan_mode()?;

        // Create cron job
        let db = self.db.clone();
        let library_name = library.name.clone();

        let job = Job::new_async(cron_schedule.as_str(), move |_uuid, _lock| {
            let db = db.clone();
            let library_name = library_name.clone();
            let mode_str = scan_mode.to_string();

            Box::pin(async move {
                info!(
                    "Triggering scheduled {} scan for library {}",
                    mode_str, library_name
                );

                let task_type = TaskType::ScanLibrary {
                    library_id,
                    mode: mode_str.clone(),
                };

                match TaskRepository::enqueue(&db, task_type, 0, None).await {
                    Ok(_) => {
                        debug!("Successfully triggered scan for library {}", library_name);
                    }
                    Err(e) => {
                        error!(
                            "Failed to trigger scheduled scan for library {}: {}",
                            library_name, e
                        );
                    }
                }
            })
        })
        .context("Failed to create cron job")?;

        self.scheduler
            .add(job)
            .await
            .context("Failed to add job to scheduler")?;

        info!(
            "Added schedule for library {} with cron '{}' (mode: {})",
            library.name, cron_schedule, scan_mode
        );

        Ok(())
    }

    /// Remove a library's schedule
    pub async fn remove_library_schedule(&mut self, library_id: Uuid) -> Result<()> {
        // Note: tokio-cron-scheduler doesn't have a direct way to remove jobs by metadata
        // The best approach is to reload all schedules
        info!("Removing schedule for library {}", library_id);
        self.reload_schedules().await
    }

    /// Shutdown the scheduler
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down scan scheduler");
        self.scheduler
            .shutdown()
            .await
            .context("Failed to shutdown scheduler")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanning_config_parsing() {
        let json = r#"{
            "cron_schedule": "0 */6 * * *",
            "scan_mode": "normal",
            "auto_scan_on_create": true,
            "enabled": true,
            "scan_on_start": true,
            "purge_deleted_on_scan": true
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
            "scan_mode": "deep"
        }"#;

        let config: ScanningConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.get_scan_mode().unwrap(), ScanMode::Deep);
    }
}
