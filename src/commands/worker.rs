use codex_cli_common::{
    TracingHandles, display_database_config, ensure_data_directories, get_worker_count,
    init_database, init_settings_service, init_tracing, load_config, shutdown_workers,
    spawn_workers,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Worker command handler - starts task workers without web server
pub async fn worker_command(config_path: PathBuf) -> anyhow::Result<()> {
    // Load configuration
    let (config, _config_created) = load_config(config_path.clone())?;

    // Initialize tracing with config (composes fmt + optional OTel layer)
    let tracing_handles = init_tracing(&config)?;
    info!("Logging level: {}", tracing_handles.log_level);
    info!("Loading configuration from {:?}", config_path);
    info!("Configuration loaded successfully");

    // Ensure all required data directories exist
    ensure_data_directories(&config)?;

    info!("========================================");
    info!("Starting Codex Worker v{}", env!("CARGO_PKG_VERSION"));
    info!("========================================");

    // Display database configuration
    display_database_config(&config);

    // Initialize database connection
    let db = init_database(&config).await?;

    // Create cancellation token for graceful shutdown of background tasks
    let background_task_cancel = CancellationToken::new();

    // Initialize settings service
    let (settings_service, settings_auto_reload_handle) =
        init_settings_service(db.sea_orm_connection(), background_task_cancel.clone()).await?;

    // Get worker count from config (which includes env override) or settings fallback
    let worker_count = get_worker_count(Some(&config.task), Some(&settings_service)).await;

    if let Ok(env_count) = std::env::var("CODEX_TASK_WORKER_COUNT") {
        info!(
            "Worker count from environment variable CODEX_TASK_WORKER_COUNT: {}",
            env_count
        );
    } else {
        info!("Worker count from config: {}", worker_count);
    }

    info!("Starting {} task queue worker(s)...", worker_count);

    // Create event broadcaster for real-time updates (workers don't need to emit events, but handlers might)
    //
    // In a distributed (PostgreSQL) deployment the worker runs in a separate
    // process from the web server and has no local SSE subscribers. Task
    // completion is bridged by a DB trigger, but progress is high-frequency and
    // ephemeral: we bridge it by forwarding progress events into a channel that
    // a publisher drains and re-emits via `pg_notify('task_progress', ...)`.
    let (task_progress_notifier, event_broadcaster) =
        if config.database.db_type == codex_config::DatabaseType::Postgres {
            // Bounded channel: progress is best-effort, so a full channel drops
            // events rather than applying backpressure to task execution.
            let (tx, rx) = tokio::sync::mpsc::channel(1024);
            codex_services::task_progress_publisher::spawn(
                db.sea_orm_connection().clone(),
                rx,
                codex_services::task_progress_publisher::DEFAULT_THROTTLE,
            );
            info!("Task progress publisher started (PostgreSQL LISTEN/NOTIFY bridge)");
            let broadcaster =
                Arc::new(codex_events::EventBroadcaster::new(1000).with_task_notifier(tx.clone()));
            (Some(tx), broadcaster)
        } else {
            (None, Arc::new(codex_events::EventBroadcaster::new(1000)))
        };
    info!("Event broadcaster initialized");

    // Initialize thumbnail service
    let thumbnail_service = Arc::new(codex_services::ThumbnailService::new(config.files.clone()));
    info!(
        "Files service initialized (thumbnails: {}, uploads: {})",
        config.files.thumbnail_dir, config.files.uploads_dir
    );

    // Initialize task metrics service
    let task_metrics_service = Arc::new(codex_services::TaskMetricsService::new(
        db.sea_orm_connection().clone(),
        settings_service.clone(),
    ));
    info!("Task metrics service initialized");

    // Start background jobs for metrics (flush, cleanup, rollup)
    let task_metrics_handles = task_metrics_service
        .clone()
        .start_background_jobs(background_task_cancel.clone());
    info!("Task metrics background jobs started");

    // Initialize PDF page cache service
    let pdf_page_cache = Arc::new(codex_services::PdfPageCache::new(
        &config.pdf.cache_dir,
        config.pdf.cache_rendered_pages,
    ));
    if config.pdf.cache_rendered_pages {
        info!(
            "PDF page cache initialized (cache_dir: {})",
            config.pdf.cache_dir
        );
    } else {
        info!("PDF page cache disabled");
    }

    // Initialize PDF handle (open-document) cache so the scanner can invalidate
    // cached PDFium documents when book files change. Standalone workers don't
    // serve API requests, but the scanner still updates books and we want the
    // cache contract (open once) to hold across deployments that share state.
    let handle_cache_cfg = &config.pdf_handle_cache;
    let pdf_handle_cache = Arc::new(codex_services::PdfHandleCache::new(
        handle_cache_cfg.capacity,
        std::time::Duration::from_secs(handle_cache_cfg.idle_ttl_minutes * 60),
        handle_cache_cfg.enabled,
    ));

    // Initialize PDFium library for PDF page rendering
    // Treat empty string same as None (auto-detect from system paths)
    let pdfium_path = config
        .pdf
        .pdfium_library_path
        .as_ref()
        .filter(|s| !s.is_empty())
        .map(std::path::Path::new);
    match codex_parsers::pdf::init_pdfium(pdfium_path) {
        Ok(()) => {
            info!("PDFium library initialized successfully");
        }
        Err(e) => {
            tracing::warn!(
                "PDFium initialization failed: {}. PDF page rendering will be unavailable for text-only PDFs.",
                e
            );
        }
    }

    // Initialize plugin metrics service for plugin operation metrics
    info!("Initializing plugin metrics service...");
    let plugin_metrics_service = Arc::new(codex_services::PluginMetricsService::new());

    // Initialize plugin manager for plugin auto-match tasks
    //
    // Note: no broadcaster injection. Reverse-RPC handlers (e.g.
    // `releases/record`) emit through the task-local recording broadcaster
    // set up by `TaskWorker::run_task`, not through a manager-held one.
    // See `codex_events::with_recording_broadcaster`.
    info!("Initializing plugin manager...");
    // Wire plugin file storage so plugins spawned in the worker (sync cron,
    // recommendations refresh, …) receive a real `dataDir` in their init
    // message. Without it, `resolve_plugin_data_dir` returns None and plugins
    // fall back to a container-local temp dir for any files they write.
    let plugin_file_storage = Arc::new(codex_services::PluginFileStorage::new(
        &config.files.plugins_dir,
    ));
    let plugin_manager = Arc::new(
        codex_services::plugin::PluginManager::with_defaults(Arc::new(
            db.sea_orm_connection().clone(),
        ))
        .with_plugin_log_level(Some(config.logging.level.as_plugin_log_level().to_string()))
        .with_metrics_service(plugin_metrics_service)
        .with_plugin_file_storage(plugin_file_storage),
    );
    // Load enabled plugins from database
    match plugin_manager.load_all().await {
        Ok(count) => info!("  Loaded {} enabled plugins", count),
        Err(e) => tracing::warn!("  Failed to load plugins: {}", e),
    }
    // Start periodic health checks for plugins
    plugin_manager.start_health_checks().await;
    info!("  Plugin health checks started (60s interval)");

    // Create export storage for series export tasks
    let exports_dir = settings_service
        .get_string(
            "exports.dir",
            codex_services::export_storage::DEFAULT_EXPORTS_DIR,
        )
        .await
        .unwrap_or_else(|_| codex_services::export_storage::DEFAULT_EXPORTS_DIR.to_string());
    let export_storage = Arc::new(codex_services::ExportStorage::new(exports_dir));

    // Spawn multiple workers for parallel task processing
    let (worker_handles, worker_shutdown_channels) = spawn_workers(
        db.sea_orm_connection(),
        worker_count,
        event_broadcaster,
        settings_service,
        thumbnail_service,
        Some(task_metrics_service),
        config.files.clone(),
        Some(pdf_page_cache),
        Some(pdf_handle_cache),
        Some(plugin_manager),
        None, // No OAuth state manager in standalone worker (no API state to clean)
        export_storage,
        task_progress_notifier,
    );

    info!("All {} task workers started successfully", worker_count);
    info!("========================================");
    info!("✓ Worker process ready");
    info!("  Press Ctrl+C to stop");
    info!("========================================");

    // Keep log guard alive; hold the observability handle until graceful exit.
    let TracingHandles {
        file_guard: _log_guard,
        observability: observability_handle,
        log_level: _,
    } = tracing_handles;

    // Wait for shutdown signal
    shutdown_signal().await;

    // Signal all background tasks to shutdown
    info!("Signaling background tasks to shutdown...");
    background_task_cancel.cancel();

    // Await settings auto-reload task completion
    info!("Waiting for settings auto-reload task to complete...");
    if let Err(e) = settings_auto_reload_handle.await {
        tracing::warn!("Settings auto-reload task panicked: {}", e);
    }

    // Await task metrics background jobs completion
    info!("Waiting for task metrics background jobs to complete...");
    for (i, handle) in task_metrics_handles.into_iter().enumerate() {
        if let Err(e) = handle.await {
            tracing::warn!("Task metrics background job {} panicked: {}", i, e);
        }
    }
    info!("Background tasks shutdown complete");

    // Shutdown workers
    shutdown_workers(worker_handles, worker_shutdown_channels, worker_count).await;

    // Flush + shut down OTel providers (no-op when observability is disabled).
    info!("Flushing OpenTelemetry providers...");
    observability_handle.shutdown();

    info!("Shutdown complete");

    Ok(())
}

/// Wait for shutdown signal (SIGTERM or SIGINT/Ctrl+C)
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received SIGTERM signal");
        },
    }

    info!("Starting graceful shutdown...");
}
