use crate::config::{Config, DatabaseConfig, DatabaseType, EnvOverride};
use crate::db::Database;
use crate::events::EventBroadcaster;
use crate::services::SettingsService;
use crate::tasks::TaskWorker;
use sea_orm::DatabaseConnection;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

/// Result of initializing common services
pub struct CommonServices {
    pub db: Database,
    pub settings_service: Arc<SettingsService>,
    pub event_broadcaster: Arc<EventBroadcaster>,
    pub log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

/// Load and apply configuration
pub fn load_config(config_path: PathBuf) -> anyhow::Result<(Config, bool)> {
    // Check if config file exists, if not create a default one
    let config_created = if !config_path.exists() {
        println!(
            "Config file not found at {:?}, creating default configuration...",
            config_path
        );
        let default_config = Config::default();
        default_config.to_file(&config_path)?;
        println!("Default config file created at {:?}", config_path);
        true
    } else {
        false
    };

    // Load configuration
    let mut config = Config::from_file(config_path.to_str().unwrap())?;

    // Apply environment variable overrides
    config.apply_env_overrides("CODEX");

    Ok((config, config_created))
}

/// Initialize tracing with config
/// Returns an optional guard that must be kept alive and the log level string
pub fn init_tracing(
    config: &Config,
) -> anyhow::Result<(Option<tracing_appender::non_blocking::WorkerGuard>, String)> {
    use std::fs;
    use std::io;
    use tracing_subscriber::fmt::writer::MakeWriterExt;

    // Get log level from config or environment
    let log_level = if let Ok(env_log) = std::env::var("RUST_LOG") {
        if env_log.contains("sqlx=") {
            env_log
        } else {
            let base_level = if env_log.contains(',') {
                env_log.split(',').next().unwrap_or(&env_log).trim()
            } else {
                &env_log
            };
            match base_level {
                "debug" | "trace" => env_log,
                _ => format!("{},sqlx=warn", env_log),
            }
        }
    } else {
        let config_level = config.logging.level.as_str();
        match config_level {
            "debug" | "trace" => config_level.to_string(),
            _ => format!("{},sqlx=warn", config_level),
        }
    };

    let env_filter = EnvFilter::new(&log_level);
    let console_enabled = config.logging.console;

    let guard = match (console_enabled, &config.logging.file) {
        (true, Some(log_file_path)) => {
            let log_path = std::path::Path::new(log_file_path);
            if let Some(parent) = log_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let directory = log_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            let filename = log_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("codex.log");

            let file_appender = tracing_appender::rolling::daily(directory, filename);
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

            let writer = io::stdout.and(non_blocking);

            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_writer(writer)
                .init();

            Some(guard)
        }
        (true, None) => {
            tracing_subscriber::fmt().with_env_filter(env_filter).init();
            None
        }
        (false, Some(log_file_path)) => {
            let log_path = std::path::Path::new(log_file_path);
            if let Some(parent) = log_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let directory = log_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            let filename = log_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("codex.log");

            let file_appender = tracing_appender::rolling::daily(directory, filename);
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_writer(non_blocking)
                .with_ansi(false)
                .init();

            Some(guard)
        }
        (false, None) => {
            tracing_subscriber::fmt().with_env_filter(env_filter).init();
            None
        }
    };

    Ok((guard, log_level))
}

/// Display database configuration
pub fn display_database_config(config: &Config) {
    info!("Database Configuration:");
    match config.database.db_type {
        DatabaseType::Postgres => {
            let pg_config = config.database.postgres.as_ref().unwrap();
            info!("  Type: PostgreSQL");
            info!("  Host: {}", pg_config.host);
            info!("  Port: {}", pg_config.port);
            info!("  Database: {}", pg_config.database_name);
            info!("  Username: {}", pg_config.username);
        }
        DatabaseType::SQLite => {
            let sqlite_config = config.database.sqlite.as_ref().unwrap();
            info!("  Type: SQLite");
            info!("  Path: {}", sqlite_config.path);
            if let Some(pragmas) = &sqlite_config.pragmas {
                info!("  Pragmas:");
                for (key, value) in pragmas {
                    info!("    {}: {}", key, value);
                }
            }
        }
    }
}

/// Wait for migrations to complete
///
/// Polls the database until migrations are complete or timeout is reached.
///
/// Parameters:
/// - `timeout_seconds`: Optional timeout in seconds (overrides env var)
/// - `check_interval_seconds`: Optional check interval in seconds (overrides env var)
///
/// Environment variables (used if parameters are None):
/// - CODEX_MIGRATION_WAIT_TIMEOUT: Timeout in seconds (default: 300)
/// - CODEX_MIGRATION_WAIT_INTERVAL: Check interval in seconds (default: 2)
pub async fn wait_for_migrations_complete(
    config: &DatabaseConfig,
    timeout_seconds: Option<u64>,
    check_interval_seconds: Option<u64>,
) -> anyhow::Result<()> {
    let timeout_seconds = timeout_seconds
        .or_else(|| {
            std::env::var("CODEX_MIGRATION_WAIT_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(300); // Default 5 minutes
    let check_interval_seconds = check_interval_seconds
        .or_else(|| {
            std::env::var("CODEX_MIGRATION_WAIT_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(2); // Default 2 seconds

    let timeout = Duration::from_secs(timeout_seconds);
    let check_interval = Duration::from_secs(check_interval_seconds);
    let start_time = std::time::Instant::now();

    info!("Waiting for migrations to complete...");
    info!("  Timeout: {} seconds", timeout.as_secs());
    info!("  Check interval: {} seconds", check_interval.as_secs());

    loop {
        // Check if we've exceeded the timeout
        if start_time.elapsed() > timeout {
            anyhow::bail!(
                "Timeout waiting for migrations to complete ({} seconds)",
                timeout.as_secs()
            );
        }

        // Try to connect to database
        match Database::new(config).await {
            Ok(db) => {
                // Check if migrations are complete
                match db.migrations_complete().await {
                    Ok(true) => {
                        info!("✓ All migrations are complete");
                        return Ok(());
                    }
                    Ok(false) => {
                        let elapsed = start_time.elapsed().as_secs();
                        warn!(
                            "Migrations not complete yet (elapsed: {}s, remaining: {}s)",
                            elapsed,
                            timeout.as_secs().saturating_sub(elapsed)
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

/// Initialize database connection and run migrations
///
/// If CODEX_SKIP_MIGRATIONS environment variable is set to "true" or "1",
/// migrations will be skipped and the function will wait for migrations to complete
/// (useful when migrations are run separately via a job/init container).
pub async fn init_database(config: &Config) -> anyhow::Result<Database> {
    info!("========================================");
    info!("Initializing database connection...");
    let db = Database::new(&config.database).await?;
    info!("Database connected successfully");

    // Check if migrations should be skipped
    let skip_migrations = std::env::var("CODEX_SKIP_MIGRATIONS")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);

    if skip_migrations {
        info!("Skipping migrations (CODEX_SKIP_MIGRATIONS is set)");
        info!("Waiting for migrations to complete (run externally)...");
        // Wait for migrations to complete (run by external process)
        // Use environment variables for timeout/interval configuration
        wait_for_migrations_complete(&config.database, None, None).await?;
        info!("Migrations are complete");
    } else {
        // Run migrations to ensure database schema is up to date
        db.run_migrations().await?;
        info!("Database migrations applied successfully");
    }

    // Verify database health
    db.health_check().await?;
    info!("Database health check passed");

    Ok(db)
}

/// Initialize settings service with auto-reload
pub async fn init_settings_service(
    db: &DatabaseConnection,
) -> anyhow::Result<Arc<SettingsService>> {
    info!("Initializing settings service...");
    let settings_service = Arc::new(
        SettingsService::new(db.clone())
            .await
            .expect("Failed to initialize settings service"),
    );
    info!(
        "Settings service initialized with {} cached settings",
        settings_service.cache_size().await
    );

    // Start auto-reload task for settings service (reload every 10 seconds)
    let settings_clone = settings_service.clone();
    tokio::spawn(async move {
        settings_clone.start_auto_reload(10).await;
    });
    info!("Settings service auto-reload task started (10 second interval)");

    Ok(settings_service)
}

/// Get worker count from config (which already includes env override)
/// Falls back to settings if config not available (for backward compatibility)
pub async fn get_worker_count(
    config: Option<&crate::config::TaskConfig>,
    settings_service: Option<&SettingsService>,
) -> u32 {
    // Priority: config (with env override) > settings > default
    if let Some(task_config) = config {
        return task_config.worker_count;
    }

    // Fallback to settings for backward compatibility
    if let Some(settings) = settings_service {
        return settings.get_uint("task.worker_count", 4).await.unwrap_or(4) as u32;
    }

    // Final fallback
    4
}

/// Spawn task workers
/// Returns handles and shutdown channels for graceful shutdown
pub fn spawn_workers(
    db: &DatabaseConnection,
    worker_count: u32,
    event_broadcaster: Arc<EventBroadcaster>,
    settings_service: Arc<SettingsService>,
    thumbnail_service: Arc<crate::services::ThumbnailService>,
) -> (
    Vec<tokio::task::JoinHandle<()>>,
    Vec<tokio::sync::broadcast::Sender<()>>,
) {
    let mut worker_handles = Vec::new();
    let mut worker_shutdown_channels = Vec::new();

    for i in 0..worker_count {
        let worker_id = format!(
            "worker-{}-{}",
            std::env::var("HOSTNAME")
                .or_else(|_| std::env::var("COMPUTERNAME"))
                .unwrap_or_else(|_| "host".to_string()),
            i
        );

        let task_worker = TaskWorker::new(db.clone())
            .with_worker_id(&worker_id)
            .with_event_broadcaster(event_broadcaster.clone())
            .with_settings_service(settings_service.clone())
            .with_thumbnail_service(thumbnail_service.clone());

        let (mut task_worker, worker_shutdown_tx) = task_worker.with_shutdown();
        worker_shutdown_channels.push(worker_shutdown_tx);

        let worker_id_clone = worker_id.clone();
        let worker_handle = tokio::spawn(async move {
            if let Err(e) = task_worker.run().await {
                tracing::error!("Task worker {} error: {}", worker_id_clone, e);
            }
        });

        worker_handles.push(worker_handle);
        info!("Task worker {} started", worker_id);
    }

    (worker_handles, worker_shutdown_channels)
}

/// Shutdown workers gracefully
pub async fn shutdown_workers(
    worker_handles: Vec<tokio::task::JoinHandle<()>>,
    worker_shutdown_channels: Vec<tokio::sync::broadcast::Sender<()>>,
    worker_count: u32,
) {
    info!("Shutting down {} task worker(s)...", worker_count);

    // Signal all workers to shutdown
    for shutdown_tx in worker_shutdown_channels {
        let _ = shutdown_tx.send(());
    }

    // Wait for all workers to finish (with timeout)
    let shutdown_timeout = std::time::Duration::from_secs(30);
    let mut completed = 0;
    for (i, worker_handle) in worker_handles.into_iter().enumerate() {
        match tokio::time::timeout(shutdown_timeout, worker_handle).await {
            Ok(Ok(_)) => {
                completed += 1;
                info!("Task worker {} shut down successfully", i);
            }
            Ok(Err(e)) => {
                tracing::warn!("Task worker {} error during shutdown: {}", i, e);
            }
            Err(_) => {
                tracing::warn!("Task worker {} did not shut down within 30 seconds", i);
            }
        }
    }
    info!(
        "{}/{} task workers shut down successfully",
        completed, worker_count
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TaskConfig;
    use crate::db::test_helpers::create_test_db;
    use crate::services::SettingsService;

    #[tokio::test]
    async fn test_get_worker_count_from_config() {
        let task_config = TaskConfig { worker_count: 8 };
        let worker_count = get_worker_count(Some(&task_config), None).await;
        assert_eq!(worker_count, 8);
    }

    #[tokio::test]
    async fn test_get_worker_count_from_settings() {
        let (_db, _temp_dir) = create_test_db().await;
        let db = _db.sea_orm_connection().clone();
        let settings_service = Arc::new(
            SettingsService::new(db.clone())
                .await
                .expect("Failed to create settings service"),
        );

        // task.worker_count is now in config file, not database
        // Test that when config is None, it falls back to default (not settings)
        // Since task.worker_count is no longer in database, settings fallback won't work
        let worker_count = get_worker_count(None, Some(&settings_service)).await;
        assert_eq!(worker_count, 4); // Default value when config is None
    }

    #[tokio::test]
    async fn test_get_worker_count_config_priority() {
        let (_db, _temp_dir) = create_test_db().await;
        let db = _db.sea_orm_connection().clone();
        let settings_service = Arc::new(
            SettingsService::new(db.clone())
                .await
                .expect("Failed to create settings service"),
        );

        // Config should be used when provided (task.worker_count is now in config, not database)
        let task_config = TaskConfig { worker_count: 5 };
        let worker_count = get_worker_count(Some(&task_config), Some(&settings_service)).await;
        assert_eq!(worker_count, 5); // Config value takes priority
    }

    #[tokio::test]
    async fn test_get_worker_count_default() {
        let worker_count = get_worker_count(None, None).await;
        assert_eq!(worker_count, 4); // Default value
    }
}
