use crate::commands::common::{
    display_database_config, ensure_data_directories, get_worker_count, init_database,
    init_settings_service, init_tracing, load_config, shutdown_workers, spawn_workers,
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

    // Initialize tracing with config
    let (log_guard, log_level) = init_tracing(&config)?;
    info!("Logging level: {}", log_level);
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
    let event_broadcaster = Arc::new(crate::events::EventBroadcaster::new(1000));
    info!("Event broadcaster initialized");

    // Initialize thumbnail service
    let thumbnail_service = Arc::new(crate::services::ThumbnailService::new(config.files.clone()));
    info!(
        "Files service initialized (thumbnails: {}, uploads: {})",
        config.files.thumbnail_dir, config.files.uploads_dir
    );

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

    // Spawn multiple workers for parallel task processing
    let (worker_handles, worker_shutdown_channels) = spawn_workers(
        db.sea_orm_connection(),
        worker_count,
        event_broadcaster,
        settings_service,
        thumbnail_service,
        Some(task_metrics_service),
        config.files.clone(),
    );

    info!("All {} task workers started successfully", worker_count);
    info!("========================================");
    info!("✓ Worker process ready");
    info!("  Press Ctrl+C to stop");
    info!("========================================");

    // Keep log guard alive
    let _log_guard = log_guard;

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
