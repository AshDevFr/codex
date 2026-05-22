use crate::commands::common::{display_database_config, init_tracing, load_config};
use crate::db::Database;
use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::info;

/// Migrate command handler - runs database migrations and exits
pub async fn migrate_command(config_path: PathBuf) -> Result<()> {
    // Load configuration
    let (config, _config_created) = load_config(config_path.clone())?;

    // Initialize tracing with config (composes fmt + optional OTel layer)
    let _tracing_handles = init_tracing(&config)?;
    info!("Logging level: {}", _tracing_handles.log_level);
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_sqlite_config(temp_dir: &TempDir) -> PathBuf {
        let config_path = temp_dir.path().join("test_config.yaml");
        let db_path = temp_dir.path().join("test.db");
        let config_content = format!(
            r#"
application:
  host: "127.0.0.1"
  port: 8080

database:
  db_type: sqlite
  sqlite:
    path: "{}"
"#,
            db_path.to_str().unwrap()
        );
        std::fs::write(&config_path, config_content).unwrap();
        config_path
    }

    #[tokio::test]
    async fn migrate_command_runs_to_completion() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_sqlite_config(&temp_dir);

        let result = migrate_command(config_path).await;

        assert!(result.is_ok(), "migrate should succeed: {:?}", result);
    }

    #[tokio::test]
    async fn migrate_command_is_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_sqlite_config(&temp_dir);

        migrate_command(config_path.clone())
            .await
            .expect("first migrate should succeed");

        let result = migrate_command(config_path).await;
        assert!(result.is_ok(), "re-running migrate should also succeed");
    }
}
