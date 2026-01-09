use crate::commands::common::{display_database_config, init_tracing, load_config};
use crate::db::Database;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};

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

    let timeout = Duration::from_secs(timeout_seconds.unwrap_or(300)); // Default 5 minutes
    let check_interval = Duration::from_secs(check_interval_seconds.unwrap_or(2)); // Default 2 seconds
    let start_time = std::time::Instant::now();

    info!("Waiting for migrations to complete...");
    info!("  Timeout: {} seconds", timeout.as_secs());
    info!("  Check interval: {} seconds", check_interval.as_secs());
    info!("========================================");

    loop {
        // Check if we've exceeded the timeout
        if start_time.elapsed() > timeout {
            anyhow::bail!(
                "Timeout waiting for migrations to complete ({} seconds)",
                timeout.as_secs()
            );
        }

        // Try to connect to database
        match Database::new(&config.database).await {
            Ok(db) => {
                // Check if migrations are complete
                match db.migrations_complete().await {
                    Ok(true) => {
                        info!("========================================");
                        info!("✓ All migrations are complete");
                        info!("========================================");
                        return Ok(());
                    }
                    Ok(false) => {
                        let elapsed = start_time.elapsed().as_secs();
                        warn!(
                            "Migrations not complete yet (elapsed: {}s, remaining: {}s)",
                            elapsed,
                            timeout.as_secs() - elapsed
                        );
                    }
                    Err(e) => {
                        let elapsed = start_time.elapsed().as_secs();
                        warn!(
                            "Failed to check migration status (elapsed: {}s): {}",
                            elapsed, e
                        );
                    }
                }
            }
            Err(e) => {
                let elapsed = start_time.elapsed().as_secs();
                warn!(
                    "Failed to connect to database (elapsed: {}s): {}",
                    elapsed, e
                );
            }
        }

        // Wait before checking again
        tokio::time::sleep(check_interval).await;
    }
}
