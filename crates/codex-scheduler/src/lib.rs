pub mod release_sources;

use anyhow::{Context, Result};
use chrono_tz::Tz;
use sea_orm::DatabaseConnection;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use codex_db::entities::{library_jobs, plugins, user_plugins};
use codex_db::repositories::{
    LibraryJobRepository, LibraryRepository, PluginsRepository, TaskRepository,
    UserPluginsRepository,
};
use codex_scanner::{ScanMode, ScanningConfig};
use codex_services::library_jobs::{LibraryJobConfig, parse_job_config};
use codex_services::settings::SettingsService;
use codex_tasks::types::TaskType;
use codex_utils::cron::{normalize_cron_expression, parse_timezone};

/// Generic scheduler for managing scheduled tasks (library scans, deduplication, etc.)
pub struct Scheduler {
    scheduler: JobScheduler,
    db: DatabaseConnection,
    /// Server-level default timezone for all cron schedules
    default_tz: Tz,
    /// Reconcile state for the per-source release-polling jobs.
    release_sources: release_sources::ReleaseSourceSchedule,
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
            release_sources: release_sources::ReleaseSourceSchedule::new(),
        })
    }

    /// Trigger a release-source reconcile. Call after writes to the
    /// `release_sources` table so the scheduler picks up enable/disable
    /// changes without a full restart.
    pub async fn reconcile_release_sources(&mut self) -> Result<()> {
        let settings = SettingsService::new(self.db.clone()).await?;
        let server_default = release_sources::read_server_default_cron(&settings).await;
        release_sources::reconcile(
            &mut self.scheduler,
            &mut self.release_sources,
            &self.db,
            server_default,
        )
        .await
    }

    /// Start the scheduler and load all scheduled jobs
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting job scheduler");

        // Load library scan schedules
        self.load_library_schedules().await?;

        // Load per-library scheduled metadata-refresh entries
        self.load_library_metadata_refresh_schedules().await?;

        // Load deduplication schedule
        self.load_deduplication_schedule().await?;

        // Load PDF cache cleanup schedule
        self.load_pdf_cache_cleanup_schedule().await?;

        // Load plugin data cleanup schedule (OAuth flows, expired storage)
        self.load_plugin_data_cleanup_schedule().await?;

        // Load admin-configured per-plugin user-sync schedules
        self.load_plugin_sync_schedules().await?;

        // Load refresh-token cleanup schedule
        self.load_refresh_token_cleanup_schedule().await?;

        // Load series exports cleanup schedule
        self.load_series_exports_cleanup_schedule().await?;

        // Load thumbnail generation schedules
        self.load_book_thumbnail_schedule().await?;
        self.load_series_thumbnail_schedule().await?;

        // Load release-source polling schedules.
        if let Err(e) = self.reconcile_release_sources().await {
            warn!("Failed to load release-source schedules: {}", e);
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

    /// Load refresh-token cleanup schedule
    ///
    /// Daily sweep that removes expired refresh-token rows and any rows that
    /// were revoked more than 30 days ago. Always enabled - the table grows
    /// linearly with logins and would otherwise accumulate forever.
    async fn load_refresh_token_cleanup_schedule(&mut self) -> Result<()> {
        // 02:30 every day (6-part cron: sec min hour day month weekday).
        // Chosen to avoid colliding with the 02:00 scan windows and the 15-min
        // plugin-data sweep.
        let cron = "0 30 2 * * *";

        let db = self.db.clone();
        let tz = self.default_tz;
        let job = Job::new_async_tz(cron, tz, move |_uuid, _lock| {
            let db = db.clone();
            Box::pin(async move {
                debug!("Triggering scheduled refresh-token cleanup");

                let task_type = TaskType::CleanupRefreshTokens;
                match TaskRepository::enqueue(&db, task_type, None).await {
                    Ok(_) => debug!("Refresh-token cleanup task enqueued"),
                    Err(e) => error!("Failed to enqueue refresh-token cleanup: {}", e),
                }
            })
        })
        .context("Failed to create refresh-token cleanup cron job")?;

        self.scheduler
            .add(job)
            .await
            .context("Failed to add refresh-token cleanup job to scheduler")?;

        info!("Added refresh-token cleanup schedule: {}", cron);

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

    /// Load library-jobs cron entries.
    ///
    /// Walks `library_jobs` rows where `enabled = true` and dispatches by
    /// `r#type`. Currently only handles `metadata_refresh`; future job types
    /// extend the match.
    async fn load_library_metadata_refresh_schedules(&mut self) -> Result<()> {
        let jobs = LibraryJobRepository::list_enabled(&self.db, None).await?;
        for job in jobs {
            if let Err(e) = self.add_library_job_schedule(&job).await {
                warn!(
                    "Failed to add schedule for library job {} ('{}'): {}",
                    job.id, job.name, e
                );
            }
        }
        Ok(())
    }

    /// Resolve the timezone for a library job.
    fn resolve_library_job_timezone(&self, tz_str: Option<&str>) -> Tz {
        if let Some(tz_str) = tz_str {
            match parse_timezone(tz_str) {
                Ok(tz) => return tz,
                Err(e) => {
                    warn!(
                        "Invalid library-job timezone '{}': {}. Using server default ({})",
                        tz_str, e, self.default_tz
                    );
                }
            }
        }
        self.default_tz
    }

    /// Register a single library-job's cron entry.
    ///
    /// Each firing performs a per-job skip-if-already-running check
    /// before enqueuing `RefreshLibraryMetadata { job_id }`. Two jobs on
    /// the same library can run concurrently because the guard scopes
    /// per-job, not per-library.
    pub async fn add_library_job_schedule(&mut self, job: &library_jobs::Model) -> Result<()> {
        // Type dispatch. Currently only metadata_refresh.
        let cfg = match parse_job_config(&job.r#type, &job.config) {
            Ok(c) => c,
            Err(e) => {
                warn!(
                    "Library job {} ('{}') has invalid config ({}); skipping schedule",
                    job.id, job.name, e
                );
                return Ok(());
            }
        };

        match cfg {
            LibraryJobConfig::MetadataRefresh(_) => {}
        }

        if !job.enabled {
            debug!("Skipping disabled library job {} ('{}')", job.id, job.name);
            return Ok(());
        }

        let cron_schedule = normalize_cron_expression(&job.cron_schedule)
            .context("Invalid cron expression for library job")?;
        let tz = self.resolve_library_job_timezone(job.timezone.as_deref());

        let db = self.db.clone();
        let job_id = job.id;
        let job_name = job.name.clone();

        let scheduled_job = Job::new_async_tz(cron_schedule.as_str(), tz, move |_uuid, _lock| {
            let db = db.clone();
            let job_name = job_name.clone();

            Box::pin(async move {
                match has_active_refresh_for_job(&db, job_id).await {
                    Ok(true) => {
                        warn!(
                            "Skipping library job '{}' ({}): previous run still active",
                            job_name, job_id
                        );
                        return;
                    }
                    Ok(false) => {}
                    Err(e) => {
                        warn!(
                            "Failed to check in-flight task for job {}: {}; proceeding",
                            job_id, e
                        );
                    }
                }

                info!("Triggering library job '{}' ({})", job_name, job_id);
                let task_type = TaskType::RefreshLibraryMetadata { job_id };
                match TaskRepository::enqueue(&db, task_type, None).await {
                    Ok(_) => debug!("Enqueued library job '{}'", job_name),
                    Err(e) => error!("Failed to enqueue library job {}: {}", job_id, e),
                }
            })
        })
        .context("Failed to create library job cron")?;

        self.scheduler
            .add(scheduled_job)
            .await
            .context("Failed to add library job cron to scheduler")?;

        info!(
            "Added library job '{}' ({}) cron='{}' tz={}",
            job.name, job.id, cron_schedule, tz
        );
        Ok(())
    }

    /// Load admin-configured per-plugin user-sync schedules.
    ///
    /// Each enabled plugin with a `sync_cron_schedule` and the `user_read_sync`
    /// capability gets one cron entry. When it fires, the scheduler fans out a
    /// `UserPluginSync` task for every connected user who opted into auto sync.
    /// The row set is small (admin-managed), so a full reload is cheap.
    async fn load_plugin_sync_schedules(&mut self) -> Result<()> {
        let plugins = PluginsRepository::list_sync_scheduled(&self.db).await?;
        for plugin in plugins {
            if let Err(e) = self.add_plugin_sync_schedule(&plugin).await {
                warn!(
                    "Failed to add sync schedule for plugin {} ('{}'): {}",
                    plugin.id, plugin.name, e
                );
            }
        }
        Ok(())
    }

    /// Register a single plugin's user-sync cron entry.
    ///
    /// Skips (without erroring the whole load) plugins whose cached manifest
    /// does not declare the `user_read_sync` capability, so we never schedule a
    /// cron that can only fan out to zero eligible connections. Uses the server
    /// default timezone; cadence is an admin/integration concern, not per-user.
    async fn add_plugin_sync_schedule(&mut self, plugin: &plugins::Model) -> Result<()> {
        let Some(cron_raw) = plugin.sync_cron_schedule.as_deref() else {
            return Ok(());
        };

        let supports_sync = plugin
            .cached_manifest()
            .map(|m| m.capabilities.user_read_sync)
            .unwrap_or(false);
        if !supports_sync {
            warn!(
                "Plugin {} ('{}') has a sync cron but no user_read_sync capability; skipping",
                plugin.id, plugin.name
            );
            return Ok(());
        }

        let cron = normalize_cron_expression(cron_raw)
            .context("Invalid cron expression for plugin sync schedule")?;
        let tz = self.default_tz;

        let db = self.db.clone();
        let plugin_id = plugin.id;
        let plugin_name = plugin.name.clone();

        let job = Job::new_async_tz(cron.as_str(), tz, move |_uuid, _lock| {
            let db = db.clone();
            let plugin_name = plugin_name.clone();
            Box::pin(async move {
                match fan_out_plugin_sync(&db, plugin_id).await {
                    Ok(summary) => info!(
                        "Plugin sync '{}' ({}): {} eligible, {} enqueued, {} skipped (ineligible), {} skipped (in flight)",
                        plugin_name,
                        plugin_id,
                        summary.candidates,
                        summary.enqueued,
                        summary.skipped_ineligible,
                        summary.skipped_in_flight
                    ),
                    Err(e) => error!(
                        "Plugin sync fan-out failed for '{}' ({}): {}",
                        plugin_name, plugin_id, e
                    ),
                }
            })
        })
        .context("Failed to create plugin sync cron")?;

        self.scheduler
            .add(job)
            .await
            .context("Failed to add plugin sync cron to scheduler")?;

        info!(
            "Added plugin sync schedule for '{}' ({}) cron='{}' tz={}",
            plugin.name, plugin.id, cron, tz
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

/// Adapter that lets the `services` layer drive a `Scheduler` through the
/// [`codex_services::scheduler_handle::SchedulerReconciler`] trait without
/// holding the concrete type. The trait inverts the layer dependency so
/// `services` can ask for a reconcile without importing `scheduler`.
pub struct LockedSchedulerReconciler {
    inner: std::sync::Arc<tokio::sync::Mutex<Scheduler>>,
}

impl LockedSchedulerReconciler {
    pub fn new(inner: std::sync::Arc<tokio::sync::Mutex<Scheduler>>) -> Self {
        Self { inner }
    }
}

impl codex_services::scheduler_handle::SchedulerReconciler for LockedSchedulerReconciler {
    fn reconcile_release_sources(&self) -> futures::future::BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let mut guard = self.inner.lock().await;
            guard.reconcile_release_sources().await
        })
    }
}

/// Whether an active (`pending` or `processing`) `refresh_library_metadata`
/// task already exists for the given **job**.
///
/// `job_id` is stored inside `tasks.params` as JSON, so we use a backend-
/// specific JSON path query — same pattern as
/// [`codex_db::repositories::TaskRepository::has_pending_or_processing`].
pub async fn has_active_refresh_for_job(db: &DatabaseConnection, job_id: Uuid) -> Result<bool> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let job_id_str = job_id.to_string();
    let backend = db.get_database_backend();
    let stmt = match backend {
        DbBackend::Postgres => Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"SELECT 1 FROM tasks
               WHERE task_type = $1
                 AND status IN ('pending', 'processing')
                 AND params->>'job_id' = $2
               LIMIT 1"#,
            vec!["refresh_library_metadata".into(), job_id_str.into()],
        ),
        _ => Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"SELECT 1 FROM tasks
               WHERE task_type = ?
                 AND status IN ('pending', 'processing')
                 AND json_extract(params, '$.job_id') = ?
               LIMIT 1"#,
            vec!["refresh_library_metadata".into(), job_id_str.into()],
        ),
    };

    let result = db
        .query_one(stmt)
        .await
        .context("Failed to check for active refresh tasks")?;
    Ok(result.is_some())
}

