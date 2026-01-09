use crate::commands::common::{
    display_database_config, init_tracing, load_config, wait_for_migrations_complete,
};
use anyhow::Result;
use std::path::PathBuf;
use tracing::info;

/// Wait for migrations command handler - waits for migrations to complete
pub async fn wait_for_migrations_command(
    config_path: PathBuf,
    timeout_seconds: Option<u64>,
    check_interval_seconds: Option<u64>,
) -> Result<()> {
    // Load configuration
    let (config, _config_created) = load_config(config_path.clone())?;

    // Initialize tracing with config
    let (_log_guard, log_level) = init_tracing(&config)?;
    info!("Logging level: {}", log_level);
    info!("Loading configuration from {:?}", config_path);
    info!("Configuration loaded successfully");

    info!("========================================");
    info!("Codex Migration Waiter v{}", env!("CARGO_PKG_VERSION"));
    info!("========================================");

    // Display database configuration
    display_database_config(&config);

    info!("========================================");

    // Use the shared wait function
    wait_for_migrations_complete(&config.database, timeout_seconds, check_interval_seconds).await?;

    info!("========================================");
    info!("✓ All migrations are complete");
    info!("========================================");

    Ok(())
}
