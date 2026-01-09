use crate::commands::common::{
    display_database_config, get_worker_count, init_database, init_settings_service, init_tracing,
    load_config, shutdown_workers, spawn_workers,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
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

    info!("========================================");
    info!("Starting Codex Worker v{}", env!("CARGO_PKG_VERSION"));
    info!("========================================");

    // Display database configuration
    display_database_config(&config);

    // Initialize database connection
    let db = init_database(&config).await?;

    // Initialize settings service
    let settings_service = init_settings_service(db.sea_orm_connection()).await?;

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

    // Spawn multiple workers for parallel task processing
    let (worker_handles, worker_shutdown_channels) = spawn_workers(
        db.sea_orm_connection(),
        worker_count,
        event_broadcaster,
        settings_service,
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
