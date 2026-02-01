use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::repositories::{LibraryRepository, TaskRepository};
use crate::scanner::{ScanMode, ScanningConfig};
use crate::services::settings::SettingsService;
use crate::tasks::types::TaskType;

/// Generic scheduler for managing scheduled tasks (library scans, deduplication, etc.)
pub struct Scheduler {
    scheduler: JobScheduler,
    db: DatabaseConnection,
}

impl Scheduler {
    /// Create a new scheduler
    pub async fn new(db: DatabaseConnection) -> Result<Self> {
        let scheduler = JobScheduler::new()
            .await
            .context("Failed to create job scheduler")?;

        Ok(Self { scheduler, db })
    }

    /// Start the scheduler and load all scheduled jobs
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting job scheduler");

        // Load library scan schedules
        self.load_library_schedules().await?;

        // Load deduplication schedule
        self.load_deduplication_schedule().await?;

        // Load PDF cache cleanup schedule
        self.load_pdf_cache_cleanup_schedule().await?;

        // Load thumbnail generation schedules
        self.load_book_thumbnail_schedule().await?;
        self.load_series_thumbnail_schedule().await?;

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
        info!("Job scheduler started with {} jobs", job_count);

        Ok(())
    }

    /// Load all library scan schedules
    async fn load_library_schedules(&mut self) -> Result<()> {
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

        Ok(())
    }

    /// Load deduplication schedule from settings
    async fn load_deduplication_schedule(&mut self) -> Result<()> {
        // Initialize settings service to read deduplication settings
        let settings = SettingsService::new(self.db.clone()).await?;

        // Check if deduplication is enabled
        let enabled = settings.get_bool("deduplication.enabled", true).await?;

        if !enabled {
            debug!("Deduplication scheduled scanning disabled");
            return Ok(());
        }

        // Get cron schedule
        let cron = settings
            .get_string("deduplication.cron_schedule", "")
            .await?;

        if cron.is_empty() {
            debug!("Deduplication scheduled scanning disabled (no cron)");
            return Ok(());
        }

        // Create cron job
        let db = self.db.clone();
        let job = Job::new_async(cron.as_str(), move |_uuid, _lock| {
            let db = db.clone();
            Box::pin(async move {
                info!("Triggering scheduled duplicate detection");

                let task_type = TaskType::FindDuplicates;
                match TaskRepository::enqueue(&db, task_type, 0, None).await {
                    Ok(_) => debug!("Duplicate detection task enqueued"),
                    Err(e) => error!("Failed to enqueue duplicate detection: {}", e),
                }
            })
        })
        .context("Failed to create deduplication cron job")?;

        self.scheduler
            .add(job)
            .await
            .context("Failed to add deduplication job to scheduler")?;

        info!("Added deduplication schedule: {}", cron);

        Ok(())
    }

    /// Load PDF cache cleanup schedule from settings
    async fn load_pdf_cache_cleanup_schedule(&mut self) -> Result<()> {
        // Initialize settings service to read PDF cache settings
        let settings = SettingsService::new(self.db.clone()).await?;

        // Get cron schedule (empty string = disabled)
        let cron = settings.get_string("pdf_cache.cron_schedule", "").await?;

        if cron.is_empty() {
            debug!("PDF cache cleanup disabled (no cron schedule)");
            return Ok(());
        }

        // Create cron job
        let db = self.db.clone();
        let job = Job::new_async(cron.as_str(), move |_uuid, _lock| {
            let db = db.clone();
            Box::pin(async move {
                info!("Triggering scheduled PDF cache cleanup");

                let task_type = TaskType::CleanupPdfCache;
                match TaskRepository::enqueue(&db, task_type, 0, None).await {
                    Ok(_) => debug!("PDF cache cleanup task enqueued"),
                    Err(e) => error!("Failed to enqueue PDF cache cleanup: {}", e),
                }
            })
        })
        .context("Failed to create PDF cache cleanup cron job")?;

        self.scheduler
            .add(job)
            .await
            .context("Failed to add PDF cache cleanup job to scheduler")?;

        info!("Added PDF cache cleanup schedule: {}", cron);

        Ok(())
    }

    /// Load book thumbnail generation schedule from settings
    ///
    /// This job generates thumbnails for all books that don't have one.
    /// It uses the GenerateThumbnails task type which fans out to individual book tasks.
    async fn load_book_thumbnail_schedule(&mut self) -> Result<()> {
        let settings = SettingsService::new(self.db.clone()).await?;

        // Get cron schedule (empty string = disabled)
        let cron = settings
            .get_string("thumbnail.book_cron_schedule", "")
            .await?;

        if cron.is_empty() {
            debug!("Book thumbnail generation disabled (no cron schedule)");
            return Ok(());
        }

        // Create cron job
        let db = self.db.clone();
        let job = Job::new_async(cron.as_str(), move |_uuid, _lock| {
            let db = db.clone();
            Box::pin(async move {
                info!("Triggering scheduled book thumbnail generation");

                // GenerateThumbnails with no scopes will process all books
                let task_type = TaskType::GenerateThumbnails {
                    library_id: None,
                    series_id: None,
                    series_ids: None,
                    book_ids: None,
                    force: false, // Only generate missing thumbnails
                };

                match TaskRepository::enqueue(&db, task_type, 0, None).await {
                    Ok(_) => debug!("Book thumbnail generation task enqueued"),
                    Err(e) => error!("Failed to enqueue book thumbnail generation: {}", e),
                }
            })
        })
        .context("Failed to create book thumbnail generation cron job")?;

        self.scheduler
            .add(job)
            .await
            .context("Failed to add book thumbnail generation job to scheduler")?;

        info!("Added book thumbnail generation schedule: {}", cron);

        Ok(())
    }

    /// Load series thumbnail generation schedule from settings
    ///
    /// This job generates thumbnails for all series that don't have one.
    /// It enqueues a GenerateSeriesThumbnails fan-out task that handles
    /// filtering and enqueueing individual GenerateSeriesThumbnail tasks.
    async fn load_series_thumbnail_schedule(&mut self) -> Result<()> {
        let settings = SettingsService::new(self.db.clone()).await?;

        // Get cron schedule (empty string = disabled)
        let cron = settings
            .get_string("thumbnail.series_cron_schedule", "")
            .await?;

        if cron.is_empty() {
            debug!("Series thumbnail generation disabled (no cron schedule)");
            return Ok(());
        }

        // Create cron job
        let db = self.db.clone();
        let job = Job::new_async(cron.as_str(), move |_uuid, _lock| {
            let db = db.clone();
            Box::pin(async move {
                info!("Triggering scheduled series thumbnail generation");

                // Enqueue fan-out task that will filter and enqueue individual tasks
                let task_type = TaskType::GenerateSeriesThumbnails {
                    library_id: None,
                    series_ids: None,
                    force: false, // Only generate missing thumbnails
                };

                match TaskRepository::enqueue(&db, task_type, 0, None).await {
                    Ok(_) => debug!("Series thumbnail generation task enqueued"),
                    Err(e) => error!("Failed to enqueue series thumbnail generation: {}", e),
                }
            })
        })
        .context("Failed to create series thumbnail generation cron job")?;

        self.scheduler
            .add(job)
            .await
            .context("Failed to add series thumbnail generation job to scheduler")?;

        info!("Added series thumbnail generation schedule: {}", cron);

        Ok(())
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

    /// Reload all schedules (useful when libraries or settings are updated)
    pub async fn reload_schedules(&mut self) -> Result<()> {
        info!("Reloading all schedules");

        // Remove all existing jobs
        self.scheduler.shutdown().await?;
        self.scheduler = JobScheduler::new().await?;

        // Reload
        self.start().await
    }

    /// Shutdown the scheduler
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down job scheduler");
        self.scheduler
            .shutdown()
            .await
            .context("Failed to shutdown scheduler")
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_scheduler_can_be_created() {
        // This test is a placeholder - proper tests require a database connection
        // See tests/scheduler/mod.rs for integration tests
    }
}
