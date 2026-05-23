use crate::commands::common::{
    TracingHandles, display_database_config, ensure_data_directories, get_worker_count,
    init_database, init_settings_service, init_tracing, load_config, shutdown_workers,
    spawn_workers,
};
use crate::config::DatabaseType;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Serve command handler - starts the media server
pub async fn serve_command(config_path: PathBuf) -> anyhow::Result<()> {
    // Load configuration
    let (config, config_created) = load_config(config_path.clone())?;

    // Initialize tracing with config (composes fmt + optional OTel layer)
    let tracing_handles = init_tracing(&config)?;
    info!("Logging level: {}", tracing_handles.log_level);
    info!(
        "Observability: traces={}, metrics={}",
        tracing_handles.observability.traces_enabled(),
        tracing_handles.observability.metrics_enabled(),
    );

    if config_created {
        info!("Created default configuration file");
    }
    info!("Loading configuration from {:?}", config_path);
    info!("Configuration loaded successfully");

    // Ensure all required data directories exist
    ensure_data_directories(&config)?;

    info!("========================================");
    info!("Starting Codex v{}", env!("CARGO_PKG_VERSION"));
    info!("  Application name is configurable via database settings");
    info!("========================================");

    // Display application configuration
    info!("Application Configuration:");
    info!("  Host: {}", config.application.host);
    info!("  Port: {}", config.application.port);

    // Display database configuration
    display_database_config(&config);

    // Initialize database connection
    let db = init_database(&config).await?;

    // Create cancellation token for graceful shutdown of background tasks
    let background_task_cancel = CancellationToken::new();

    // Initialize settings service
    let (settings_service, settings_auto_reload_handle) =
        init_settings_service(db.sea_orm_connection(), background_task_cancel.clone()).await?;

    // Create event broadcaster for real-time updates
    let event_broadcaster = Arc::new(crate::events::EventBroadcaster::new(1000));
    info!("Event broadcaster initialized");

    // Start cleanup event subscriber to handle file cleanup on entity deletion
    let cleanup_subscriber = crate::services::CleanupEventSubscriber::new(
        db.sea_orm_connection().clone(),
        event_broadcaster.clone(),
    );
    let _cleanup_subscriber_handle = cleanup_subscriber.start();
    info!("Cleanup event subscriber started");

    // Start PostgreSQL task listener for multi-container deployments
    // This allows workers in separate containers to notify the web server of task completions
    if config.database.db_type == DatabaseType::Postgres {
        info!("Starting PostgreSQL task listener for cross-container notifications...");
        match crate::services::TaskListener::from_sea_orm(
            db.sea_orm_connection(),
            event_broadcaster.clone(),
        ) {
            Ok(listener) => {
                tokio::spawn(async move {
                    if let Err(e) = listener.start().await {
                        tracing::error!("Task listener error: {}", e);
                    }
                });
                info!("PostgreSQL task listener started successfully");
            }
            Err(e) => {
                tracing::warn!("Failed to start task listener (non-fatal): {}", e);
                tracing::warn!("SSE events will only work if workers run in the same process");
            }
        }
    } else {
        info!("Task listener not started (only available with PostgreSQL)");
        info!("For SSE to work with SQLite, workers must run in the same process");
    }

    // Check if workers should be disabled (useful for web-only pods in k8s)
    let disable_workers = std::env::var("CODEX_DISABLE_WORKERS")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);

    // Initialize thumbnail service (needed for both workers, API handlers, and scheduler)
    let thumbnail_service = Arc::new(crate::services::ThumbnailService::new(config.files.clone()));
    info!(
        "Files service initialized (thumbnails: {}, uploads: {})",
        config.files.thumbnail_dir, config.files.uploads_dir
    );

    // Create and start scheduler
    info!("Initializing job scheduler...");
    let scheduler: Arc<tokio::sync::Mutex<crate::scheduler::Scheduler>> =
        Arc::new(tokio::sync::Mutex::new(
            crate::scheduler::Scheduler::new(
                db.sea_orm_connection().clone(),
                &config.scheduler.timezone,
            )
            .await?,
        ));
    scheduler.lock().await.start().await?;
    info!("Job scheduler started successfully");

    // Initialize file cleanup service (for orphaned file cleanup via API)
    let file_cleanup_service = Arc::new(crate::services::FileCleanupService::new(
        config.files.clone(),
    ));

    // Initialize task metrics service
    let task_metrics_service = Arc::new(crate::services::TaskMetricsService::new(
        db.sea_orm_connection().clone(),
        settings_service.clone(),
    ));
    info!("Task metrics service initialized");

    // Start background jobs for metrics (flush, cleanup, rollup)
    let task_metrics_handles = task_metrics_service
        .clone()
        .start_background_jobs(background_task_cancel.clone());
    info!("Task metrics background jobs started");

    // Refresh the inventory metric snapshot every 30s so the OTel observable
    // gauges have current values. Cheap: five `COUNT(*)` queries. The poller
    // exits as soon as the cancellation token fires.
    let inventory_poller_handle = crate::observability::inventory::spawn_poller(
        Arc::new(db.sea_orm_connection().clone()),
        std::time::Duration::from_secs(30),
        background_task_cancel.clone(),
    );

    // Initialize read progress batching service
    let read_progress_service = Arc::new(crate::services::ReadProgressService::new(
        db.sea_orm_connection().clone(),
    ));
    info!("Read progress batching service initialized");

    // Start background flush job for read progress
    let read_progress_handle = read_progress_service
        .clone()
        .start_background_flush(background_task_cancel.clone());
    info!("Read progress background flush started (5s interval)");

    // Initialize auth tracking batching service
    let auth_tracking_service = Arc::new(crate::services::AuthTrackingService::new(
        db.sea_orm_connection().clone(),
    ));
    info!("Auth tracking batching service initialized");

    // Start background flush job for auth tracking
    let auth_tracking_handle = auth_tracking_service
        .clone()
        .start_background_flush(background_task_cancel.clone());
    info!("Auth tracking background flush started (60s interval)");

    // Initialize PDFium library for PDF page rendering
    // Treat empty string same as None (auto-detect from system paths)
    let pdfium_path = config
        .pdf
        .pdfium_library_path
        .as_ref()
        .filter(|s| !s.is_empty())
        .map(std::path::Path::new);
    match crate::parsers::pdf::init_pdfium(pdfium_path) {
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

    // Initialize PDF page cache service
    let pdf_page_cache = Arc::new(crate::services::PdfPageCache::new(
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

    // Initialize PDF handle (open-document) cache and its idle sweeper.
    // Bounded by capacity (hard cap) and an idle TTL applied by a background
    // task. The cache stays empty until the page handler wires `get_or_open`
    // into the render miss path.
    let handle_cache_cfg = &config.pdf_handle_cache;
    let pdf_handle_cache = Arc::new(crate::services::PdfHandleCache::new(
        handle_cache_cfg.capacity,
        std::time::Duration::from_secs(handle_cache_cfg.idle_ttl_minutes * 60),
        handle_cache_cfg.enabled,
    ));
    let pdf_handle_cache_sweeper_handle = if handle_cache_cfg.enabled {
        info!(
            "PDF handle cache initialized (capacity: {}, idle TTL: {}min, sweep: {}s)",
            handle_cache_cfg.capacity,
            handle_cache_cfg.idle_ttl_minutes,
            handle_cache_cfg.sweep_interval_seconds,
        );
        Some(pdf_handle_cache.clone().spawn_sweeper(
            std::time::Duration::from_secs(handle_cache_cfg.sweep_interval_seconds),
            background_task_cancel.clone(),
        ))
    } else {
        info!("PDF handle cache disabled");
        None
    };

    // Subscribe the handle cache to entity events so book mutations evict
    // stale handles automatically. Covers BookUpdated (analyzer, manual edits,
    // scanner soft-delete/restore) and BookDeleted (purge paths).
    let _pdf_handle_cache_subscriber_handle = if handle_cache_cfg.enabled {
        let subscriber = crate::services::PdfHandleCacheSubscriber::new(
            pdf_handle_cache.clone(),
            event_broadcaster.clone(),
        );
        Some(subscriber.start())
    } else {
        None
    };

    // Initialize rate limiter service if enabled
    let rate_limiter_service = if config.rate_limit.enabled {
        let service = Arc::new(crate::services::RateLimiterService::new(Arc::new(
            config.rate_limit.clone(),
        )));
        info!(
            "Rate limiter initialized (anonymous: {} rps/{} burst, authenticated: {} rps/{} burst)",
            config.rate_limit.anonymous_rps,
            config.rate_limit.anonymous_burst,
            config.rate_limit.authenticated_rps,
            config.rate_limit.authenticated_burst
        );
        Some(service)
    } else {
        info!("Rate limiting disabled");
        None
    };

    // Start rate limiter background cleanup if enabled
    let rate_limiter_cleanup_handle = rate_limiter_service.as_ref().map(|service| {
        service
            .clone()
            .start_background_cleanup(background_task_cancel.clone())
    });

    // Initialize email service
    info!("Initializing email service...");
    let mut email_config = config.email.clone();
    // Resolve verification_url_base: explicit config > application.base_url > http://{host}:{port}
    if email_config.verification_url_base.is_none() {
        email_config.verification_url_base = Some(config.application.effective_base_url());
    }
    let email_service = Arc::new(crate::services::email::EmailService::new(email_config));
    info!("  SMTP host: {}", config.email.smtp_host);
    info!("  SMTP port: {}", config.email.smtp_port);
    info!("  From: {}", config.email.smtp_from_email);

    // Initialize OIDC service if enabled
    let oidc_service = if config.auth.oidc.enabled {
        info!("Initializing OIDC authentication service...");
        let base_url = config
            .auth
            .oidc
            .redirect_uri_base
            .clone()
            .unwrap_or_else(|| config.application.effective_base_url());
        info!("  Redirect URI base: {}", base_url);
        info!(
            "  Auto-create users: {}",
            config.auth.oidc.auto_create_users
        );
        info!("  Default role: {}", config.auth.oidc.default_role.as_str());
        let service = crate::services::OidcService::new(config.auth.oidc.clone(), base_url.clone());
        let provider_count = service.get_providers().len();
        info!("  Providers: {}", provider_count);
        for (name, provider_config) in &config.auth.oidc.providers {
            info!("    - {} ({})", provider_config.display_name, name);
            info!("      Issuer: {}", provider_config.issuer_url);
            info!("      Client ID: {}", provider_config.client_id);
            info!(
                "      Scopes: openid {}",
                if provider_config.scopes.is_empty() {
                    "(none additional)".to_string()
                } else {
                    provider_config.scopes.join(", ")
                }
            );
            info!(
                "      Callback: {}/api/v1/auth/oidc/{}/callback",
                base_url.trim_end_matches('/'),
                name
            );
        }
        Some(Arc::new(service))
    } else {
        info!("OIDC authentication disabled");
        None
    };

    // Initialize plugin metrics service
    info!("Initializing plugin metrics service...");
    let plugin_metrics_service = Arc::new(crate::services::PluginMetricsService::new());
    info!("Plugin metrics service initialized");

    // Initialize plugin file storage (shared between plugin manager and app state)
    let plugin_file_storage = Arc::new(crate::services::PluginFileStorage::new(
        &config.files.plugins_dir,
    ));

    // Initialize plugin manager (before workers so they can handle plugin tasks)
    //
    // Note: no broadcaster injection. Reverse-RPC handlers (e.g.
    // `releases/record`) emit through the task-local recording broadcaster
    // set up by `TaskWorker::run_task`, not through a manager-held one.
    // See `crate::events::with_recording_broadcaster`.
    info!("Initializing plugin manager...");
    let plugin_manager = Arc::new(
        crate::services::plugin::PluginManager::with_defaults(Arc::new(
            db.sea_orm_connection().clone(),
        ))
        .with_metrics_service(plugin_metrics_service.clone())
        .with_plugin_file_storage(plugin_file_storage.clone())
        .with_scheduler(scheduler.clone()),
    );
    // Load enabled plugins from database
    match plugin_manager.load_all().await {
        Ok(count) => info!("  Loaded {} enabled plugins", count),
        Err(e) => tracing::warn!("  Failed to load plugins: {}", e),
    }
    // Start periodic health checks for plugins
    plugin_manager.start_health_checks().await;
    info!("  Plugin health checks started (60s interval)");

    // Initialize OAuth state manager (shared between API and workers for cleanup)
    let oauth_state_manager = Arc::new(crate::services::user_plugin::OAuthStateManager::new());

    // Create export storage for series export tasks (shared between workers and API)
    let exports_dir = settings_service
        .get_string(
            "exports.dir",
            crate::services::export_storage::DEFAULT_EXPORTS_DIR,
        )
        .await
        .unwrap_or_else(|_| crate::services::export_storage::DEFAULT_EXPORTS_DIR.to_string());
    let export_storage = Arc::new(crate::services::ExportStorage::new(exports_dir));

    // Initialize worker tracking variables
    let mut worker_handles = Vec::new();
    let mut worker_shutdown_channels = Vec::new();
    let mut worker_count = 0u32;

    if disable_workers {
        info!("Workers disabled via CODEX_DISABLE_WORKERS environment variable");
    } else {
        // Get worker count from config (which includes env override) or settings fallback
        worker_count = get_worker_count(Some(&config.task), Some(&settings_service)).await;

        if let Ok(env_count) = std::env::var("CODEX_TASK_WORKER_COUNT") {
            info!(
                "Worker count from environment variable CODEX_TASK_WORKER_COUNT: {}",
                env_count
            );
        } else {
            info!("Worker count from settings: {}", worker_count);
        }

        // Reconcile orphaned series exports from prior crash/restart
        if let Err(e) = crate::tasks::handlers::cleanup_series_exports::reconcile_on_startup(
            db.sea_orm_connection(),
        )
        .await
        {
            tracing::warn!("Failed to reconcile orphaned exports on startup: {e}");
        }

        info!("Starting {} task queue worker(s)...", worker_count);

        // Spawn multiple workers for parallel task processing
        let (handles, channels) = spawn_workers(
            db.sea_orm_connection(),
            worker_count,
            event_broadcaster.clone(),
            settings_service.clone(),
            thumbnail_service.clone(),
            Some(task_metrics_service.clone()),
            config.files.clone(),
            Some(pdf_page_cache.clone()),
            Some(pdf_handle_cache.clone()),
            Some(plugin_manager.clone()),
            Some(oauth_state_manager.clone()),
            export_storage.clone(),
        );
        worker_handles = handles;
        worker_shutdown_channels = channels;

        info!("All {} task workers started successfully", worker_count);
    }

    // Build the in-memory fuzzy search index from the current DB snapshot.
    // The build itself runs in serial with startup so that the first request
    // sees a fully populated index. If the build fails (unlikely — it's just
    // reads) we fall back to an empty index and continue starting up; queries
    // will simply return no results until the event listener catches up.
    info!("Building in-memory fuzzy search index...");
    let fuzzy_index = match crate::search::builder::build_from_db(db.sea_orm_connection()).await {
        Ok(idx) => Arc::new(idx),
        Err(err) => {
            tracing::warn!(
                "Failed to build fuzzy search index at startup: {err:#}. \
                 Continuing with an empty index; results will be incomplete until rebuild."
            );
            Arc::new(crate::search::FuzzyIndex::empty())
        }
    };

    // Spawn the event listener so the index applies entity CRUD events as
    // they happen. Lifetime is tied to `background_task_cancel`; on shutdown
    // either the cancel token fires or the broadcaster's shutdown signal
    // wakes the recv and the listener exits.
    let fuzzy_listener_handle = crate::search::spawn_listener(
        fuzzy_index.clone(),
        event_broadcaster.clone(),
        db.sea_orm_connection().clone(),
        background_task_cancel.clone(),
    );

    // Create application state for API
    let refresh_token_service = Arc::new(crate::services::RefreshTokenService::new(
        db.sea_orm_connection().clone(),
        config.auth.refresh_token_expiry_days,
    ));
    let api_state = Arc::new(crate::api::AppState {
        db: db.sea_orm_connection().clone(),
        jwt_service: Arc::new(crate::utils::jwt::JwtService::new(
            config.auth.jwt_secret.clone(),
            config.auth.jwt_expiry_hours,
        )),
        refresh_token_service,
        auth_config: Arc::new(config.auth.clone()),
        database_config: Arc::new(config.database.clone()),
        pdf_config: Arc::new(config.pdf.clone()),
        email_service,
        event_broadcaster: event_broadcaster.clone(),
        settings_service,
        thumbnail_service,
        file_cleanup_service,
        task_metrics_service: Some(task_metrics_service),
        scheduler: if disable_workers {
            None
        } else {
            Some(scheduler.clone())
        },
        read_progress_service,
        auth_tracking_service,
        pdf_page_cache,
        pdf_handle_cache,
        inflight_thumbnails: Arc::new(crate::services::InflightThumbnailTracker::new()),
        user_auth_cache: Arc::new(crate::api::extractors::auth::UserAuthCache::new()),
        rate_limiter_service,
        plugin_manager: plugin_manager.clone(),
        plugin_metrics_service,
        oidc_service,
        oauth_state_manager: oauth_state_manager.clone(),
        export_storage: Some(export_storage.clone()),
        plugin_file_storage: Some(plugin_file_storage),
        scheduler_timezone: config.scheduler.timezone.clone(),
        fuzzy_index,
    });

    // Build router using API module
    info!("========================================");
    info!("Building HTTP router...");

    // Display API configuration
    info!("API Configuration:");
    info!("  Base path: {}", config.api.base_path);
    info!("  CORS enabled: {}", config.api.cors_enabled);
    if config.api.cors_enabled {
        info!("  CORS origins: {:?}", config.api.cors_origins);
    }
    info!("  Max page size: {}", config.api.max_page_size);

    let app = crate::api::create_router(api_state, &config);

    info!("Registered routes:");
    info!("  GET  /health - Health check endpoint");
    info!("  POST /api/v1/auth/login - Login endpoint");
    info!("  POST /api/v1/auth/logout - Logout endpoint");
    info!("  GET  /api/v1/libraries - List libraries");
    info!("  GET  /api/v1/series - List series");
    info!("  GET  /api/v1/books - List books");
    info!("  GET  /api/v1/users - List users (admin)");
    if config.api.enable_api_docs {
        info!("  GET  {} - API docs (Scalar)", config.api.api_docs_path);
    }

    // Destructure the tracing handles: keep file guard alive for the
    // remainder of `serve_command`, and hold onto the OTel guard so we can
    // flush providers explicitly during graceful shutdown.
    let TracingHandles {
        file_guard: _log_guard,
        observability: observability_handle,
        log_level: _,
    } = tracing_handles;

    // Start server
    info!("========================================");
    let addr = format!("{}:{}", config.application.host, config.application.port);
    info!("Starting HTTP server...");
    info!("  Binding to: {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("========================================");
    info!("✓ Server ready and listening on http://{}", addr);
    info!("  Health check: http://{}/health", addr);
    info!("========================================");

    // Set up graceful shutdown with broadcaster shutdown
    let event_broadcaster_for_shutdown = event_broadcaster.clone();
    let graceful = axum::serve(listener, app).with_graceful_shutdown(async move {
        shutdown_signal().await;
        // Shutdown event broadcaster to close all SSE connections
        info!("Closing SSE connections...");
        event_broadcaster_for_shutdown.shutdown();
    });

    // Run server with graceful shutdown
    let server_result = graceful.await;

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

    // Await inventory metrics poller completion
    info!("Waiting for inventory metrics poller to complete...");
    if let Err(e) = inventory_poller_handle.await {
        tracing::warn!("Inventory metrics poller panicked: {}", e);
    }

    // Await read progress background flush task completion
    info!("Waiting for read progress flush task to complete...");
    if let Err(e) = read_progress_handle.await {
        tracing::warn!("Read progress flush task panicked: {}", e);
    }

    // Await auth tracking background flush task completion
    info!("Waiting for auth tracking flush task to complete...");
    if let Err(e) = auth_tracking_handle.await {
        tracing::warn!("Auth tracking flush task panicked: {}", e);
    }

    // Await rate limiter cleanup task completion if it was started
    if let Some(handle) = rate_limiter_cleanup_handle {
        info!("Waiting for rate limiter cleanup task to complete...");
        if let Err(e) = handle.await {
            tracing::warn!("Rate limiter cleanup task panicked: {}", e);
        }
    }

    // Await PDF handle cache sweeper if it was started
    if let Some(handle) = pdf_handle_cache_sweeper_handle {
        info!("Waiting for PDF handle cache sweeper to complete...");
        if let Err(e) = handle.await {
            tracing::warn!("PDF handle cache sweeper panicked: {}", e);
        }
    }

    // Await fuzzy search event listener
    info!("Waiting for fuzzy search event listener to complete...");
    if let Err(e) = fuzzy_listener_handle.await {
        tracing::warn!("Fuzzy search event listener panicked: {}", e);
    }
    info!("Background tasks shutdown complete");

    // Shutdown scheduler
    info!("Shutting down job scheduler...");
    if let Err(e) = scheduler.lock().await.shutdown().await {
        tracing::warn!("Failed to shutdown scheduler gracefully: {}", e);
    }

    // Shutdown plugin manager (stops health checks and all plugins)
    info!("Shutting down plugin manager...");
    plugin_manager.shutdown_all().await;
    info!("Plugin manager shutdown complete");

    // Shutdown workers if they were started
    if !disable_workers && worker_count > 0 {
        shutdown_workers(worker_handles, worker_shutdown_channels, worker_count).await;
    }

    // Flush + shut down OTel providers (no-op when observability is disabled).
    // Done last so any spans emitted during shutdown still get exported.
    info!("Flushing OpenTelemetry providers...");
    observability_handle.shutdown();
    info!("OpenTelemetry providers flushed");

    info!("Shutdown complete");
    server_result?;
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