/// Whether a connection is eligible for an automatic (scheduled) sync.
///
/// Eligible only when the connection is enabled, connected, and the user has
/// opted into auto sync (`config._codex.autoSync`). "Connected" means
/// authenticated for plugins that require per-user auth, or simply present for
/// credential-less / shared-key plugins (`requires_auth == false`).
/// `get_users_with_plugin` already filters to enabled rows, but we re-check
/// `enabled` so the predicate is self-contained and correct in isolation. A
/// connection whose token has expired but is otherwise authenticated is still
/// eligible here; the sync handler is responsible for surfacing/refreshing auth
/// (v1 does not refresh in the cron path).
pub(crate) fn is_auto_sync_eligible(up: &user_plugins::Model, requires_auth: bool) -> bool {
    up.enabled && up.is_connected(requires_auth) && up.auto_sync_enabled()
}

/// Outcome of one plugin-sync fan-out, used for the per-firing log line.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FanOutSummary {
    /// Connections eligible for auto sync (enabled + authenticated + opted in).
    pub candidates: usize,
    /// Eligible connections for which a new sync task was enqueued.
    pub enqueued: usize,
    /// Connections skipped because they were not eligible.
    pub skipped_ineligible: usize,
    /// Eligible connections skipped because a sync was already pending/processing.
    pub skipped_in_flight: usize,
}

