use codex_api::observability::ObservabilityHandle;
use codex_config::{Config, DatabaseConfig, DatabaseType};
use codex_db::Database;
use codex_events::{EventBroadcaster, TaskProgressEvent};
use codex_services::{SettingsService, TaskMetricsService};
use codex_tasks::TaskWorker;
use sea_orm::DatabaseConnection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

/// Ensure a directory exists, creating it and any parent directories if necessary
pub fn ensure_dir_exists(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

/// Ensure parent directory of a file path exists
pub fn ensure_parent_dir_exists(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Ensure all data directories from config exist
/// Call this after loading config to ensure thumbnail_dir, uploads_dir, plugins_dir, and database dir exist
pub fn ensure_data_directories(config: &Config) -> anyhow::Result<()> {
    // Ensure thumbnail directory exists
    let thumbnail_path = Path::new(&config.files.thumbnail_dir);
    ensure_dir_exists(thumbnail_path)?;
    info!(
        "Ensured thumbnail directory exists: {}",
        config.files.thumbnail_dir
    );

    // Ensure uploads directory exists
    let uploads_path = Path::new(&config.files.uploads_dir);
    ensure_dir_exists(uploads_path)?;
    info!(
        "Ensured uploads directory exists: {}",
        config.files.uploads_dir
    );

    // Ensure plugins directory exists
    let plugins_path = Path::new(&config.files.plugins_dir);
    ensure_dir_exists(plugins_path)?;
    info!(
        "Ensured plugins directory exists: {}",
        config.files.plugins_dir
    );

    // Ensure SQLite database parent directory exists (if using SQLite)
    if let Some(ref sqlite_config) = config.database.sqlite {
        let db_path = Path::new(&sqlite_config.path);
        ensure_parent_dir_exists(db_path)?;
        info!(
            "Ensured database directory exists for: {}",
            sqlite_config.path
        );
    }

    // Ensure PDF cache directory exists
    let pdf_cache_path = Path::new(&config.pdf.cache_dir);
    ensure_dir_exists(pdf_cache_path)?;
    info!(
        "Ensured PDF cache directory exists: {}",
        config.pdf.cache_dir
    );

    Ok(())
}

/// Load and apply configuration
pub fn load_config(config_path: PathBuf) -> anyhow::Result<(Config, bool)> {
    // Ensure config file parent directory exists
    ensure_parent_dir_exists(&config_path)?;

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

    let config = resolve_config(&config_path)?;

    warn_about_renamed_env_vars();

    Ok((config, config_created))
}

/// Resolve configuration without touching the filesystem.
///
/// A missing file yields the defaults rather than being created, so callers
/// that only want to *inspect* the configuration leave no trace. `config
/// check` relies on this: it is meant to run as a Kubernetes initContainer
/// against a read-only mount, and a validation step that writes a config file
/// as a side effect would be its own bug.
pub fn resolve_config(config_path: &Path) -> anyhow::Result<Config> {
    Config::load(config_path)
}

/// Emit a single line when environment variables will need renaming in the
/// next major version, or are being ignored right now.
///
/// Deliberately one line rather than one per variable. In this version the
/// flat names are still correct, so a warning per variable would be recurring
/// noise about something the operator cannot act on yet. `codex config check`
/// is where the detail lives.
fn warn_about_renamed_env_vars() {
    if let Some(notice) = env_var_notice(&codex_config::audit_env()) {
        warn!("{notice}");
    }
}

/// The single advisory line, or `None` when there is nothing to say.
///
/// Returning at most one string is the point: the count goes in the log and
/// the detail lives in `codex config check`. Naming each variable here would
/// put a growing block in every process's startup output on every boot.
fn env_var_notice(findings: &[codex_config::Finding]) -> Option<String> {
    if findings.is_empty() {
        return None;
    }

    let ignored = findings.iter().filter(|f| f.is_ignored_now()).count();
    let renamed = findings.len() - ignored;

    Some(match (renamed, ignored) {
        (0, _) => format!(
            "{ignored} environment variable(s) are not being read. \
             Run `codex config check` for details."
        ),
        (_, 0) => format!(
            "{renamed} environment variable(s) will be renamed in Codex 2.0. \
             Run `codex config check` for the list."
        ),
        _ => format!(
            "{renamed} environment variable(s) will be renamed in Codex 2.0 and \
             {ignored} are not being read. Run `codex config check` for details."
        ),
    })
}

/// Bundle of long-lived guards returned by [`init_tracing`].
///
/// `file_guard` keeps the non-blocking file appender's worker thread alive,
/// `observability` owns the OTel providers so [`ObservabilityHandle::shutdown`]
/// can flush them on graceful exit, and `log_level` is the effective filter
/// string for diagnostic logging.
pub struct TracingHandles {
    pub file_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
    pub observability: ObservabilityHandle,
    pub log_level: String,
}

/// Initialize tracing with config.
///
/// Composes the existing fmt + file appender with an optional OpenTelemetry
/// layer when `observability.enabled` is true. Returns a [`TracingHandles`]
/// bundle that the caller is expected to keep alive for the process lifetime
/// and to drive shutdown through.
pub fn init_tracing(config: &Config) -> anyhow::Result<TracingHandles> {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

    // Resolve the effective log filter: explicit RUST_LOG wins, then config.
    // At info/warn/error we silence sqlx down to warn (it is otherwise noisy
    // at info), preserving the user's level for the rest of the workspace.
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

    // Build the writer + keep the appender's worker guard alive. Branches on
    // the (console, file) matrix and erases the writer type via `BoxMakeWriter`
    // so the registry composition stays uniform.
    let (writer, file_guard, ansi_enabled) =
        build_log_writer(config.logging.console, config.logging.file.as_deref())?;

    // Initialize OTel providers (no-op when disabled or feature off). Done
    // before constructing the bridge layer so the global tracer is in place
    // for any code that grabs it via `global::tracer(...)` later.
    let observability = codex_api::observability::init(&config.observability)?;

    let fmt_layer = fmt::layer()
        .with_writer(writer)
        .with_ansi(ansi_enabled)
        .event_format(codex_api::observability::TraceContextFormat::default());

    // Compose subscribers inline: a generic helper here trips up the
    // Layer<S>/Subscriber bounds because each `.with(...)` changes S, so the
    // inline form is the cleanest path. Keep the two branches in sync.
    //
    // `try_init().ok()` (instead of `init()`) so a second call in the same
    // process — e.g. tests that drive migrate + wait_for_migrations back to
    // back — no-ops on the global subscriber instead of panicking.
    #[cfg(feature = "observability")]
    {
        let otel_layer = observability
            .tracer()
            .cloned()
            .map(|t| tracing_opentelemetry::layer().with_tracer(t));
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(otel_layer)
            .try_init()
            .ok();
    }
    #[cfg(not(feature = "observability"))]
    {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .try_init()
            .ok();
    }

    Ok(TracingHandles {
        file_guard,
        observability,
        log_level,
    })
}

/// Build a `MakeWriter` covering the (console, file) matrix.
///
/// Returns a type-erased writer plus the file appender's worker guard (when
/// applicable) and whether ANSI escapes should be emitted (off for file-only
/// output to keep log files plain text).
fn build_log_writer(
    console_enabled: bool,
    log_file: Option<&str>,
) -> anyhow::Result<(
    tracing_subscriber::fmt::writer::BoxMakeWriter,
    Option<tracing_appender::non_blocking::WorkerGuard>,
    bool,
)> {
    use std::io;
    use tracing_subscriber::fmt::writer::{BoxMakeWriter, MakeWriterExt};

    match (console_enabled, log_file) {
        (true, Some(path)) => {
            let (non_blocking, guard) = build_file_appender(path)?;
            let combined = io::stdout.and(non_blocking);
            Ok((BoxMakeWriter::new(combined), Some(guard), true))
        }
        (true, None) => Ok((BoxMakeWriter::new(io::stdout), None, true)),
        (false, Some(path)) => {
            let (non_blocking, guard) = build_file_appender(path)?;
            Ok((BoxMakeWriter::new(non_blocking), Some(guard), false))
        }
        (false, None) => Ok((BoxMakeWriter::new(io::sink), None, false)),
    }
}

fn build_file_appender(
    log_path: &str,
) -> anyhow::Result<(
    tracing_appender::non_blocking::NonBlocking,
    tracing_appender::non_blocking::WorkerGuard,
)> {
    let log_path = std::path::Path::new(log_path);
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
    Ok(tracing_appender::non_blocking(file_appender))
}

/// Display database configuration
pub fn display_database_config(config: &Config) {
    info!("Database Configuration:");
    // No unwrapping: a `db_type` whose section is missing is rejected by
    // `Config::validate` during load, so reaching here without one would mean
    // a caller bypassed the loader. Logging is not the place to abort.
    match config.database.db_type {
        DatabaseType::Postgres => {
            let Some(pg_config) = config.database.postgres.as_ref() else {
                warn!("  Type: PostgreSQL, but no `database.postgres` section is configured");
                return;
            };
            info!("  Type: PostgreSQL");
            info!("  Host: {}", pg_config.host);
            info!("  Port: {}", pg_config.port);
            info!("  Database: {}", pg_config.database_name);
            info!("  Username: {}", pg_config.username);
        }
        DatabaseType::SQLite => {
            let Some(sqlite_config) = config.database.sqlite.as_ref() else {
                warn!("  Type: SQLite, but no `database.sqlite` section is configured");
                return;
            };
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

/// How long to wait for the database, and how often to look.
///
/// Governs both waiting for a database to accept connections and waiting for
/// its schema to be current: during a release those are the same budget, a
/// deployment's tolerance for the database being briefly unavailable.
///
/// Parameters win over environment variables, which win over the defaults.
struct DbWait {
    timeout: Duration,
    check_interval: Duration,
}

impl DbWait {
    /// Environment variables (used when the parameter is None):
    /// - CODEX_MIGRATION_WAIT_TIMEOUT: Timeout in seconds (default: 300)
    /// - CODEX_MIGRATION_WAIT_INTERVAL: Check interval in seconds (default: 2)
    fn resolve(timeout_seconds: Option<u64>, check_interval_seconds: Option<u64>) -> Self {
        fn from_env(name: &str) -> Option<u64> {
            std::env::var(name).ok().and_then(|v| v.parse().ok())
        }

        Self {
            timeout: Duration::from_secs(
                timeout_seconds
                    .or_else(|| from_env("CODEX_MIGRATION_WAIT_TIMEOUT"))
                    .unwrap_or(300), // Default 5 minutes
            ),
            check_interval: Duration::from_secs(
                check_interval_seconds
                    .or_else(|| from_env("CODEX_MIGRATION_WAIT_INTERVAL"))
                    .unwrap_or(2), // Default 2 seconds
            ),
        }
    }

    fn log(&self, what: &str) {
        info!("Waiting for {what}...");
        info!("  Timeout: {} seconds", self.timeout.as_secs());
        info!(
            "  Check interval: {} seconds",
            self.check_interval.as_secs()
        );
    }

    fn expired(&self, start_time: std::time::Instant) -> bool {
        start_time.elapsed() > self.timeout
    }

    fn timed_out(&self) -> anyhow::Error {
        anyhow::anyhow!(
            "Timeout waiting for migrations to complete ({} seconds)",
            self.timeout.as_secs()
        )
    }

    fn timed_out_connecting(&self) -> anyhow::Error {
        anyhow::anyhow!(
            "Timeout waiting for the database to accept connections ({} seconds)",
            self.timeout.as_secs()
        )
    }
}

/// Wait for migrations to complete, polling over an existing connection.
///
/// Prefer this over [`wait_for_migrations_complete`] wherever the caller has
/// already connected: a process that is up and waiting must not keep opening
/// connections to ask the same question. On a PostgreSQL server whose limit is
/// shared across a deployment that turns a wait into an attack on the pods it is
/// waiting for — each poll claims connections, holds them for the acquire
/// timeout, and drops them, while the process already holds a working pool it
/// could have asked instead.
pub async fn wait_for_migrations_on(
    db: &Database,
    timeout_seconds: Option<u64>,
    check_interval_seconds: Option<u64>,
) -> anyhow::Result<()> {
    let wait = DbWait::resolve(timeout_seconds, check_interval_seconds);
    wait.log("migrations to complete");
    poll_until_migrated(db, &wait, std::time::Instant::now()).await
}

/// Connect, retrying while the database refuses connections.
///
/// A database that is briefly unavailable is a normal condition, not a reason
/// to abort: it restarts to apply a parameter that cannot be reloaded, it fails
/// over, it moves between nodes. A command that gives up on the first refused
/// connection turns seconds of that into a failed release, because the
/// migration Job burns its backoff limit inside the window and everything
/// gated on that Job never rolls out.
///
/// Uses the configured pool, so the caller gets a connection it can work over.
/// Callers that only need to ask one question want [`Database::new_probe`].
pub async fn connect_with_retry(
    config: &DatabaseConfig,
    timeout_seconds: Option<u64>,
    check_interval_seconds: Option<u64>,
) -> anyhow::Result<Database> {
    let wait = DbWait::resolve(timeout_seconds, check_interval_seconds);
    wait.log("the database to accept connections");
    retry_connect(&wait, std::time::Instant::now(), || Database::new(config)).await
}

/// Open a connection, retrying on failure until the budget runs out.
async fn retry_connect<F, Fut>(
    wait: &DbWait,
    start_time: std::time::Instant,
    open: F,
) -> anyhow::Result<Database>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Database>>,
{
    loop {
        if wait.expired(start_time) {
            return Err(wait.timed_out_connecting());
        }

        match open().await {
            Ok(db) => return Ok(db),
            Err(e) => {
                warn!(
                    "Failed to connect to database (elapsed: {}s): {}",
                    start_time.elapsed().as_secs(),
                    e
                );
                tokio::time::sleep(wait.check_interval).await;
            }
        }
    }
}

/// Wait for migrations to complete, for callers with no connection yet.
///
/// Opens a single-connection probe pool (retrying while the server is
/// unreachable, which is the normal state of an init container racing the
/// database) and then polls over it. Callers that already hold a pool want
/// [`wait_for_migrations_on`] instead.
///
/// Parameters:
/// - `timeout_seconds`: Optional timeout in seconds (overrides env var)
/// - `check_interval_seconds`: Optional check interval in seconds (overrides env var)
pub async fn wait_for_migrations_complete(
    config: &DatabaseConfig,
    timeout_seconds: Option<u64>,
    check_interval_seconds: Option<u64>,
) -> anyhow::Result<()> {
    let wait = DbWait::resolve(timeout_seconds, check_interval_seconds);
    wait.log("migrations to complete");
    let start_time = std::time::Instant::now();

    // Connect once. The retry is for the server not being up yet, not for
    // re-establishing a pool we already have.
    let db = retry_connect(&wait, start_time, || Database::new_probe(config)).await?;

    poll_until_migrated(&db, &wait, start_time).await
}

/// Poll `db` until every migration has been applied or the budget runs out.
async fn poll_until_migrated(
    db: &Database,
    wait: &DbWait,
    start_time: std::time::Instant,
) -> anyhow::Result<()> {
    loop {
        if wait.expired(start_time) {
            return Err(wait.timed_out());
        }

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
                    wait.timeout.as_secs().saturating_sub(elapsed)
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

        tokio::time::sleep(wait.check_interval).await;
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
        // Poll over the pool opened above rather than opening more. This process
        // is already connected; a wait that reconnects each time would spend the
        // whole timeout competing for connections with the deployment it is
        // waiting to join.
        // Timeout/interval come from the environment.
        wait_for_migrations_on(&db, None, None).await?;
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

/// Verdict of comparing this process's pools against the PostgreSQL server.
#[derive(Debug, PartialEq, Eq)]
pub enum PgBudget {
    /// The pools fit alongside everything already connected.
    Fits,
    /// This process alone asks for more than the server can ever hand out.
    ExceedsServer { usable: u32 },
    /// The pools fit on their own but not next to the connections already in
    /// use, which is the shape a multi-replica deployment fails in.
    ExceedsRemaining { usable: u32, in_use: u32 },
}

/// Compare a process's requested connections against the server's supply.
///
/// `in_use` covers the whole server, so this sees the other replicas a
/// per-process check would miss. It includes this process's own connections,
/// which makes the estimate slightly pessimistic; that is the right direction
/// for a warning.
fn assess_pg_budget(requested: u32, server_max: u32, reserved: u32, in_use: u32) -> PgBudget {
    let usable = server_max.saturating_sub(reserved);

    if requested > usable {
        PgBudget::ExceedsServer { usable }
    } else if in_use.saturating_add(requested) > usable {
        PgBudget::ExceedsRemaining { usable, in_use }
    } else {
        PgBudget::Fits
    }
}

/// Warn (non-fatal) when this process's connection pools do not fit the
/// PostgreSQL server's budget.
///
/// Every Codex process opens its own pools while `max_connections` is a limit
/// for the whole server, so the sizing that works for one process silently
/// breaks when a deployment runs several. Checking against connections already
/// in use catches that: the warning fires on the replica that is about to tip
/// the server over, not only on a process that is oversized on its own.
///
/// Best-effort. If the values cannot be read the check is skipped quietly, and
/// SQLite has no server-side limit so it is skipped there too.
pub async fn warn_if_pg_budget_exceeded(
    conn: &DatabaseConnection,
    api_max: u32,
    background_max: u32,
) {
    use sea_orm::{ConnectionTrait, Statement};

    if conn.get_database_backend() != sea_orm::DatabaseBackend::Postgres {
        return;
    }

    let requested = api_max + background_max;
    let row = conn
        .query_one(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT current_setting('max_connections')::int AS max_connections, \
             current_setting('superuser_reserved_connections')::int AS reserved, \
             (SELECT count(*)::int FROM pg_stat_activity) AS in_use"
                .to_string(),
        ))
        .await;

    let row = match row {
        Ok(Some(row)) => row,
        Ok(None) => return,
        Err(e) => {
            warn!("Could not read PostgreSQL connection limits for budget check: {e}");
            return;
        }
    };

    let (Ok(server_max), Ok(reserved), Ok(in_use)) = (
        row.try_get::<i32>("", "max_connections"),
        row.try_get::<i32>("", "reserved"),
        row.try_get::<i32>("", "in_use"),
    ) else {
        return;
    };

    match assess_pg_budget(
        requested,
        server_max.max(0) as u32,
        reserved.max(0) as u32,
        in_use.max(0) as u32,
    ) {
        PgBudget::Fits => {
            info!(
                "PostgreSQL connection budget OK: API {api_max} + background {background_max} \
                 = {requested} requested, {in_use} of {server_max} in use"
            );
        }
        PgBudget::ExceedsServer { usable } => {
            warn!(
                "Configured connection pools (API {api_max} + background {background_max} \
                 = {requested}) exceed what this PostgreSQL server can hand out ({usable} \
                 of {server_max}, {reserved} reserved for the superuser). Reduce \
                 database.postgres.max_connections or background_max_connections, or raise \
                 the server limit."
            );
        }
        PgBudget::ExceedsRemaining { usable, in_use } => {
            warn!(
                "Connection pools for this process ({requested}) do not fit alongside the \
                 {in_use} connections already open on this PostgreSQL server ({usable} \
                 usable). Other processes sharing this database are using the budget; size \
                 max_connections per process as (server limit / number of processes), or \
                 raise the server limit."
            );
        }
    }
}

/// Initialize settings service with auto-reload
///
/// Accepts a `CancellationToken` for graceful shutdown support.
/// Returns a tuple of (SettingsService, JoinHandle for the auto-reload task).
pub async fn init_settings_service(
    db: &DatabaseConnection,
    cancel_token: CancellationToken,
) -> anyhow::Result<(Arc<SettingsService>, tokio::task::JoinHandle<()>)> {
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
    let auto_reload_handle = settings_service.clone().start_auto_reload(10, cancel_token);
    info!("Settings service auto-reload task started (10 second interval)");

    Ok((settings_service, auto_reload_handle))
}

/// Get worker count from config (which already includes env override)
/// Falls back to settings if config not available (for backward compatibility)
pub async fn get_worker_count(
    config: Option<&codex_config::TaskConfig>,
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
#[allow(clippy::too_many_arguments)]
pub fn spawn_workers(
    db: &DatabaseConnection,
    worker_count: u32,
    event_broadcaster: Arc<EventBroadcaster>,
    settings_service: Arc<SettingsService>,
    thumbnail_service: Arc<codex_services::ThumbnailService>,
    task_metrics_service: Option<Arc<TaskMetricsService>>,
    files_config: codex_config::FilesConfig,
    pdf_page_cache: Option<Arc<codex_services::PdfPageCache>>,
    pdf_handle_cache: Option<Arc<codex_services::PdfHandleCache>>,
    plugin_manager: Option<Arc<codex_services::plugin::PluginManager>>,
    oauth_state_manager: Option<Arc<codex_services::user_plugin::OAuthStateManager>>,
    export_storage: Arc<codex_services::ExportStorage>,
    task_progress_notifier: Option<tokio::sync::mpsc::Sender<TaskProgressEvent>>,
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

        let mut task_worker = TaskWorker::new(db.clone())
            .with_worker_id(&worker_id)
            .with_event_broadcaster(event_broadcaster.clone())
            .with_settings_service(settings_service.clone())
            .with_thumbnail_service(thumbnail_service.clone())
            .with_files_config(files_config.clone());

        // Add task metrics service if available
        if let Some(ref metrics) = task_metrics_service {
            task_worker = task_worker.with_task_metrics_service(metrics.clone());
        }

        // Add PDF cache handler if available
        if let Some(ref pdf_cache) = pdf_page_cache {
            task_worker = task_worker.with_pdf_cache(pdf_cache.clone(), settings_service.clone());
        }

        // Wire the PDF handle cache so scanner-triggered file updates can
        // evict cached open PdfDocument handles for changed books.
        if let Some(ref handle_cache) = pdf_handle_cache {
            task_worker = task_worker.with_pdf_handle_cache(handle_cache.clone());
        }

        // Add plugin manager if available (for plugin auto-match tasks)
        if let Some(ref pm) = plugin_manager {
            task_worker = task_worker.with_plugin_manager(pm.clone());
        }

        // Add OAuth state manager if available (for cleaning up expired OAuth flows)
        if let Some(ref osm) = oauth_state_manager {
            task_worker = task_worker.with_oauth_state_manager(osm.clone());
        }

        // Add export storage for series export tasks
        task_worker = task_worker.with_export_storage(export_storage.clone());

        // Bridge task progress to the web server in distributed deployments.
        if let Some(ref notifier) = task_progress_notifier {
            task_worker = task_worker.with_task_progress_notifier(notifier.clone());
        }

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
    use codex_config::{FilesConfig, SQLiteConfig, TaskConfig};
    use codex_db::test_helpers::create_test_db;
    use codex_services::SettingsService;
    use tempfile::TempDir;

    fn sqlite_config_at(db_path: &Path) -> DatabaseConfig {
        DatabaseConfig {
            db_type: DatabaseType::SQLite,
            postgres: None,
            sqlite: Some(SQLiteConfig {
                path: db_path.to_str().unwrap().to_string(),
                pragmas: None,
                ..SQLiteConfig::default()
            }),
        }
    }

    /// Unlink the database out from under an open pool.
    ///
    /// POSIX keeps the inode alive for the already-open handle, so the existing
    /// connection carries on reading the migrated schema, while anything that
    /// opens a *new* connection gets a fresh empty database (the URL uses
    /// `mode=rwc`). That asymmetry is what lets a test tell "reused the
    /// connection it was handed" apart from "opened another one".
    fn unlink_database(db_path: &Path) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = fs::remove_file(format!("{}{}", db_path.display(), suffix));
        }
    }

    /// Put a regular file where the database's parent directory needs to be.
    ///
    /// Opening the database then fails for everyone, root included, until the
    /// blocker is removed — a database that is not up yet, without needing a
    /// server. Removing it makes the very same config connect.
    fn block_database_path(temp_dir: &TempDir) -> (PathBuf, DatabaseConfig) {
        let blocker = temp_dir.path().join("db_dir");
        fs::write(&blocker, b"not a directory").unwrap();
        let config = sqlite_config_at(&blocker.join("codex.db"));
        (blocker, config)
    }

    #[tokio::test]
    async fn connect_with_retry_waits_for_a_database_that_is_not_up_yet() {
        let temp_dir = TempDir::new().unwrap();
        let (blocker, config) = block_database_path(&temp_dir);

        // A database restarting to apply a parameter, or failing over, looks
        // like this: refused now, fine in a moment.
        assert!(Database::new(&config).await.is_err());

        let connecting = tokio::spawn({
            let config = config.clone();
            async move { connect_with_retry(&config, Some(10), Some(1)).await }
        });

        tokio::time::sleep(Duration::from_millis(200)).await;
        fs::remove_file(&blocker).unwrap();

        let result = connecting.await.unwrap();
        assert!(
            result.is_ok(),
            "a database that comes back within the budget must not fail the \
             command: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn connect_with_retry_gives_up_at_the_timeout() {
        let temp_dir = TempDir::new().unwrap();
        let (_blocker, config) = block_database_path(&temp_dir);

        let start = std::time::Instant::now();
        let result = connect_with_retry(&config, Some(2), Some(1)).await;

        assert!(
            result.is_err(),
            "a database that never comes back must fail"
        );
        assert!(start.elapsed() >= Duration::from_secs(2));
    }

    #[tokio::test]
    async fn wait_for_migrations_polls_over_the_connection_it_was_given() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("migrated.db");
        let config = sqlite_config_at(&db_path);

        let db = Database::new(&config).await.unwrap();
        db.run_migrations().await.unwrap();
        // Force the pool to hold a live connection before the file goes away.
        assert!(db.migrations_complete().await.unwrap());

        unlink_database(&db_path);

        // Guard the premise: a fresh connection now lands on an empty database.
        // Without this the test would pass for the wrong reason if unlinking
        // ever stopped separating the two cases.
        let reconnected = Database::new(&config).await.unwrap();
        assert!(
            !reconnected.migrations_complete().await.unwrap(),
            "a new connection should see an unmigrated database"
        );
        drop(reconnected);

        let result = wait_for_migrations_on(&db, Some(2), Some(1)).await;

        assert!(
            result.is_ok(),
            "the wait must poll over the caller's connection rather than opening \
             its own; opening one here lands on an empty database: {result:?}"
        );
    }

    #[tokio::test]
    async fn wait_for_migrations_on_times_out_when_the_schema_never_lands() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("empty.db");
        let config = sqlite_config_at(&db_path);

        // Connected, but no migrations ever run against it.
        let db = Database::new(&config).await.unwrap();

        let start = std::time::Instant::now();
        let result = wait_for_migrations_on(&db, Some(2), Some(1)).await;

        assert!(
            result.is_err(),
            "an unmigrated database must not report ready"
        );
        assert!(start.elapsed() >= Duration::from_secs(2));
    }

    #[test]
    fn pg_budget_fits_when_the_process_and_its_neighbours_leave_room() {
        // 25 + 16 against a stock server with 40 already open: 81 of 97.
        assert_eq!(assess_pg_budget(41, 100, 3, 40), PgBudget::Fits);
    }

    #[test]
    fn pg_budget_flags_a_process_sized_for_the_whole_server() {
        // The old default: one process claiming every slot the server has.
        assert_eq!(
            assess_pg_budget(100, 100, 3, 0),
            PgBudget::ExceedsServer { usable: 97 }
        );
    }

    #[test]
    fn pg_budget_flags_the_replica_that_tips_the_server_over() {
        // Each process fits on its own; together they do not. This is the case a
        // per-process check cannot see, and the one that takes a deployment down.
        assert_eq!(
            assess_pg_budget(25, 100, 3, 80),
            PgBudget::ExceedsRemaining {
                usable: 97,
                in_use: 80
            }
        );
    }

    #[test]
    fn pg_budget_survives_a_server_that_reserves_everything() {
        // saturating_sub, so a nonsense reading warns rather than panicking.
        assert_eq!(
            assess_pg_budget(1, 3, 100, 0),
            PgBudget::ExceedsServer { usable: 0 }
        );
    }

    #[test]
    fn test_ensure_dir_exists_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let new_dir = temp_dir.path().join("new_directory");

        assert!(!new_dir.exists());
        ensure_dir_exists(&new_dir).unwrap();
        assert!(new_dir.exists());
        assert!(new_dir.is_dir());
    }

    #[test]
    fn test_ensure_dir_exists_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        let nested_dir = temp_dir.path().join("level1").join("level2").join("level3");

        assert!(!nested_dir.exists());
        ensure_dir_exists(&nested_dir).unwrap();
        assert!(nested_dir.exists());
        assert!(nested_dir.is_dir());
    }

    #[test]
    fn test_ensure_dir_exists_already_exists() {
        let temp_dir = TempDir::new().unwrap();
        let existing_dir = temp_dir.path().join("existing");
        fs::create_dir(&existing_dir).unwrap();

        assert!(existing_dir.exists());
        // Should not error when directory already exists
        ensure_dir_exists(&existing_dir).unwrap();
        assert!(existing_dir.exists());
    }

    #[test]
    fn test_ensure_parent_dir_exists_creates_parent() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("parent_dir").join("file.txt");

        assert!(!path.parent().unwrap().exists());
        ensure_parent_dir_exists(&path).unwrap();
        assert!(path.parent().unwrap().exists());
        assert!(!path.exists()); // File itself should not be created
    }

    #[test]
    fn test_ensure_parent_dir_exists_nested() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir
            .path()
            .join("a")
            .join("b")
            .join("c")
            .join("file.db");

        assert!(!path.parent().unwrap().exists());
        ensure_parent_dir_exists(&path).unwrap();
        assert!(path.parent().unwrap().exists());
    }

    #[test]
    fn test_ensure_parent_dir_exists_empty_parent() {
        // File in current directory (empty parent)
        let path = Path::new("file.txt");
        // Should not error
        ensure_parent_dir_exists(path).unwrap();
    }

    #[test]
    fn test_ensure_data_directories_creates_all() {
        let temp_dir = TempDir::new().unwrap();
        let thumbnail_dir = temp_dir.path().join("thumbnails");
        let uploads_dir = temp_dir.path().join("uploads");
        let db_path = temp_dir.path().join("data").join("codex.db");
        let pdf_cache_dir = temp_dir.path().join("pdf_cache");

        let config = Config {
            files: FilesConfig {
                thumbnail_dir: thumbnail_dir.to_string_lossy().to_string(),
                uploads_dir: uploads_dir.to_string_lossy().to_string(),
                plugins_dir: temp_dir
                    .path()
                    .join("plugins")
                    .to_string_lossy()
                    .to_string(),
            },
            database: codex_config::DatabaseConfig {
                db_type: codex_config::DatabaseType::SQLite,
                sqlite: Some(SQLiteConfig {
                    path: db_path.to_string_lossy().to_string(),
                    pragmas: None,
                    ..SQLiteConfig::default()
                }),
                postgres: None,
            },
            pdf: codex_config::PdfConfig {
                cache_dir: pdf_cache_dir.to_string_lossy().to_string(),
                ..codex_config::PdfConfig::default()
            },
            ..Config::default()
        };

        let plugins_dir = temp_dir.path().join("plugins");

        assert!(!thumbnail_dir.exists());
        assert!(!uploads_dir.exists());
        assert!(!db_path.parent().unwrap().exists());
        assert!(!pdf_cache_dir.exists());
        assert!(!plugins_dir.exists());

        ensure_data_directories(&config).unwrap();

        assert!(thumbnail_dir.exists());
        assert!(uploads_dir.exists());
        assert!(db_path.parent().unwrap().exists());
        assert!(pdf_cache_dir.exists());
        assert!(plugins_dir.exists());
    }

    fn rename_finding(var: &str) -> codex_config::Finding {
        codex_config::Finding::WillRename {
            var: var.to_string(),
            v2_name: format!("{var}__X"),
            path: "task.worker_count".to_string(),
        }
    }

    fn ignored_finding(var: &str) -> codex_config::Finding {
        codex_config::Finding::NotYetValid {
            var: var.to_string(),
            v1_name: "CODEX_TASK_WORKER_COUNT".to_string(),
        }
    }

    #[test]
    fn env_notice_is_silent_when_nothing_is_wrong() {
        assert_eq!(env_var_notice(&[]), None);
    }

    /// One line regardless of how many variables are involved. A per-variable
    /// warning would be recurring noise about names that are still correct in
    /// this version.
    #[test]
    fn env_notice_is_a_single_line_that_names_no_variables() {
        let findings: Vec<_> = (0..14)
            .map(|i| rename_finding(&format!("CODEX_THING_{i}")))
            .collect();
        let notice = env_var_notice(&findings).expect("should produce a notice");

        assert_eq!(notice.lines().count(), 1, "notice must be one line");
        assert!(
            !notice.contains("CODEX_"),
            "notice must not name variables: {notice}"
        );
        assert!(notice.contains("14"), "notice should carry the count");
        assert!(notice.contains("codex config check"));
    }

    #[test]
    fn env_notice_distinguishes_renames_from_ignored_variables() {
        let renames_only = env_var_notice(&[rename_finding("CODEX_A")]).unwrap();
        assert!(renames_only.contains("renamed in Codex 2.0"));
        assert!(!renames_only.contains("not being read"));

        let ignored_only = env_var_notice(&[ignored_finding("CODEX_B__C")]).unwrap();
        assert!(ignored_only.contains("not being read"));
        assert!(!ignored_only.contains("renamed in Codex 2.0"));

        let both = env_var_notice(&[rename_finding("CODEX_A"), ignored_finding("CODEX_B__C")])
            .expect("should produce a notice");
        assert!(both.contains("renamed in Codex 2.0"));
        assert!(both.contains("not being read"));
        assert_eq!(both.lines().count(), 1);
    }

    /// `resolve_config` backs `codex config check`, which is meant to run
    /// against a read-only mount as an initContainer.
    #[test]
    fn resolve_config_does_not_create_a_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.path().join("absent").join("codex.yaml");

        let config = resolve_config(&missing).unwrap();

        assert!(!missing.exists(), "resolve_config must not write anything");
        assert!(!missing.parent().unwrap().exists());
        assert!(!config.application.host.is_empty());
    }

    #[test]
    fn resolve_config_reads_an_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("codex.yaml");
        fs::write(&path, "application:\n  port: 9123\n").unwrap();

        let config = resolve_config(&path).unwrap();

        assert_eq!(config.application.port, 9123);
    }

    #[test]
    fn test_load_config_creates_parent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config").join("codex.yaml");

        assert!(!config_path.parent().unwrap().exists());

        let (config, created) = load_config(config_path.clone()).unwrap();

        assert!(config_path.parent().unwrap().exists());
        assert!(config_path.exists());
        assert!(created);
        // Verify it's a valid config
        assert!(!config.application.host.is_empty());
    }

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

    fn make_sqlite_config(db_path: &std::path::Path) -> Config {
        Config {
            database: DatabaseConfig {
                db_type: DatabaseType::SQLite,
                postgres: None,
                sqlite: Some(SQLiteConfig {
                    path: db_path.to_str().unwrap().to_string(),
                    pragmas: None,
                    ..SQLiteConfig::default()
                }),
            },
            ..Config::default()
        }
    }

    #[tokio::test]
    #[serial_test::serial(codex_migration_env)]
    async fn init_database_runs_migrations_by_default() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        unsafe {
            std::env::remove_var("CODEX_SKIP_MIGRATIONS");
        }

        let config = make_sqlite_config(&db_path);
        let db = init_database(&config)
            .await
            .expect("init_database should succeed when skip is unset");

        let complete = db.migrations_complete().await.unwrap();
        assert!(complete, "migrations should be complete");
    }

    #[tokio::test]
    #[serial_test::serial(codex_migration_env)]
    async fn init_database_with_skip_succeeds_when_migrations_already_complete() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let config = make_sqlite_config(&db_path);

        // Run migrations out of band first.
        let db = Database::new(&config.database).await.unwrap();
        db.run_migrations().await.unwrap();
        drop(db);

        unsafe {
            std::env::set_var("CODEX_SKIP_MIGRATIONS", "true");
        }
        let result = init_database(&config).await;
        unsafe {
            std::env::remove_var("CODEX_SKIP_MIGRATIONS");
        }

        assert!(
            result.is_ok(),
            "init_database should succeed when skip is set and migrations are done: {:?}",
            result
        );
    }

    #[tokio::test]
    #[serial_test::serial(codex_migration_env)]
    async fn init_database_with_skip_waits_for_concurrent_migrations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        unsafe {
            std::env::set_var("CODEX_SKIP_MIGRATIONS", "true");
            std::env::set_var("CODEX_MIGRATION_WAIT_TIMEOUT", "10");
            std::env::set_var("CODEX_MIGRATION_WAIT_INTERVAL", "1");
        }

        let config = make_sqlite_config(&db_path);

        let config_clone = config.clone();
        let migration_handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let db = Database::new(&config_clone.database).await.unwrap();
            db.run_migrations().await.unwrap();
        });

        let result = init_database(&config).await;
        migration_handle.await.unwrap();

        unsafe {
            std::env::remove_var("CODEX_SKIP_MIGRATIONS");
            std::env::remove_var("CODEX_MIGRATION_WAIT_TIMEOUT");
            std::env::remove_var("CODEX_MIGRATION_WAIT_INTERVAL");
        }

        let db = result.expect("init_database should succeed once migrations complete");
        assert!(db.migrations_complete().await.unwrap());
    }

    #[tokio::test]
    #[serial_test::serial(codex_migration_env)]
    async fn init_database_with_skip_accepts_one_as_truthy() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let config = make_sqlite_config(&db_path);

        let db = Database::new(&config.database).await.unwrap();
        db.run_migrations().await.unwrap();
        drop(db);

        unsafe {
            std::env::set_var("CODEX_SKIP_MIGRATIONS", "1");
        }
        let result = init_database(&config).await;
        unsafe {
            std::env::remove_var("CODEX_SKIP_MIGRATIONS");
        }

        assert!(
            result.is_ok(),
            "'1' should be treated as truthy: {:?}",
            result
        );
    }

    #[tokio::test]
    #[serial_test::serial(codex_migration_env)]
    async fn init_database_with_skip_times_out_when_migrations_never_run() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        unsafe {
            std::env::set_var("CODEX_SKIP_MIGRATIONS", "true");
            std::env::set_var("CODEX_MIGRATION_WAIT_TIMEOUT", "2");
            std::env::set_var("CODEX_MIGRATION_WAIT_INTERVAL", "1");
        }

        let config = make_sqlite_config(&db_path);
        let result = init_database(&config).await;

        unsafe {
            std::env::remove_var("CODEX_SKIP_MIGRATIONS");
            std::env::remove_var("CODEX_MIGRATION_WAIT_TIMEOUT");
            std::env::remove_var("CODEX_MIGRATION_WAIT_INTERVAL");
        }

        let err = result.expect_err("should time out when migrations never complete");
        let msg = err.to_string();
        assert!(
            msg.to_lowercase().contains("timeout"),
            "error should mention timeout: {}",
            msg
        );
    }
}
