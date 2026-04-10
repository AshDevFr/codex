use anyhow::{Context, Result};
use chrono_tz::Tz;
use sea_orm::DatabaseConnection;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::repositories::{LibraryRepository, TaskRepository};
use crate::scanner::{ScanMode, ScanningConfig};
use crate::services::settings::SettingsService;
use crate::tasks::types::TaskType;
use crate::utils::cron::{normalize_cron_expression, parse_timezone};

/// Generic scheduler for managing scheduled tasks (library scans, deduplication, etc.)
pub struct Scheduler {
    scheduler: JobScheduler,
    db: DatabaseConnection,
    /// Server-level default timezone for all cron schedules
    default_tz: Tz,
}

impl Scheduler {
    /// Create a new scheduler with a default timezone
    ///
    /// The `timezone` parameter should be an IANA timezone string (e.g., "America/Los_Angeles").
    /// Falls back to UTC if the string is invalid (with a warning).
    pub async fn new(db: DatabaseConnection, timezone: &str) -> Result<Self> {
        let scheduler = JobScheduler::new()
            .await
            .context("Failed to create job scheduler")?;

        let default_tz = match parse_timezone(timezone) {
            Ok(tz) => {
                info!("Scheduler timezone: {}", timezone);
                tz
            }
            Err(e) => {
                warn!(
                    "Invalid scheduler timezone '{}': {}. Falling back to UTC",
                    timezone, e
                );
                Tz::UTC
            }
        };

        Ok(Self {
            scheduler,
            db,
            default_tz,
        })
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

        // Load plugin data cleanup schedule (OAuth flows, expired storage)
        self.load_plugin_data_cleanup_schedule().await?;

        // Load series exports cleanup schedule
        self.load_series_exports_cleanup_schedule().await?;

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

    /// Resolve the timezone for a library scan job.
    ///
    /// Priority: library's `cronTimezone` > server default timezone
    fn resolve_library_timezone(&self, config: &ScanningConfig) -> Tz {
        if let Some(ref tz_str) = config.cron_timezone {
            match parse_timezone(tz_str) {
                Ok(tz) => return tz,
                Err(e) => {
                    warn!(
                        "Invalid library cron timezone '{}': {}. Using server default ({})",
                        tz_str, e, self.default_tz
                    );
                }
            }
        }
        self.default_tz
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
            if let Some(config_json) = &library.scanning_config
                && let Ok(config) = serde_json::from_str::<ScanningConfig>(config_json)
                && config.scan_on_start
            {
                info!("Triggering scan-on-start for library {}", library.name);
                let scan_mode = config.get_scan_mode().unwrap_or(ScanMode::Normal);

                let task_type = TaskType::ScanLibrary {
                    library_id: library.id,
                    mode: scan_mode.to_string(),
                };

                if let Err(e) = TaskRepository::enqueue(&self.db, task_type, None).await {
                    warn!(
                        "Failed to trigger scan-on-start for library {}: {}",
                        library.name, e
                    );
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

        // Normalize cron expression (converts 5-part Unix cron to 6-part format)
        let cron = normalize_cron_expression(&cron)
            .context("Invalid cron expression for deduplication schedule")?;

        // Create cron job with timezone
        let db = self.db.clone();
        let tz = self.default_tz;
        let job = Job::new_async_tz(cron.as_str(), tz, move |_uuid, _lock| {
            let db = db.clone();
            Box::pin(async move {
                info!("Triggering scheduled duplicate detection");

                let task_type = TaskType::FindDuplicates;
                match TaskRepository::enqueue(&db, task_type, None).await {
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

        info!(
            "Added deduplication schedule: {} (timezone: {})",
            cron, self.default_tz
        );

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

        // Normalize cron expression (converts 5-part Unix cron to 6-part format)
        let cron = normalize_cron_expression(&cron)
            .context("Invalid cron expression for PDF cache cleanup schedule")?;

        // Create cron job with timezone
        let db = self.db.clone();
        let tz = self.default_tz;
        let job = Job::new_async_tz(cron.as_str(), tz, move |_uuid, _lock| {
            let db = db.clone();
            Box::pin(async move {
                info!("Triggering scheduled PDF cache cleanup");

                let task_type = TaskType::CleanupPdfCache;
                match TaskRepository::enqueue(&db, task_type, None).await {
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

        info!(
            "Added PDF cache cleanup schedule: {} (timezone: {})",
            cron, self.default_tz
        );

        Ok(())
    }

    /// Load plugin data cleanup schedule
    ///
    /// Periodically cleans up expired OAuth pending flows and plugin storage data.
    /// Runs every 15 minutes. This is always enabled as it's essential housekeeping
    /// to prevent memory leaks from abandoned OAuth flows.
    async fn load_plugin_data_cleanup_schedule(&mut self) -> Result<()> {
        // Every 15 minutes (6-part cron: sec min hour day month weekday)
        let cron = "0 */15 * * * *";

        let db = self.db.clone();
        let tz = self.default_tz;
        let job = Job::new_async_tz(cron, tz, move |_uuid, _lock| {
            let db = db.clone();
            Box::pin(async move {
                debug!("Triggering scheduled plugin data cleanup");

                let task_type = TaskType::CleanupPluginData;
                match TaskRepository::enqueue(&db, task_type, None).await {
                    Ok(_) => debug!("Plugin data cleanup task enqueued"),
                    Err(e) => error!("Failed to enqueue plugin data cleanup: {}", e),
                }
            })
        })
        .context("Failed to create plugin data cleanup cron job")?;

        self.scheduler
            .add(job)
            .await
            .context("Failed to add plugin data cleanup job to scheduler")?;

        info!("Added plugin data cleanup schedule: {}", cron);

        Ok(())
    }

    /// Load series exports cleanup schedule from settings
    ///
    /// Periodically cleans up expired exports, stale tmp files, and enforces
    /// the global storage cap. Cron is configurable via DB settings.
    async fn load_series_exports_cleanup_schedule(&mut self) -> Result<()> {
        let settings = SettingsService::new(self.db.clone()).await?;

        // Default: every hour at minute 30
        let cron = settings
            .get_string("exports.cleanup_cron", "0 30 * * * *")
            .await?;

        if cron.is_empty() {
            debug!("Series exports cleanup disabled (empty cron schedule)");
            return Ok(());
        }

        let cron = normalize_cron_expression(&cron)
            .context("Invalid cron expression for series exports cleanup schedule")?;

        let db = self.db.clone();
        let tz = self.default_tz;
        let job = Job::new_async_tz(cron.as_str(), tz, move |_uuid, _lock| {
            let db = db.clone();
            Box::pin(async move {
                debug!("Triggering scheduled series exports cleanup");

                let task_type = TaskType::CleanupSeriesExports;
                match TaskRepository::enqueue(&db, task_type, None).await {
                    Ok(_) => debug!("Series exports cleanup task enqueued"),
                    Err(e) => error!("Failed to enqueue series exports cleanup: {}", e),
                }
            })
        })
        .context("Failed to create series exports cleanup cron job")?;

        self.scheduler
            .add(job)
            .await
            .context("Failed to add series exports cleanup job to scheduler")?;

        info!(
            "Added series exports cleanup schedule: {} (timezone: {})",
            cron, self.default_tz
        );

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

        // Normalize cron expression (converts 5-part Unix cron to 6-part format)
        let cron = normalize_cron_expression(&cron)
            .context("Invalid cron expression for book thumbnail schedule")?;

        // Create cron job with timezone
        let db = self.db.clone();
        let tz = self.default_tz;
        let job = Job::new_async_tz(cron.as_str(), tz, move |_uuid, _lock| {
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

                match TaskRepository::enqueue(&db, task_type, None).await {
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

        info!(
            "Added book thumbnail generation schedule: {} (timezone: {})",
            cron, self.default_tz
        );

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

        // Normalize cron expression (converts 5-part Unix cron to 6-part format)
        let cron = normalize_cron_expression(&cron)
            .context("Invalid cron expression for series thumbnail schedule")?;

        // Create cron job with timezone
        let db = self.db.clone();
        let tz = self.default_tz;
        let job = Job::new_async_tz(cron.as_str(), tz, move |_uuid, _lock| {
            let db = db.clone();
            Box::pin(async move {
                info!("Triggering scheduled series thumbnail generation");

                // Enqueue fan-out task that will filter and enqueue individual tasks
                let task_type = TaskType::GenerateSeriesThumbnails {
                    library_id: None,
                    series_ids: None,
                    force: false, // Only generate missing thumbnails
                };

                match TaskRepository::enqueue(&db, task_type, None).await {
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

        info!(
            "Added series thumbnail generation schedule: {} (timezone: {})",
            cron, self.default_tz
        );

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

        // Normalize cron expression (converts 5-part Unix cron to 6-part format)
        let cron_schedule = normalize_cron_expression(&cron_schedule)
            .context("Invalid cron expression for library schedule")?;

        // Resolve timezone: library override > server default
        let tz = self.resolve_library_timezone(&config);

        // Parse scan mode
        let scan_mode = config.get_scan_mode()?;

        // Create cron job with timezone
        let db = self.db.clone();
        let library_name = library.name.clone();

        let job = Job::new_async_tz(cron_schedule.as_str(), tz, move |_uuid, _lock| {
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

                match TaskRepository::enqueue(&db, task_type, None).await {
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
            "Added schedule for library {} with cron '{}' (mode: {}, timezone: {})",
            library.name, cron_schedule, scan_mode, tz
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
