use crate::commands::common::{
    display_database_config, ensure_data_directories, get_worker_count, init_database,
    init_settings_service, init_tracing, load_config, shutdown_workers, spawn_workers,
};
use crate::config::DatabaseType;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tracing::info;

/// Serve command handler - starts the media server
pub async fn serve_command(config_path: PathBuf) -> anyhow::Result<()> {
    // Load configuration
    let (config, config_created) = load_config(config_path.clone())?;

    // Initialize tracing with config
    let (log_guard, log_level) = init_tracing(&config)?;
    info!("Logging level: {}", log_level);

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

    // Create and start scheduler
    info!("Initializing job scheduler...");
    let scheduler: Arc<tokio::sync::Mutex<crate::scheduler::Scheduler>> =
        Arc::new(tokio::sync::Mutex::new(
            crate::scheduler::Scheduler::new(db.sea_orm_connection().clone()).await?,
        ));
    scheduler.lock().await.start().await?;
    info!("Job scheduler started successfully");

    // Initialize settings service
    let settings_service = init_settings_service(db.sea_orm_connection()).await?;

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

    // Initialize thumbnail service (needed for both workers and API handlers)
    let thumbnail_service = Arc::new(crate::services::ThumbnailService::new(config.files.clone()));
    info!(
        "Files service initialized (thumbnails: {}, uploads: {})",
        config.files.thumbnail_dir, config.files.uploads_dir
    );

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
    task_metrics_service.clone().start_background_jobs();
    info!("Task metrics background jobs started");

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
        );
        worker_handles = handles;
        worker_shutdown_channels = channels;

        info!("All {} task workers started successfully", worker_count);
    }

    // Initialize email service
    info!("Initializing email service...");
    let email_service = Arc::new(crate::services::email::EmailService::new(
        config.email.clone(),
    ));
    info!("  SMTP host: {}", config.email.smtp_host);
    info!("  SMTP port: {}", config.email.smtp_port);
    info!("  From: {}", config.email.smtp_from_email);

    // Create application state for API
    let api_state = Arc::new(crate::api::AppState {
        db: db.sea_orm_connection().clone(),
        jwt_service: Arc::new(crate::utils::jwt::JwtService::new(
            config.auth.jwt_secret.clone(),
            config.auth.jwt_expiry_hours,
        )),
        auth_config: Arc::new(config.auth.clone()),
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

    let mut app = crate::api::create_router(api_state, &config.api);

    // Conditionally mount Scalar API docs if enabled
    if config.api.enable_api_docs {
        use crate::api::ApiDoc;
        use utoipa::OpenApi;
        use utoipa_scalar::{Scalar, Servable};

        info!("API docs (Scalar) enabled at {}", config.api.api_docs_path);

        // Scalar needs a 'static string, so we leak it
        // This is acceptable since it's created once at server startup
        let api_docs_path: &'static str =
            Box::leak(config.api.api_docs_path.clone().into_boxed_str());
        app = app.merge(Scalar::with_url(api_docs_path, ApiDoc::openapi()));
    }

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

    // Keep log guard alive
    let _log_guard = log_guard;

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

    // Shutdown scheduler
    info!("Shutting down job scheduler...");
    if let Err(e) = scheduler.lock().await.shutdown().await {
        tracing::warn!("Failed to shutdown scheduler gracefully: {}", e);
    }

    // Shutdown workers if they were started
    if !disable_workers && worker_count > 0 {
        shutdown_workers(worker_handles, worker_shutdown_channels, worker_count).await;
    }

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
