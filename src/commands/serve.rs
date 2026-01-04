use crate::config::{Config, DatabaseType, EnvOverride};
use crate::db::Database;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};
use tracing_subscriber::{util::SubscriberInitExt, EnvFilter};

#[derive(Clone)]
struct AppState {
    db: Database,
    config: Arc<Config>,
}

/// Serve command handler - starts the media server
pub async fn serve_command(config_path: PathBuf) -> anyhow::Result<()> {
    // Check if config file exists, if not create a default one
    let config_created = if !config_path.exists() {
        println!("Config file not found at {:?}, creating default configuration...", config_path);
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

    // Initialize tracing with config
    init_tracing(&config)?;

    if config_created {
        info!("Created default configuration file");
    }
    info!("Loading configuration from {:?}", config_path);
    info!("Configuration loaded successfully");

    info!("========================================");
    info!("Starting {} v{}", config.application.name, env!("CARGO_PKG_VERSION"));
    info!("========================================");

    // Display application configuration
    info!("Application Configuration:");
    info!("  Host: {}", config.application.host);
    info!("  Port: {}", config.application.port);
    info!("  Debug mode: {}", config.application.debug);

    // Display database configuration
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

    // Initialize database connection
    info!("========================================");
    info!("Initializing database connection...");
    let db = Database::new(&config.database).await?;
    info!("Database connected successfully");

    // Verify database health
    db.health_check().await?;
    info!("Database health check passed");

    // Create application state
    let state = AppState {
        db,
        config: Arc::new(config.clone()),
    };

    // Build router
    info!("========================================");
    info!("Building HTTP router...");
    let app = Router::new()
        .route("/health", get(health_check))
        .with_state(state);
    info!("Registered routes:");
    info!("  GET /health - Health check endpoint");

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

    axum::serve(listener, app).await?;

    Ok(())
}

/// Health check endpoint - checks database connectivity
async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    // Check database health
    let db_status = match state.db.health_check().await {
        Ok(_) => {
            info!("Health check: database OK");
            "healthy"
        }
        Err(e) => {
            warn!("Health check: database error: {}", e);
            "unhealthy"
        }
    };

    let response = json!({
        "status": if db_status == "healthy" { "healthy" } else { "unhealthy" },
        "database": db_status,
        "version": env!("CARGO_PKG_VERSION"),
    });

    if db_status == "healthy" {
        (StatusCode::OK, Json(response))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(response))
    }
}

/// Initialize tracing with both console and file output based on config
fn init_tracing(config: &Config) -> anyhow::Result<()> {
    use std::fs;
    use std::io;
    use tracing_subscriber::fmt::writer::MakeWriterExt;

    // Get log level from config or environment
    let log_level = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| config.logging.level.as_str().to_string());

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&log_level));

    // Create a combined writer for console and/or file
    match (&config.logging.console, &config.logging.file) {
        (true, Some(log_file_path)) => {
            // Both console and file
            let log_path = Path::new(log_file_path);
            if let Some(parent) = log_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let directory = log_path.parent().unwrap_or_else(|| Path::new("."));
            let filename = log_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("codex.log");

            let file_appender = tracing_appender::rolling::daily(directory, filename);
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
            std::mem::forget(_guard);

            let writer = io::stdout.and(non_blocking);

            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_writer(writer)
                .init();
        }
        (true, None) => {
            // Console only
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .init();
        }
        (false, Some(log_file_path)) => {
            // File only
            let log_path = Path::new(log_file_path);
            if let Some(parent) = log_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let directory = log_path.parent().unwrap_or_else(|| Path::new("."));
            let filename = log_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("codex.log");

            let file_appender = tracing_appender::rolling::daily(directory, filename);
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
            std::mem::forget(_guard);

            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_writer(non_blocking)
                .with_ansi(false)
                .init();
        }
        (false, None) => {
            // Neither (fallback to console)
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .init();
        }
    }

    Ok(())
}