/// Enqueue a `UserPluginSync` task for every eligible connection of `plugin_id`.
///
/// Skips any connection that already has a pending/processing sync, reusing the
/// exact per-(user, plugin) dedup the manual trigger endpoint uses
/// ([`TaskRepository::has_pending_or_processing`]). A slow plugin therefore
/// never stacks duplicate tasks across cron ticks. Execution rate is bounded
/// downstream by the task queue + worker concurrency, so no jitter is applied.
pub(crate) async fn fan_out_plugin_sync(
    db: &DatabaseConnection,
    plugin_id: Uuid,
) -> Result<FanOutSummary> {
    let connections = UserPluginsRepository::get_users_with_plugin(db, plugin_id).await?;
    let mut summary = FanOutSummary::default();

    // Whether this plugin needs per-user auth. Credential-less / shared-key
    // plugins are eligible without per-user credentials. No manifest → assume
    // auth is required (conservative).
    let requires_auth = PluginsRepository::get_by_id(db, plugin_id)
        .await?
        .and_then(|p| p.cached_manifest())
        .map(|m| m.requires_authentication())
        .unwrap_or(true);

    for up in &connections {
        if !is_auto_sync_eligible(up, requires_auth) {
            summary.skipped_ineligible += 1;
            continue;
        }
        summary.candidates += 1;

        match TaskRepository::has_pending_or_processing(
            db,
            "user_plugin_sync",
            plugin_id,
            up.user_id,
        )
        .await
        {
            Ok(true) => {
                summary.skipped_in_flight += 1;
                continue;
            }
            Ok(false) => {}
            Err(e) => {
                // Don't let one bad check abort the whole fan-out; skip this user.
                warn!(
                    "Failed to check in-flight sync for user {} plugin {}: {}; skipping",
                    up.user_id, plugin_id, e
                );
                summary.skipped_in_flight += 1;
                continue;
            }
        }

        let task_type = TaskType::UserPluginSync {
            plugin_id,
            user_id: up.user_id,
        };
        match TaskRepository::enqueue(db, task_type, None).await {
            Ok(_) => summary.enqueued += 1,
            Err(e) => {
                error!(
                    "Failed to enqueue auto sync for user {} plugin {}: {}",
                    up.user_id, plugin_id, e
                );
                summary.skipped_in_flight += 1;
            }
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use codex_db::entities::users;
    use codex_db::repositories::{LibraryRepository, UserRepository};
    use codex_db::test_helpers::setup_test_db;
    use codex_models::ScanningStrategy;
    use codex_tasks::types::TaskType;
    use sea_orm::{ActiveModelTrait, Set};

    #[test]
    fn test_scheduler_can_be_created() {
        // This test is a placeholder - proper tests require a database connection
        // See tests/scheduler/mod.rs for integration tests
    }

    #[tokio::test]
    async fn has_active_refresh_for_job_returns_false_when_no_tasks() {
        let db = setup_test_db().await;
        let _library = LibraryRepository::create(&db, "Lib", "/tmp/lib", ScanningStrategy::Default)
            .await
            .unwrap();

        let active = has_active_refresh_for_job(&db, Uuid::new_v4())
            .await
            .unwrap();
        assert!(
            !active,
            "Fresh DB has no refresh tasks; helper must report false"
        );
    }

    #[tokio::test]
    async fn has_active_refresh_for_job_detects_pending_task() {
        let db = setup_test_db().await;
        let job_id = Uuid::new_v4();
        TaskRepository::enqueue(&db, TaskType::RefreshLibraryMetadata { job_id }, None)
            .await
            .unwrap();

        let active = has_active_refresh_for_job(&db, job_id).await.unwrap();
        assert!(active, "Pending task for this job must be detected");
    }

    #[tokio::test]
    async fn has_active_refresh_for_job_is_scoped_per_job() {
        let db = setup_test_db().await;
        let job_a = Uuid::new_v4();
        let job_b = Uuid::new_v4();
        TaskRepository::enqueue(
            &db,
            TaskType::RefreshLibraryMetadata { job_id: job_a },
            None,
        )
        .await
        .unwrap();

        let active_a = has_active_refresh_for_job(&db, job_a).await.unwrap();
        let active_b = has_active_refresh_for_job(&db, job_b).await.unwrap();
        assert!(active_a, "job A has the in-flight task");
        assert!(!active_b, "job B has no in-flight task");
    }

    /// Build an in-memory user_plugins row for predicate testing (no DB).
    fn make_up(enabled: bool, authed: bool, auto: bool) -> user_plugins::Model {
        user_plugins::Model {
            id: Uuid::new_v4(),
            plugin_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            credentials: if authed { Some(vec![1, 2, 3]) } else { None },
            config: if auto {
                serde_json::json!({ "_codex": { "autoSync": true } })
            } else {
                serde_json::json!({})
            },
            oauth_access_token: None,
            oauth_refresh_token: None,
            oauth_expires_at: None,
            oauth_scope: None,
            external_user_id: None,
            external_username: None,
            external_avatar_url: None,
            enabled,
            health_status: "unknown".to_string(),
            failure_count: 0,
            last_failure_at: None,
            last_success_at: None,
            last_sync_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn is_auto_sync_eligible_matrix() {
        // Auth-required plugin (requires_auth = true)
        assert!(
            is_auto_sync_eligible(&make_up(true, true, true), true),
            "enabled + authed + auto is eligible"
        );
        assert!(
            !is_auto_sync_eligible(&make_up(false, true, true), true),
            "disabled is ineligible"
        );
        assert!(
            !is_auto_sync_eligible(&make_up(true, false, true), true),
            "unauthenticated is ineligible when auth is required"
        );
        assert!(
            !is_auto_sync_eligible(&make_up(true, true, false), true),
            "manual (opt-out) is ineligible"
        );

        // No-auth plugin (requires_auth = false): eligible without credentials,
        // but still gated by enabled + auto-sync opt-in.
        assert!(
            is_auto_sync_eligible(&make_up(true, false, true), false),
            "credential-less plugin is eligible without auth"
        );
        assert!(
            !is_auto_sync_eligible(&make_up(true, false, false), false),
            "credential-less plugin still needs auto-sync opt-in"
        );
        assert!(
            !is_auto_sync_eligible(&make_up(false, false, true), false),
            "credential-less plugin still needs to be enabled"
        );
    }

    async fn create_sync_plugin(db: &DatabaseConnection, name: &str) -> Uuid {
        PluginsRepository::create(
            db,
            name,
            name,
            None,
            "user",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![],
            vec![],
            None,
            "env",
            None,
            true,
            None,
            Some(60),
            None,
        )
        .await
        .unwrap()
        .id
    }

    /// Create a real connection (user + user_plugins row) and set its auth /
    /// auto-sync state via a direct ActiveModel update. Returns the user id.
    async fn create_connection(
        db: &DatabaseConnection,
        plugin_id: Uuid,
        authed: bool,
        auto: bool,
    ) -> Uuid {
        let user = users::Model {
            id: Uuid::new_v4(),
            username: format!("u_{}", Uuid::new_v4()),
            email: format!("{}@example.com", Uuid::new_v4()),
            password_hash: "hash".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: false,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        let user = UserRepository::create(db, &user).await.unwrap();

        let up = UserPluginsRepository::create(db, plugin_id, user.id)
            .await
            .unwrap();
        let mut active: user_plugins::ActiveModel = up.into();
        if authed {
            active.credentials = Set(Some(vec![1, 2, 3]));
        }
        if auto {
            active.config = Set(serde_json::json!({ "_codex": { "autoSync": true } }));
        }
        active.update(db).await.unwrap();

        user.id
    }

    #[tokio::test]
    async fn fan_out_enqueues_only_eligible_and_dedups() {
        let db = setup_test_db().await;
        let plugin_id = create_sync_plugin(&db, "sync_plugin").await;
        let other_plugin = create_sync_plugin(&db, "other_plugin").await;

        let user_a = create_connection(&db, plugin_id, true, true).await; // eligible
        let _user_b = create_connection(&db, plugin_id, true, false).await; // manual
        let _user_c = create_connection(&db, plugin_id, false, true).await; // unauthed
        let user_d = create_connection(&db, plugin_id, true, true).await; // eligible, in flight
        TaskRepository::enqueue(
            &db,
            TaskType::UserPluginSync {
                plugin_id,
                user_id: user_d,
            },
            None,
        )
        .await
        .unwrap();
        // Eligible connection on a different plugin must be untouched.
        let user_e = create_connection(&db, other_plugin, true, true).await;

        let summary = fan_out_plugin_sync(&db, plugin_id).await.unwrap();
        assert_eq!(summary.candidates, 2, "A and D are eligible");
        assert_eq!(summary.enqueued, 1, "only A is enqueued");
        assert_eq!(summary.skipped_ineligible, 2, "B (manual) and C (unauthed)");
        assert_eq!(summary.skipped_in_flight, 1, "D already pending");

        assert!(
            TaskRepository::has_pending_or_processing(&db, "user_plugin_sync", plugin_id, user_a)
                .await
                .unwrap(),
            "A now has a sync task"
        );
        assert!(
            !TaskRepository::has_pending_or_processing(
                &db,
                "user_plugin_sync",
                other_plugin,
                user_e
            )
            .await
            .unwrap(),
            "other plugin's user must not be enqueued by this plugin's fan-out"
        );

        // Second tick while A's task is still pending: nothing new is enqueued.
        let summary2 = fan_out_plugin_sync(&db, plugin_id).await.unwrap();
        assert_eq!(summary2.candidates, 2);
        assert_eq!(summary2.enqueued, 0, "dedup across cron ticks");
        assert_eq!(summary2.skipped_in_flight, 2, "A and D both in flight now");
    }
}
