use crate::commands::common::{display_database_config, init_tracing, load_config};
use crate::db::Database;
use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::info;

/// Migrate command handler - runs database migrations and exits
pub async fn migrate_command(config_path: PathBuf) -> Result<()> {
    // Load configuration
    let (config, _config_created) = load_config(config_path.clone())?;

    // Initialize tracing with config
    let (_log_guard, log_level) = init_tracing(&config)?;
    info!("Logging level: {}", log_level);
    info!("Loading configuration from {:?}", config_path);
    info!("Configuration loaded successfully");

    info!("========================================");
    info!("Codex Migration Tool v{}", env!("CARGO_PKG_VERSION"));
    info!("========================================");

    // Display database configuration
    display_database_config(&config);

    // Initialize database connection
    info!("========================================");
    info!("Connecting to database...");
    let db = Database::new(&config.database)
        .await
        .context("Failed to connect to database")?;
    info!("Database connected successfully");

    // Run migrations
    db.run_migrations()
        .await
        .context("Failed to run database migrations")?;

    // Verify migrations are complete
    let complete = db
        .migrations_complete()
        .await
        .context("Failed to check migration status")?;

    if complete {
        info!("========================================");
        info!("✓ All migrations applied successfully");
        info!("========================================");
        Ok(())
    } else {
        anyhow::bail!("Migrations completed but status check indicates pending migrations");
    }
}
