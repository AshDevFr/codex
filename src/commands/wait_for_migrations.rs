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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::migrate::migrate_command;
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

    fn write_unreachable_config(temp_dir: &TempDir) -> PathBuf {
        let config_path = temp_dir.path().join("test_config.yaml");
        let config_content = r#"
application:
  host: "127.0.0.1"
  port: 8080

database:
  db_type: sqlite
  sqlite:
    path: "/nonexistent/path/to/database.db"
"#;
        std::fs::write(&config_path, config_content).unwrap();
        config_path
    }

    #[tokio::test]
    async fn returns_quickly_when_already_complete() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_sqlite_config(&temp_dir);

        migrate_command(config_path.clone())
            .await
            .expect("migrate should succeed");

        let result = wait_for_migrations_command(config_path, Some(10), Some(1)).await;

        assert!(result.is_ok(), "wait should succeed: {:?}", result);
    }

    #[tokio::test]
    async fn errors_when_database_unreachable() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_unreachable_config(&temp_dir);

        let result = wait_for_migrations_command(config_path, Some(2), Some(1)).await;

        assert!(result.is_err(), "wait should fail for unreachable db");
    }

    #[tokio::test]
    async fn unblocks_when_migrations_run_concurrently() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_sqlite_config(&temp_dir);

        let wait_handle = tokio::spawn({
            let config_path = config_path.clone();
            async move { wait_for_migrations_command(config_path, Some(10), Some(1)).await }
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        migrate_command(config_path)
            .await
            .expect("migrate should succeed");

        let result = wait_handle.await.unwrap();
        assert!(
            result.is_ok(),
            "wait should unblock once migrations complete: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn respects_configured_timeout() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = write_unreachable_config(&temp_dir);

        let start = std::time::Instant::now();
        let result = wait_for_migrations_command(config_path, Some(2), None).await;
        let elapsed = start.elapsed();

        assert!(result.is_err());
        assert!(
            elapsed >= std::time::Duration::from_secs(2),
            "should wait at least the configured timeout"
        );
    }
}
