use crate::config::{Config, DatabaseType, EnvOverride};
use crate::db::Database;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{util::SubscriberInitExt, EnvFilter};


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
    let log_guard = init_tracing(&config)?;

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

    // Create application state for API
    let api_state = Arc::new(crate::api::AuthState {
        db: db.sea_orm_connection().clone(),
        jwt_service: Arc::new(crate::utils::jwt::JwtService::new(
            config.auth.jwt_secret.clone(),
            config.auth.jwt_expiry_hours,
        )),
    });

    // Build router using API module
    info!("========================================");
    info!("Building HTTP router...");
    let mut app = crate::api::create_router(api_state);

    // Conditionally mount Swagger UI if enabled
    if config.api.enable_swagger {
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;
        use crate::api::ApiDoc;

        info!("Swagger UI enabled at {}", config.api.swagger_path);

        // SwaggerUi needs a 'static string, so we leak it
        // This is acceptable since it's created once at server startup
        let swagger_path: &'static str = Box::leak(config.api.swagger_path.clone().into_boxed_str());
        app = app.merge(SwaggerUi::new(swagger_path).url("/api-docs/openapi.json", <ApiDoc as OpenApi>::openapi()));
    }

    info!("Registered routes:");
    info!("  GET  /health - Health check endpoint");
    info!("  POST /api/v1/auth/login - Login endpoint");
    info!("  POST /api/v1/auth/logout - Logout endpoint");
    info!("  GET  /api/v1/libraries - List libraries");
    info!("  GET  /api/v1/series - List series");
    info!("  GET  /api/v1/books - List books");
    info!("  GET  /api/v1/users - List users (admin)");
    if config.api.enable_swagger {
        info!("  GET  {} - Swagger UI", config.api.swagger_path);
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

    axum::serve(listener, app).await?;

    Ok(())
}


/// Initialize tracing with both console and file output based on config
/// Returns an optional guard that must be kept alive for the duration of the application
fn init_tracing(config: &Config) -> anyhow::Result<Option<tracing_appender::non_blocking::WorkerGuard>> {
    use std::fs;
    use std::io;
    use tracing_subscriber::fmt::writer::MakeWriterExt;

    // Get log level from config or environment
    let log_level = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| config.logging.level.as_str().to_string());

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&log_level));

    // Create a combined writer for console and/or file
    let guard = match (&config.logging.console, &config.logging.file) {
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
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

            let writer = io::stdout.and(non_blocking);

            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_writer(writer)
                .init();

            Some(guard)
        }
        (true, None) => {
            // Console only
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .init();

            None
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
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_writer(non_blocking)
                .with_ansi(false)
                .init();

            Some(guard)
        }
        (false, None) => {
            // Neither (fallback to console)
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .init();

            None
        }
    };

    Ok(guard)
}
