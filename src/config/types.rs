use super::env_override::{env_bool_or, env_or, env_string_opt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// TaskConfig must be defined before Config since Config references it
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct TaskConfig {
    /// Number of parallel task workers to process tasks from the queue
    /// This is a startup-time setting - changes require a restart
    pub worker_count: u32,
}

/// Configuration for the Komga-compatible API layer
/// This enables third-party apps like Komic to connect to Codex using the Komga API format
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct KomgaApiConfig {
    /// Enable Komga-compatible API endpoints
    /// When enabled, routes are mounted at /{prefix}/api/v1/*
    pub enabled: bool,

    /// URL prefix for Komga API (default: "komga")
    /// The final URL structure will be: /{prefix}/api/v1/...
    /// Example with default: /komga/api/v1/libraries
    pub prefix: String,
}

fn default_komga_prefix() -> String {
    "komga".to_string()
}

/// Configuration for API rate limiting
/// Uses token bucket algorithm with per-client tracking
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct RateLimitConfig {
    /// Enable rate limiting (default: true)
    pub enabled: bool,

    /// Requests per second for anonymous users (default: 10)
    pub anonymous_rps: u32,

    /// Burst size for anonymous users (default: 50)
    pub anonymous_burst: u32,

    /// Requests per second for authenticated users (default: 50)
    pub authenticated_rps: u32,

    /// Burst size for authenticated users (default: 200)
    pub authenticated_burst: u32,

    /// Glob patterns for paths exempt from rate limiting (e.g. `/api/v1/books/*/thumbnail`)
    pub exempt_paths: Vec<String>,

    /// Cleanup interval in seconds for stale buckets (default: 60)
    pub cleanup_interval_secs: u64,

    /// Time in seconds before a bucket is considered stale (default: 300)
    pub bucket_ttl_secs: u64,
}

fn default_exempt_paths() -> Vec<String> {
    vec![
        "/health".to_string(),
        "/api/v1/events".to_string(),
        "/api/v1/events/**".to_string(),
    ]
}

/// Default role for OIDC users
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OidcDefaultRole {
    Admin,
    Maintainer,
    #[default]
    Reader,
}

impl OidcDefaultRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            OidcDefaultRole::Admin => "admin",
            OidcDefaultRole::Maintainer => "maintainer",
            OidcDefaultRole::Reader => "reader",
        }
    }
}

/// Configuration for a single OIDC provider
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OidcProviderConfig {
    /// Display name shown on login button
    pub display_name: String,

    /// OIDC discovery URL (provider's issuer URL)
    /// e.g., "https://authentik.example.com/application/o/codex/"
    pub issuer_url: String,

    /// OAuth2 client ID
    pub client_id: String,

    /// OAuth2 client secret (optional if using client_secret_env)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,

    /// Environment variable name containing client secret
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret_env: Option<String>,

    /// Scopes to request (openid is always included)
    #[serde(default)]
    pub scopes: Vec<String>,

    /// Group-to-role mapping: role -> [group names]
    /// e.g., {"admin": ["codex-admins"], "reader": ["codex-users"]}
    #[serde(default)]
    pub role_mapping: HashMap<String, Vec<String>>,

    /// Claim containing groups (default: "groups")
    #[serde(default = "default_groups_claim")]
    pub groups_claim: String,

    /// Claim for username (default: "preferred_username")
    #[serde(default = "default_username_claim")]
    pub username_claim: String,

    /// Claim for email (default: "email")
    #[serde(default = "default_email_claim")]
    pub email_claim: String,
}

fn default_groups_claim() -> String {
    "groups".to_string()
}

fn default_username_claim() -> String {
    "preferred_username".to_string()
}

fn default_email_claim() -> String {
    "email".to_string()
}

/// Configuration for OpenID Connect (OIDC) authentication
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct OidcConfig {
    /// Enable OIDC authentication
    pub enabled: bool,

    /// Auto-create users on first OIDC login
    pub auto_create_users: bool,

    /// Default role for new OIDC users (if no group mapping matches)
    pub default_role: OidcDefaultRole,

    /// Public-facing base URL for OIDC redirect URIs
    /// e.g., "https://codex.example.com" or "http://localhost:8080"
    /// If not set, falls back to http://{application.host}:{application.port}
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect_uri_base: Option<String>,

    /// Provider configurations keyed by provider name (e.g., "authentik", "keycloak")
    #[serde(default)]
    pub providers: HashMap<String, OidcProviderConfig>,
}

impl Default for OidcConfig {
    fn default() -> Self {
        Self {
            enabled: env_bool_or("CODEX_AUTH_OIDC_ENABLED", false),
            auto_create_users: env_bool_or("CODEX_AUTH_OIDC_AUTO_CREATE_USERS", true),
            default_role: OidcDefaultRole::Reader,
            redirect_uri_base: std::env::var("CODEX_AUTH_OIDC_REDIRECT_URI_BASE").ok(),
            providers: HashMap::new(),
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: env_bool_or("CODEX_RATE_LIMIT_ENABLED", true),
            anonymous_rps: env_or("CODEX_RATE_LIMIT_ANONYMOUS_RPS", 10),
            anonymous_burst: env_or("CODEX_RATE_LIMIT_ANONYMOUS_BURST", 50),
            authenticated_rps: env_or("CODEX_RATE_LIMIT_AUTHENTICATED_RPS", 50),
            authenticated_burst: env_or("CODEX_RATE_LIMIT_AUTHENTICATED_BURST", 200),
            exempt_paths: env_string_opt("CODEX_RATE_LIMIT_EXEMPT_PATHS")
                .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
                .unwrap_or_else(default_exempt_paths),
            cleanup_interval_secs: env_or("CODEX_RATE_LIMIT_CLEANUP_INTERVAL_SECS", 60),
            bucket_ttl_secs: env_or("CODEX_RATE_LIMIT_BUCKET_TTL_SECS", 300),
        }
    }
}

impl Default for KomgaApiConfig {
    fn default() -> Self {
        Self {
            enabled: env_bool_or("CODEX_KOMGA_API_ENABLED", false),
            prefix: env_string_opt("CODEX_KOMGA_API_PREFIX").unwrap_or_else(default_komga_prefix),
        }
    }
}

impl Default for TaskConfig {
    fn default() -> Self {
        Self {
            worker_count: env_or("CODEX_TASK_WORKER_COUNT", 2),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// Base data directory — all other data dirs derive from this unless overridden.
    /// When set, sub-directories (thumbnails, uploads, plugins, cache, SQLite DB) default
    /// to paths under this directory. Explicit overrides take precedence.
    /// Default: "data" (env: CODEX_DATA_DIR)
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub application: ApplicationConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub email: EmailConfig,
    #[serde(default)]
    pub task: TaskConfig,
    #[serde(default)]
    pub scanner: ScannerConfig,
    #[serde(default)]
    pub scheduler: SchedulerConfig,
    #[serde(default)]
    pub files: FilesConfig,
    #[serde(default)]
    pub pdf: PdfConfig,
    #[serde(default)]
    pub komga_api: KomgaApiConfig,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
}

fn default_data_dir() -> String {
    env_string_opt("CODEX_DATA_DIR").unwrap_or_else(|| "data".to_string())
}

/// Default sub-directory names under data_dir
const DEFAULT_THUMBNAILS_SUBDIR: &str = "thumbnails";
const DEFAULT_UPLOADS_SUBDIR: &str = "uploads";
const DEFAULT_PLUGINS_SUBDIR: &str = "plugins";
const DEFAULT_CACHE_SUBDIR: &str = "cache";
const DEFAULT_SQLITE_FILENAME: &str = "codex.db";

impl Config {
    /// Resolve sub-directory paths relative to `data_dir`.
    ///
    /// For each sub-path (thumbnail_dir, uploads_dir, plugins_dir, cache_dir, sqlite path),
    /// if the value matches the old hardcoded default (e.g., "data/thumbnails") AND no
    /// explicit env var override is set for that field, replace it with `{data_dir}/{subdir}`.
    ///
    /// This ensures backward compatibility: users who never set `data_dir` get the same
    /// paths as before ("data/thumbnails"), while users who set `data_dir: /var/lib/codex`
    /// get "/var/lib/codex/thumbnails" automatically.
    ///
    /// Explicit overrides (env vars or non-default config values) always take precedence.
    pub fn resolve_data_dir(&mut self) {
        let data_dir = &self.data_dir;

        // Helper: build the derived path from data_dir
        let derive = |subdir: &str| -> String { format!("{}/{}", data_dir, subdir) };

        // Helper: check if a field uses the old hardcoded default ("data/{subdir}")
        let is_old_default =
            |value: &str, subdir: &str| -> bool { value == format!("data/{}", subdir) };

        // Resolve files.thumbnail_dir
        if is_old_default(&self.files.thumbnail_dir, DEFAULT_THUMBNAILS_SUBDIR)
            && env_string_opt("CODEX_FILES_THUMBNAIL_DIR").is_none()
        {
            self.files.thumbnail_dir = derive(DEFAULT_THUMBNAILS_SUBDIR);
        }

        // Resolve files.uploads_dir
        if is_old_default(&self.files.uploads_dir, DEFAULT_UPLOADS_SUBDIR)
            && env_string_opt("CODEX_FILES_UPLOADS_DIR").is_none()
        {
            self.files.uploads_dir = derive(DEFAULT_UPLOADS_SUBDIR);
        }

        // Resolve files.plugins_dir
        if is_old_default(&self.files.plugins_dir, DEFAULT_PLUGINS_SUBDIR)
            && env_string_opt("CODEX_FILES_PLUGINS_DIR").is_none()
        {
            self.files.plugins_dir = derive(DEFAULT_PLUGINS_SUBDIR);
        }

        // Resolve pdf.cache_dir
        if is_old_default(&self.pdf.cache_dir, DEFAULT_CACHE_SUBDIR)
            && env_string_opt("CODEX_PDF_CACHE_DIR").is_none()
        {
            self.pdf.cache_dir = derive(DEFAULT_CACHE_SUBDIR);
        }

        // Resolve database.sqlite.path
        if let Some(ref mut sqlite_config) = self.database.sqlite {
            let old_default = format!("data/{}", DEFAULT_SQLITE_FILENAME);
            if sqlite_config.path == old_default
                && env_string_opt("CODEX_DATABASE_SQLITE_PATH").is_none()
            {
                sqlite_config.path = derive(DEFAULT_SQLITE_FILENAME);
            }
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        use std::env;

        let mut pragmas = HashMap::new();
        pragmas.insert("foreign_keys".to_string(), "ON".to_string());
        pragmas.insert("journal_mode".to_string(), "WAL".to_string());

        // Determine database type from environment or use SQLite as default
        let db_type = env::var("CODEX_DATABASE_DB_TYPE")
            .ok()
            .and_then(|t| {
                if t.eq_ignore_ascii_case("postgres") || t.eq_ignore_ascii_case("postgresql") {
                    Some(DatabaseType::Postgres)
                } else if t.eq_ignore_ascii_case("sqlite") {
                    Some(DatabaseType::SQLite)
                } else {
                    None
                }
            })
            .unwrap_or(DatabaseType::SQLite);

        // Build database config based on type
        let (postgres_config, sqlite_config) = match db_type {
            DatabaseType::Postgres => (Some(PostgresConfig::default()), None),
            DatabaseType::SQLite => (
                None,
                Some(SQLiteConfig {
                    pragmas: Some(pragmas),
                    ..SQLiteConfig::default()
                }),
            ),
        };

        Self {
            data_dir: default_data_dir(),
            database: DatabaseConfig {
                db_type,
                postgres: postgres_config,
                sqlite: sqlite_config,
            },
            application: ApplicationConfig::default(),
            logging: LoggingConfig::default(),
            auth: AuthConfig::default(),
            api: ApiConfig::default(),
            email: EmailConfig::default(),
            task: TaskConfig::default(),
            scanner: ScannerConfig::default(),
            scheduler: SchedulerConfig::default(),
            files: FilesConfig::default(),
            pdf: PdfConfig::default(),
            komga_api: KomgaApiConfig::default(),
            rate_limit: RateLimitConfig::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiry_hours: u32,
    pub refresh_token_enabled: bool,
    pub refresh_token_expiry_days: u32,
    pub email_confirmation_required: bool,
    pub argon2_memory_cost: u32,
    pub argon2_time_cost: u32,
    pub argon2_parallelism: u32,
    /// OIDC configuration for external identity provider authentication
    #[serde(default)]
    pub oidc: OidcConfig,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "INSECURE_DEFAULT_SECRET_CHANGE_IN_PRODUCTION".to_string(),
            jwt_expiry_hours: 24,
            refresh_token_enabled: false,
            refresh_token_expiry_days: 30,
            email_confirmation_required: env_bool_or(
                "CODEX_AUTH_EMAIL_CONFIRMATION_REQUIRED",
                false,
            ),
            argon2_memory_cost: 19456,
            argon2_time_cost: 2,
            argon2_parallelism: 1,
            oidc: OidcConfig::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct ApiConfig {
    pub base_path: String,
    pub enable_api_docs: bool,
    pub api_docs_path: String,
    pub cors_enabled: bool,
    pub cors_origins: Vec<String>,
    pub max_page_size: usize,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            base_path: env_string_opt("CODEX_API_BASE_PATH")
                .unwrap_or_else(|| "/api/v1".to_string()),
            enable_api_docs: env_bool_or("CODEX_API_ENABLE_API_DOCS", false),
            api_docs_path: env_string_opt("CODEX_API_DOCS_PATH")
                .unwrap_or_else(|| "/docs".to_string()),
            cors_enabled: env_bool_or("CODEX_API_CORS_ENABLED", true),
            cors_origins: env_string_opt("CODEX_API_CORS_ORIGINS")
                .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_else(|| vec!["*".to_string()]),
            max_page_size: env_or("CODEX_API_MAX_PAGE_SIZE", 100),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct DatabaseConfig {
    pub db_type: DatabaseType,

    // Postgres Specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postgres: Option<PostgresConfig>,

    // SQLite Specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sqlite: Option<SQLiteConfig>,
}

impl DatabaseConfig {
    /// Get the operation deadline in seconds for the current database type
    ///
    /// Returns the configured operation deadline, or a default of 30 seconds.
    pub fn operation_deadline_seconds(&self) -> u64 {
        match self.db_type {
            DatabaseType::Postgres => self
                .postgres
                .as_ref()
                .map(|c| c.operation_deadline_seconds)
                .unwrap_or(30),
            DatabaseType::SQLite => self
                .sqlite
                .as_ref()
                .map(|c| c.operation_deadline_seconds)
                .unwrap_or(30),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        use std::env;

        // Determine database type from environment or use SQLite as default
        let db_type = env::var("CODEX_DATABASE_DB_TYPE")
            .ok()
            .and_then(|t| {
                if t.eq_ignore_ascii_case("postgres") || t.eq_ignore_ascii_case("postgresql") {
                    Some(DatabaseType::Postgres)
                } else if t.eq_ignore_ascii_case("sqlite") {
                    Some(DatabaseType::SQLite)
                } else {
                    None
                }
            })
            .unwrap_or(DatabaseType::SQLite);

        // Build database config based on type
        let (postgres_config, sqlite_config) = match db_type {
            DatabaseType::Postgres => (Some(PostgresConfig::default()), None),
            DatabaseType::SQLite => (None, Some(SQLiteConfig::default())),
        };

        Self {
            db_type,
            postgres: postgres_config,
            sqlite: sqlite_config,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum DatabaseType {
    Postgres,
    #[default]
    SQLite,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database_name: String,

    // Connection Pool Settings
    /// Maximum number of connections in the pool (default: 100)
    /// PostgreSQL handles concurrent connections well, so higher values are fine
    pub max_connections: u32,

    /// Minimum number of connections to maintain in the pool (default: 5)
    /// Keeps connections warm for better latency
    pub min_connections: u32,

    /// Connection acquire timeout in seconds (default: 30)
    /// How long to wait for a connection before failing
    pub acquire_timeout_seconds: u64,

    /// Idle connection timeout in seconds (default: 600 = 10 minutes)
    /// Network connections are expensive to establish, keep them longer
    pub idle_timeout_seconds: u64,

    /// Maximum lifetime of a connection in seconds (default: 3600 = 1 hour)
    /// Prevents stale connections from accumulating
    pub max_lifetime_seconds: u64,

    /// Operation deadline in seconds (default: 30)
    /// Maximum time a database operation can hold a connection before timing out.
    /// Prevents indefinite connection holds from slow queries or stuck operations.
    pub operation_deadline_seconds: u64,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            host: env_string_opt("CODEX_DATABASE_POSTGRES_HOST")
                .unwrap_or_else(|| "localhost".to_string()),
            port: env_or("CODEX_DATABASE_POSTGRES_PORT", 5432),
            username: env_string_opt("CODEX_DATABASE_POSTGRES_USERNAME")
                .unwrap_or_else(|| "codex".to_string()),
            password: env_string_opt("CODEX_DATABASE_POSTGRES_PASSWORD")
                .unwrap_or_else(|| "codex".to_string()),
            database_name: env_string_opt("CODEX_DATABASE_POSTGRES_DATABASE_NAME")
                .unwrap_or_else(|| "codex".to_string()),
            // Pool settings - PostgreSQL can handle more concurrent connections
            max_connections: env_or("CODEX_DATABASE_POSTGRES_MAX_CONNECTIONS", 100),
            min_connections: env_or("CODEX_DATABASE_POSTGRES_MIN_CONNECTIONS", 5),
            acquire_timeout_seconds: env_or("CODEX_DATABASE_POSTGRES_ACQUIRE_TIMEOUT", 30),
            idle_timeout_seconds: env_or("CODEX_DATABASE_POSTGRES_IDLE_TIMEOUT", 600),
            max_lifetime_seconds: env_or("CODEX_DATABASE_POSTGRES_MAX_LIFETIME", 3600),
            operation_deadline_seconds: env_or("CODEX_DATABASE_POSTGRES_OPERATION_DEADLINE", 30),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct SQLiteConfig {
    pub path: String,
    pub pragmas: Option<HashMap<String, String>>,

    // Connection Pool Settings
    /// Maximum number of connections in the pool (default: 16)
    /// SQLite with WAL mode handles concurrent reads well, but writes are serialized.
    /// 16 connections is enough for most workloads without overwhelming the single-writer lock.
    pub max_connections: u32,

    /// Minimum number of connections to maintain in the pool (default: 2)
    /// Keep a couple warm connections ready
    pub min_connections: u32,

    /// Connection acquire timeout in seconds (default: 30)
    /// How long to wait for a connection before failing
    pub acquire_timeout_seconds: u64,

    /// Idle connection timeout in seconds (default: 300 = 5 minutes)
    /// SQLite connections are cheap, can timeout sooner
    pub idle_timeout_seconds: u64,

    /// Maximum lifetime of a connection in seconds (default: 1800 = 30 minutes)
    /// Reasonable for file-based database
    pub max_lifetime_seconds: u64,

    /// Operation deadline in seconds (default: 30)
    /// Maximum time a database operation can hold a connection before timing out.
    /// Prevents indefinite connection holds from slow queries or stuck operations.
    pub operation_deadline_seconds: u64,
}

impl Default for SQLiteConfig {
    fn default() -> Self {
        let mut pragmas = HashMap::new();
        // Note: foreign_keys is enforced at connection time in connection.rs
        // We include it here for documentation, but it's always ON regardless
        pragmas.insert("foreign_keys".to_string(), "ON".to_string());
        pragmas.insert("journal_mode".to_string(), "WAL".to_string());

        Self {
            path: env_string_opt("CODEX_DATABASE_SQLITE_PATH")
                .unwrap_or_else(|| "data/codex.db".to_string()),
            pragmas: Some(pragmas),
            // Pool settings - SQLite is more conservative due to single-writer lock
            max_connections: env_or("CODEX_DATABASE_SQLITE_MAX_CONNECTIONS", 16),
            min_connections: env_or("CODEX_DATABASE_SQLITE_MIN_CONNECTIONS", 2),
            acquire_timeout_seconds: env_or("CODEX_DATABASE_SQLITE_ACQUIRE_TIMEOUT", 30),
            idle_timeout_seconds: env_or("CODEX_DATABASE_SQLITE_IDLE_TIMEOUT", 300),
            max_lifetime_seconds: env_or("CODEX_DATABASE_SQLITE_MAX_LIFETIME", 1800),
            operation_deadline_seconds: env_or("CODEX_DATABASE_SQLITE_OPERATION_DEADLINE", 30),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct ApplicationConfig {
    pub host: String,
    pub port: u16,
    /// Public-facing base URL for the application (e.g., "https://codex.example.com")
    /// Used as a fallback for OIDC redirect URIs and email verification links.
    /// If not set, falls back to http://{host}:{port}
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl ApplicationConfig {
    /// Returns the effective base URL for external-facing links.
    ///
    /// Priority: base_url config > http://{host}:{port} fallback
    pub fn effective_base_url(&self) -> String {
        self.base_url
            .clone()
            .unwrap_or_else(|| format!("http://{}:{}", self.host, self.port))
    }
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        Self {
            host: env_string_opt("CODEX_APPLICATION_HOST").unwrap_or_else(|| "0.0.0.0".to_string()),
            port: env_or("CODEX_APPLICATION_PORT", 8080),
            base_url: env_string_opt("CODEX_APPLICATION_BASE_URL"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: LogLevel,
    pub console: bool,
    pub file: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: env_string_opt("CODEX_LOGGING_LEVEL")
                .and_then(|s| match s.to_lowercase().as_str() {
                    "error" => Some(LogLevel::Error),
                    "warn" => Some(LogLevel::Warn),
                    "info" => Some(LogLevel::Info),
                    "debug" => Some(LogLevel::Debug),
                    "trace" => Some(LogLevel::Trace),
                    _ => None,
                })
                .unwrap_or(LogLevel::Info),
            console: env_bool_or("CODEX_LOGGING_CONSOLE", true),
            file: env_string_opt("CODEX_LOGGING_FILE"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct ScannerConfig {
    /// Maximum number of concurrent library scans
    /// This is a startup-time setting - changes require a restart
    pub max_concurrent_scans: usize,
    // Note: scan_timeout_minutes and retry_failed_files remain in database
    // as they are runtime-configurable
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_scans: env_or("CODEX_SCANNER_MAX_CONCURRENT_SCANS", 2),
        }
    }
}

/// Configuration for the job scheduler
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct SchedulerConfig {
    /// Default IANA timezone for all cron schedules (e.g., "America/Los_Angeles").
    /// Individual libraries can override this via their scanning config's `cronTimezone`.
    /// Defaults to "UTC" for backward compatibility.
    /// This is a startup-time setting - changes require a restart.
    pub timezone: String,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            timezone: env_string_opt("CODEX_SCHEDULER_TIMEZONE")
                .unwrap_or_else(|| "UTC".to_string()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct FilesConfig {
    /// Full path to thumbnail cache directory
    /// This is a startup-time setting - changes require a restart
    pub thumbnail_dir: String,

    /// Full path to uploads directory for user-uploaded files (covers, etc.)
    /// This is a startup-time setting - changes require a restart
    pub uploads_dir: String,

    /// Full path to plugins data directory for plugin-specific file storage
    /// Each plugin gets an isolated subdirectory under this path.
    /// This is a startup-time setting - changes require a restart
    pub plugins_dir: String,
}

impl Default for FilesConfig {
    fn default() -> Self {
        Self {
            thumbnail_dir: env_string_opt("CODEX_FILES_THUMBNAIL_DIR")
                .unwrap_or_else(|| "data/thumbnails".to_string()),
            uploads_dir: env_string_opt("CODEX_FILES_UPLOADS_DIR")
                .unwrap_or_else(|| "data/uploads".to_string()),
            plugins_dir: env_string_opt("CODEX_FILES_PLUGINS_DIR")
                .unwrap_or_else(|| "data/plugins".to_string()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_from_email: String,
    pub smtp_from_name: String,
    pub verification_token_expiry_hours: u32,
    /// Base URL for email verification links (e.g., "https://codex.example.com")
    /// If not set, falls back to application.base_url, then http://{host}:{port}
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_url_base: Option<String>,
}

/// PDF rendering configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct PdfConfig {
    /// Path to PDFium library (optional)
    /// If not set, will search current directory and system paths
    pub pdfium_library_path: Option<String>,

    /// Default render DPI (72-300, default 150)
    /// Higher values produce better quality but larger files
    pub render_dpi: u16,

    /// JPEG quality for rendered pages (1-100, default 85)
    pub jpeg_quality: u8,

    /// Enable rendered page caching (default: true)
    pub cache_rendered_pages: bool,

    /// Directory for caching rendered PDF pages (default: data/cache)
    pub cache_dir: String,
}

impl Default for PdfConfig {
    fn default() -> Self {
        Self {
            pdfium_library_path: env_string_opt("CODEX_PDF_PDFIUM_LIBRARY_PATH"),
            render_dpi: env_or("CODEX_PDF_RENDER_DPI", 150),
            jpeg_quality: env_or("CODEX_PDF_JPEG_QUALITY", 85),
            cache_rendered_pages: env_bool_or("CODEX_PDF_CACHE_RENDERED_PAGES", true),
            cache_dir: env_string_opt("CODEX_PDF_CACHE_DIR")
                .unwrap_or_else(|| "data/cache".to_string()),
        }
    }
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            smtp_host: env_string_opt("CODEX_EMAIL_SMTP_HOST")
                .unwrap_or_else(|| "localhost".to_string()),
            smtp_port: env_or("CODEX_EMAIL_SMTP_PORT", 587),
            smtp_username: env_string_opt("CODEX_EMAIL_SMTP_USERNAME").unwrap_or_default(),
            smtp_password: env_string_opt("CODEX_EMAIL_SMTP_PASSWORD").unwrap_or_default(),
            smtp_from_email: env_string_opt("CODEX_EMAIL_SMTP_FROM_EMAIL")
                .unwrap_or_else(|| "noreply@example.com".to_string()),
            smtp_from_name: env_string_opt("CODEX_EMAIL_SMTP_FROM_NAME")
                .unwrap_or_else(|| "Codex".to_string()),
            verification_token_expiry_hours: env_or(
                "CODEX_EMAIL_VERIFICATION_TOKEN_EXPIRY_HOURS",
                24,
            ),
            verification_url_base: env_string_opt("CODEX_EMAIL_VERIFICATION_URL_BASE"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_database_type_serialization() {
        let db_type = DatabaseType::Postgres;
        let serialized = serde_yaml::to_string(&db_type).unwrap();
        assert!(serialized.contains("postgres"));

        let db_type = DatabaseType::SQLite;
        let serialized = serde_yaml::to_string(&db_type).unwrap();
        assert!(serialized.contains("sqlite"));
    }

    #[test]
    fn test_database_type_deserialization() {
        let yaml = "postgres";
        let db_type: DatabaseType = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(db_type, DatabaseType::Postgres));

        let yaml = "sqlite";
        let db_type: DatabaseType = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(db_type, DatabaseType::SQLite));
    }

    #[test]
    fn test_postgres_config() {
        let config = PostgresConfig {
            host: "localhost".to_string(),
            port: 5432,
            username: "user".to_string(),
            password: "pass".to_string(),
            database_name: "codex".to_string(),
            ..PostgresConfig::default()
        };

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5432);
        assert_eq!(config.database_name, "codex");
    }

    #[test]
    fn test_sqlite_config() {
        let config = SQLiteConfig {
            path: "/var/lib/codex.db".to_string(),
            pragmas: None,
            ..SQLiteConfig::default()
        };

        assert_eq!(config.path, "/var/lib/codex.db");
        assert!(config.pragmas.is_none());
    }

    #[test]
    fn test_sqlite_config_with_pragmas() {
        let mut pragmas = HashMap::new();
        pragmas.insert("journal_mode".to_string(), "WAL".to_string());

        let config = SQLiteConfig {
            path: "/var/lib/codex.db".to_string(),
            pragmas: Some(pragmas),
            ..SQLiteConfig::default()
        };

        assert!(config.pragmas.is_some());
        assert_eq!(config.pragmas.unwrap().get("journal_mode").unwrap(), "WAL");
    }

    #[test]
    fn test_application_config() {
        let config = ApplicationConfig {
            host: "0.0.0.0".to_string(),
            port: 8080,
            base_url: None,
        };

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert!(config.base_url.is_none());
    }

    #[test]
    fn test_effective_base_url_with_explicit_base_url() {
        let config = ApplicationConfig {
            host: "0.0.0.0".to_string(),
            port: 8080,
            base_url: Some("https://codex.example.com".to_string()),
        };

        assert_eq!(config.effective_base_url(), "https://codex.example.com");
    }

    #[test]
    fn test_effective_base_url_fallback_to_host_port() {
        let config = ApplicationConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
            base_url: None,
        };

        assert_eq!(config.effective_base_url(), "http://127.0.0.1:3000");
    }

    #[test]
    fn test_application_config_base_url_serialization() {
        let config = ApplicationConfig {
            host: "0.0.0.0".to_string(),
            port: 8080,
            base_url: Some("https://codex.example.com".to_string()),
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("base_url"));
        assert!(yaml.contains("https://codex.example.com"));

        let deserialized: ApplicationConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(
            deserialized.base_url,
            Some("https://codex.example.com".to_string())
        );
    }

    #[test]
    fn test_application_config_base_url_omitted_when_none() {
        let config = ApplicationConfig {
            host: "0.0.0.0".to_string(),
            port: 8080,
            base_url: None,
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(!yaml.contains("base_url"));
    }

    #[test]
    fn test_application_config_base_url_from_yaml() {
        let yaml_content = r#"
host: 0.0.0.0
port: 8080
base_url: https://library.example.com
"#;

        let config: ApplicationConfig = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(
            config.base_url,
            Some("https://library.example.com".to_string())
        );
        assert_eq!(config.effective_base_url(), "https://library.example.com");
    }

    #[test]
    fn test_application_config_base_url_defaults_to_none() {
        let yaml_content = r#"
host: 0.0.0.0
port: 8080
"#;

        let config: ApplicationConfig = serde_yaml::from_str(yaml_content).unwrap();
        assert!(config.base_url.is_none());
        assert_eq!(config.effective_base_url(), "http://0.0.0.0:8080");
    }

    #[test]
    fn test_email_config_verification_url_base_optional() {
        // When verification_url_base is not set, it should be None
        let yaml_content = r#"
smtp_host: localhost
smtp_port: 587
smtp_username: ""
smtp_password: ""
smtp_from_email: noreply@example.com
smtp_from_name: Codex
verification_token_expiry_hours: 24
"#;

        let config: EmailConfig = serde_yaml::from_str(yaml_content).unwrap();
        assert!(config.verification_url_base.is_none());
    }

    #[test]
    fn test_email_config_verification_url_base_explicit() {
        let yaml_content = r#"
smtp_host: localhost
smtp_port: 587
smtp_username: ""
smtp_password: ""
smtp_from_email: noreply@example.com
smtp_from_name: Codex
verification_token_expiry_hours: 24
verification_url_base: https://codex.example.com
"#;

        let config: EmailConfig = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(
            config.verification_url_base,
            Some("https://codex.example.com".to_string())
        );
    }

    #[test]
    fn test_database_config_postgres() {
        let config = DatabaseConfig {
            db_type: DatabaseType::Postgres,
            postgres: Some(PostgresConfig {
                host: "localhost".to_string(),
                port: 5432,
                username: "user".to_string(),
                password: "pass".to_string(),
                database_name: "codex".to_string(),
                ..PostgresConfig::default()
            }),
            sqlite: None,
        };

        assert!(matches!(config.db_type, DatabaseType::Postgres));
        assert!(config.postgres.is_some());
        assert!(config.sqlite.is_none());
    }

    #[test]
    fn test_database_config_sqlite() {
        let config = DatabaseConfig {
            db_type: DatabaseType::SQLite,
            postgres: None,
            sqlite: Some(SQLiteConfig {
                path: "/var/lib/codex.db".to_string(),
                pragmas: None,
                ..SQLiteConfig::default()
            }),
        };

        assert!(matches!(config.db_type, DatabaseType::SQLite));
        assert!(config.postgres.is_none());
        assert!(config.sqlite.is_some());
    }

    #[test]
    fn test_operation_deadline_seconds_sqlite() {
        let config = DatabaseConfig {
            db_type: DatabaseType::SQLite,
            postgres: None,
            sqlite: Some(SQLiteConfig {
                path: "./test.db".to_string(),
                pragmas: None,
                operation_deadline_seconds: 45,
                ..SQLiteConfig::default()
            }),
        };

        assert_eq!(config.operation_deadline_seconds(), 45);
    }

    #[test]
    fn test_operation_deadline_seconds_postgres() {
        let config = DatabaseConfig {
            db_type: DatabaseType::Postgres,
            postgres: Some(PostgresConfig {
                host: "localhost".to_string(),
                port: 5432,
                username: "user".to_string(),
                password: "pass".to_string(),
                database_name: "codex".to_string(),
                operation_deadline_seconds: 60,
                ..PostgresConfig::default()
            }),
            sqlite: None,
        };

        assert_eq!(config.operation_deadline_seconds(), 60);
    }

    #[test]
    fn test_operation_deadline_seconds_default() {
        // Test default values (should be 30 seconds)
        let config = DatabaseConfig::default();
        assert_eq!(config.operation_deadline_seconds(), 30);
    }

    #[test]
    fn test_full_config() {
        let config = Config {
            data_dir: "data".to_string(),
            database: DatabaseConfig {
                db_type: DatabaseType::SQLite,
                postgres: None,
                sqlite: Some(SQLiteConfig {
                    path: "./codex.db".to_string(),
                    pragmas: None,
                    ..SQLiteConfig::default()
                }),
            },
            application: ApplicationConfig {
                host: "127.0.0.1".to_string(),
                port: 3000,
                base_url: None,
            },
            logging: LoggingConfig::default(),
            auth: AuthConfig::default(),
            api: ApiConfig::default(),
            email: EmailConfig::default(),
            task: TaskConfig::default(),
            scanner: ScannerConfig::default(),
            scheduler: SchedulerConfig::default(),
            files: FilesConfig::default(),
            pdf: PdfConfig::default(),
            komga_api: KomgaApiConfig::default(),
            rate_limit: RateLimitConfig::default(),
        };

        // Application name moved to database settings
        assert_eq!(config.application.port, 3000);
        assert!(matches!(config.database.db_type, DatabaseType::SQLite));
    }

    #[test]
    fn test_files_config_default() {
        let config = FilesConfig::default();
        assert_eq!(config.thumbnail_dir, "data/thumbnails");
        assert_eq!(config.uploads_dir, "data/uploads");
        assert_eq!(config.plugins_dir, "data/plugins");
    }

    #[test]
    fn test_auth_config_default() {
        let config = AuthConfig::default();

        // JWT config
        assert_eq!(config.jwt_expiry_hours, 24);
        assert!(!config.refresh_token_enabled);
        assert_eq!(config.refresh_token_expiry_days, 30);

        // Argon2 config
        assert_eq!(config.argon2_memory_cost, 19456);
        assert_eq!(config.argon2_time_cost, 2);
        assert_eq!(config.argon2_parallelism, 1);

        // JWT secret should exist (even if it's the default warning value)
        assert!(!config.jwt_secret.is_empty());
    }

    #[test]
    fn test_api_config_default() {
        let config = ApiConfig::default();

        assert_eq!(config.base_path, "/api/v1");
        assert!(!config.enable_api_docs); // Disabled by default
        assert_eq!(config.api_docs_path, "/docs");
        assert!(config.cors_enabled);
        assert_eq!(config.cors_origins, vec!["*".to_string()]);
        assert_eq!(config.max_page_size, 100);
    }

    #[test]
    fn test_auth_config_serialization() {
        let config = AuthConfig {
            jwt_secret: "test-secret".to_string(),
            jwt_expiry_hours: 48,
            refresh_token_enabled: true,
            refresh_token_expiry_days: 60,
            email_confirmation_required: false,
            argon2_memory_cost: 20000,
            argon2_time_cost: 3,
            argon2_parallelism: 2,
            oidc: OidcConfig::default(),
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("test-secret"));
        assert!(yaml.contains("48"));

        let deserialized: AuthConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.jwt_secret, "test-secret");
        assert_eq!(deserialized.jwt_expiry_hours, 48);
        assert!(deserialized.refresh_token_enabled);
    }

    #[test]
    fn test_api_config_serialization() {
        let config = ApiConfig {
            base_path: "/api/v2".to_string(),
            enable_api_docs: true,
            api_docs_path: "/api-docs".to_string(),
            cors_enabled: false,
            cors_origins: vec!["https://example.com".to_string()],
            max_page_size: 200,
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("/api/v2"));
        assert!(yaml.contains("true")); // enable_api_docs

        let deserialized: ApiConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.base_path, "/api/v2");
        assert!(deserialized.enable_api_docs);
        assert_eq!(deserialized.max_page_size, 200);
    }

    #[test]
    fn test_task_config_default() {
        let config = TaskConfig::default();
        assert_eq!(config.worker_count, 2);
    }

    #[test]
    fn test_task_config_serialization() {
        let config = TaskConfig { worker_count: 8 };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("worker_count"));
        assert!(yaml.contains("8"));

        let deserialized: TaskConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.worker_count, 8);
    }

    #[test]
    fn test_scanner_config_default() {
        let config = ScannerConfig::default();
        assert_eq!(config.max_concurrent_scans, 2);
    }

    #[test]
    fn test_scanner_config_serialization() {
        let config = ScannerConfig {
            max_concurrent_scans: 6,
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("max_concurrent_scans"));
        assert!(yaml.contains("6"));

        let deserialized: ScannerConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.max_concurrent_scans, 6);
    }

    #[test]
    fn test_scheduler_config_default() {
        let config = SchedulerConfig::default();
        assert_eq!(config.timezone, "UTC");
    }

    #[test]
    fn test_scheduler_config_serialization() {
        let config = SchedulerConfig {
            timezone: "America/Los_Angeles".to_string(),
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("timezone"));
        assert!(yaml.contains("America/Los_Angeles"));

        let deserialized: SchedulerConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.timezone, "America/Los_Angeles");
    }

    #[test]
    fn test_scheduler_config_from_yaml() {
        let yaml_content = r#"
timezone: "Europe/London"
"#;

        let config: SchedulerConfig = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(config.timezone, "Europe/London");
    }

    #[test]
    fn test_config_with_scheduler() {
        let yaml_content = r#"
database:
  db_type: sqlite
  sqlite:
    path: ./test.db
scheduler:
  timezone: "America/New_York"
"#;

        let config: Config = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(config.scheduler.timezone, "America/New_York");
    }

    #[test]
    fn test_config_scheduler_uses_defaults() {
        let yaml_content = r#"
database:
  db_type: sqlite
  sqlite:
    path: ./test.db
"#;

        let config: Config = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(config.scheduler.timezone, "UTC");
    }

    #[test]
    fn test_config_with_task_and_scanner() {
        let yaml_content = r#"
database:
  db_type: sqlite
  sqlite:
    path: ./test.db
task:
  worker_count: 8
scanner:
  max_concurrent_scans: 4
"#;

        let config: Config = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(config.task.worker_count, 8);
        assert_eq!(config.scanner.max_concurrent_scans, 4);
    }

    #[test]
    fn test_pdf_config_default() {
        let config = PdfConfig::default();
        assert_eq!(config.render_dpi, 150);
        assert_eq!(config.jpeg_quality, 85);
        assert!(config.cache_rendered_pages);
        assert_eq!(config.cache_dir, "data/cache");
        // pdfium_library_path is None by default (unless env var is set)
    }

    #[test]
    fn test_pdf_config_serialization() {
        let config = PdfConfig {
            pdfium_library_path: Some("/usr/lib/libpdfium.so".to_string()),
            render_dpi: 200,
            jpeg_quality: 90,
            cache_rendered_pages: false,
            cache_dir: "/var/cache/codex".to_string(),
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("pdfium_library_path"));
        assert!(yaml.contains("render_dpi"));
        assert!(yaml.contains("200"));

        let deserialized: PdfConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(
            deserialized.pdfium_library_path,
            Some("/usr/lib/libpdfium.so".to_string())
        );
        assert_eq!(deserialized.render_dpi, 200);
        assert_eq!(deserialized.jpeg_quality, 90);
        assert!(!deserialized.cache_rendered_pages);
        assert_eq!(deserialized.cache_dir, "/var/cache/codex");
    }

    #[test]
    fn test_config_with_pdf() {
        let yaml_content = r#"
database:
  db_type: sqlite
  sqlite:
    path: ./test.db
pdf:
  render_dpi: 300
  jpeg_quality: 95
  cache_rendered_pages: true
"#;

        let config: Config = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(config.pdf.render_dpi, 300);
        assert_eq!(config.pdf.jpeg_quality, 95);
        assert!(config.pdf.cache_rendered_pages);
    }

    #[test]
    fn test_komga_api_config_default() {
        let config = KomgaApiConfig::default();
        // Disabled by default for security
        assert!(!config.enabled);
        // Default prefix is "komga"
        assert_eq!(config.prefix, "komga");
    }

    #[test]
    fn test_komga_api_config_serialization() {
        let config = KomgaApiConfig {
            enabled: true,
            prefix: "custom_prefix".to_string(),
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("enabled"));
        assert!(yaml.contains("true"));
        assert!(yaml.contains("custom_prefix"));

        let deserialized: KomgaApiConfig = serde_yaml::from_str(&yaml).unwrap();
        assert!(deserialized.enabled);
        assert_eq!(deserialized.prefix, "custom_prefix");
    }

    #[test]
    fn test_komga_api_config_from_yaml() {
        let yaml_content = r#"
enabled: true
prefix: "mykomga"
"#;

        let config: KomgaApiConfig = serde_yaml::from_str(yaml_content).unwrap();
        assert!(config.enabled);
        assert_eq!(config.prefix, "mykomga");
    }

    #[test]
    fn test_komga_api_config_default_when_empty_yaml() {
        let yaml_content = "{}";

        let config: KomgaApiConfig = serde_yaml::from_str(yaml_content).unwrap();
        // Should use defaults when not specified
        assert!(!config.enabled);
        assert_eq!(config.prefix, "komga");
    }

    #[test]
    fn test_config_includes_komga_api() {
        let yaml_content = r#"
database:
  db_type: sqlite
  sqlite:
    path: ./test.db
komga_api:
  enabled: true
  prefix: "komga"
"#;

        let config: Config = serde_yaml::from_str(yaml_content).unwrap();
        assert!(config.komga_api.enabled);
        assert_eq!(config.komga_api.prefix, "komga");
    }

    #[test]
    fn test_config_komga_api_uses_defaults() {
        // When komga_api is not specified, it should use defaults
        let yaml_content = r#"
database:
  db_type: sqlite
  sqlite:
    path: ./test.db
"#;

        let config: Config = serde_yaml::from_str(yaml_content).unwrap();
        assert!(!config.komga_api.enabled);
        assert_eq!(config.komga_api.prefix, "komga");
    }

    #[test]
    #[serial]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        // Enabled by default for security
        assert!(config.enabled);
        // Default rate limits
        assert_eq!(config.anonymous_rps, 10);
        assert_eq!(config.anonymous_burst, 50);
        assert_eq!(config.authenticated_rps, 50);
        assert_eq!(config.authenticated_burst, 200);
        // Default exempt paths
        assert_eq!(
            config.exempt_paths,
            vec![
                "/health".to_string(),
                "/api/v1/events".to_string(),
                "/api/v1/events/**".to_string(),
            ]
        );
        // Default cleanup settings
        assert_eq!(config.cleanup_interval_secs, 60);
        assert_eq!(config.bucket_ttl_secs, 300);
    }

    #[test]
    fn test_rate_limit_config_serialization() {
        let config = RateLimitConfig {
            enabled: false,
            anonymous_rps: 20,
            anonymous_burst: 100,
            authenticated_rps: 100,
            authenticated_burst: 500,
            exempt_paths: vec!["/custom".to_string()],
            cleanup_interval_secs: 120,
            bucket_ttl_secs: 600,
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("enabled"));
        assert!(yaml.contains("false"));
        assert!(yaml.contains("anonymous_rps"));
        assert!(yaml.contains("20"));

        let deserialized: RateLimitConfig = serde_yaml::from_str(&yaml).unwrap();
        assert!(!deserialized.enabled);
        assert_eq!(deserialized.anonymous_rps, 20);
        assert_eq!(deserialized.anonymous_burst, 100);
        assert_eq!(deserialized.authenticated_rps, 100);
        assert_eq!(deserialized.authenticated_burst, 500);
        assert_eq!(deserialized.exempt_paths, vec!["/custom".to_string()]);
        assert_eq!(deserialized.cleanup_interval_secs, 120);
        assert_eq!(deserialized.bucket_ttl_secs, 600);
    }

    #[test]
    fn test_rate_limit_config_from_yaml() {
        let yaml_content = r#"
enabled: true
anonymous_rps: 5
anonymous_burst: 25
authenticated_rps: 25
authenticated_burst: 100
exempt_paths:
  - /health
  - /metrics
cleanup_interval_secs: 30
bucket_ttl_secs: 180
"#;

        let config: RateLimitConfig = serde_yaml::from_str(yaml_content).unwrap();
        assert!(config.enabled);
        assert_eq!(config.anonymous_rps, 5);
        assert_eq!(config.anonymous_burst, 25);
        assert_eq!(config.authenticated_rps, 25);
        assert_eq!(config.authenticated_burst, 100);
        assert_eq!(
            config.exempt_paths,
            vec!["/health".to_string(), "/metrics".to_string()]
        );
        assert_eq!(config.cleanup_interval_secs, 30);
        assert_eq!(config.bucket_ttl_secs, 180);
    }

    #[test]
    #[serial]
    fn test_rate_limit_config_default_when_empty_yaml() {
        let yaml_content = "{}";

        let config: RateLimitConfig = serde_yaml::from_str(yaml_content).unwrap();
        // Should use defaults when not specified
        assert!(config.enabled);
        assert_eq!(config.anonymous_rps, 10);
        assert_eq!(config.anonymous_burst, 50);
    }

    #[test]
    fn test_config_includes_rate_limit() {
        let yaml_content = r#"
database:
  db_type: sqlite
  sqlite:
    path: ./test.db
rate_limit:
  enabled: false
  anonymous_rps: 5
"#;

        let config: Config = serde_yaml::from_str(yaml_content).unwrap();
        assert!(!config.rate_limit.enabled);
        assert_eq!(config.rate_limit.anonymous_rps, 5);
    }

    #[test]
    #[serial]
    fn test_config_rate_limit_uses_defaults() {
        // When rate_limit is not specified, it should use defaults
        let yaml_content = r#"
database:
  db_type: sqlite
  sqlite:
    path: ./test.db
"#;

        let config: Config = serde_yaml::from_str(yaml_content).unwrap();
        assert!(config.rate_limit.enabled);
        assert_eq!(config.rate_limit.anonymous_rps, 10);
        assert_eq!(config.rate_limit.authenticated_rps, 50);
    }

    // OIDC Configuration Tests

    #[test]
    fn test_oidc_default_role_serialization() {
        let role = OidcDefaultRole::Admin;
        let yaml = serde_yaml::to_string(&role).unwrap();
        assert!(yaml.contains("admin"));

        let role = OidcDefaultRole::Reader;
        let yaml = serde_yaml::to_string(&role).unwrap();
        assert!(yaml.contains("reader"));
    }

    #[test]
    fn test_oidc_default_role_deserialization() {
        let yaml = "admin";
        let role: OidcDefaultRole = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(role, OidcDefaultRole::Admin));

        let yaml = "maintainer";
        let role: OidcDefaultRole = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(role, OidcDefaultRole::Maintainer));

        let yaml = "reader";
        let role: OidcDefaultRole = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(role, OidcDefaultRole::Reader));
    }

    #[test]
    fn test_oidc_default_role_as_str() {
        assert_eq!(OidcDefaultRole::Admin.as_str(), "admin");
        assert_eq!(OidcDefaultRole::Maintainer.as_str(), "maintainer");
        assert_eq!(OidcDefaultRole::Reader.as_str(), "reader");
    }

    #[test]
    #[serial]
    fn test_oidc_config_default() {
        let config = OidcConfig::default();
        // Disabled by default for security
        assert!(!config.enabled);
        // Auto-create users enabled by default
        assert!(config.auto_create_users);
        // Default role is reader
        assert!(matches!(config.default_role, OidcDefaultRole::Reader));
        // No providers by default
        assert!(config.providers.is_empty());
    }

    #[test]
    fn test_oidc_provider_config_serialization() {
        let mut role_mapping = HashMap::new();
        role_mapping.insert("admin".to_string(), vec!["codex-admins".to_string()]);
        role_mapping.insert(
            "reader".to_string(),
            vec!["codex-users".to_string(), "users".to_string()],
        );

        let provider = OidcProviderConfig {
            display_name: "Authentik".to_string(),
            issuer_url: "https://authentik.example.com/application/o/codex/".to_string(),
            client_id: "codex-client".to_string(),
            client_secret: Some("secret123".to_string()),
            client_secret_env: None,
            scopes: vec!["email".to_string(), "profile".to_string()],
            role_mapping,
            groups_claim: "groups".to_string(),
            username_claim: "preferred_username".to_string(),
            email_claim: "email".to_string(),
        };

        let yaml = serde_yaml::to_string(&provider).unwrap();
        assert!(yaml.contains("Authentik"));
        assert!(yaml.contains("codex-client"));
        assert!(yaml.contains("authentik.example.com"));

        let deserialized: OidcProviderConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.display_name, "Authentik");
        assert_eq!(deserialized.client_id, "codex-client");
        assert_eq!(deserialized.client_secret, Some("secret123".to_string()));
        assert_eq!(deserialized.scopes.len(), 2);
        assert!(deserialized.role_mapping.contains_key("admin"));
    }

    #[test]
    fn test_oidc_provider_config_from_yaml() {
        let yaml_content = r#"
display_name: "Keycloak"
issuer_url: "https://keycloak.example.com/realms/codex"
client_id: "codex"
client_secret_env: "CODEX_OIDC_KEYCLOAK_SECRET"
scopes:
  - email
  - profile
  - groups
role_mapping:
  admin:
    - realm-admin
    - codex-admin
  maintainer:
    - codex-editor
  reader:
    - codex-reader
groups_claim: "groups"
username_claim: "preferred_username"
"#;

        let provider: OidcProviderConfig = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(provider.display_name, "Keycloak");
        assert_eq!(
            provider.issuer_url,
            "https://keycloak.example.com/realms/codex"
        );
        assert_eq!(provider.client_id, "codex");
        assert!(provider.client_secret.is_none());
        assert_eq!(
            provider.client_secret_env,
            Some("CODEX_OIDC_KEYCLOAK_SECRET".to_string())
        );
        assert_eq!(provider.scopes, vec!["email", "profile", "groups"]);
        assert_eq!(
            provider.role_mapping.get("admin"),
            Some(&vec!["realm-admin".to_string(), "codex-admin".to_string()])
        );
        assert_eq!(
            provider.role_mapping.get("maintainer"),
            Some(&vec!["codex-editor".to_string()])
        );
        assert_eq!(provider.groups_claim, "groups");
        assert_eq!(provider.username_claim, "preferred_username");
        // email_claim should use default
        assert_eq!(provider.email_claim, "email");
    }

    #[test]
    fn test_oidc_provider_config_defaults() {
        let yaml_content = r#"
display_name: "Test Provider"
issuer_url: "https://test.example.com"
client_id: "test-client"
"#;

        let provider: OidcProviderConfig = serde_yaml::from_str(yaml_content).unwrap();
        // Check defaults are applied
        assert_eq!(provider.groups_claim, "groups");
        assert_eq!(provider.username_claim, "preferred_username");
        assert_eq!(provider.email_claim, "email");
        assert!(provider.scopes.is_empty());
        assert!(provider.role_mapping.is_empty());
    }

    #[test]
    fn test_oidc_config_serialization() {
        let mut providers = HashMap::new();
        providers.insert(
            "authentik".to_string(),
            OidcProviderConfig {
                display_name: "Authentik".to_string(),
                issuer_url: "https://auth.example.com".to_string(),
                client_id: "client".to_string(),
                client_secret: Some("secret".to_string()),
                client_secret_env: None,
                scopes: vec!["email".to_string()],
                role_mapping: HashMap::new(),
                groups_claim: "groups".to_string(),
                username_claim: "preferred_username".to_string(),
                email_claim: "email".to_string(),
            },
        );

        let config = OidcConfig {
            enabled: true,
            auto_create_users: true,
            default_role: OidcDefaultRole::Reader,
            redirect_uri_base: None,
            providers,
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("enabled: true"));
        assert!(yaml.contains("auto_create_users: true"));
        assert!(yaml.contains("authentik"));

        let deserialized: OidcConfig = serde_yaml::from_str(&yaml).unwrap();
        assert!(deserialized.enabled);
        assert!(deserialized.auto_create_users);
        assert!(deserialized.providers.contains_key("authentik"));
    }

    #[test]
    fn test_oidc_config_from_yaml() {
        let yaml_content = r#"
enabled: true
auto_create_users: false
default_role: maintainer
providers:
  authentik:
    display_name: "Authentik SSO"
    issuer_url: "https://auth.example.com/application/o/codex/"
    client_id: "codex"
    client_secret: "secret123"
    scopes:
      - email
      - profile
      - groups
    role_mapping:
      admin:
        - codex-admins
      reader:
        - codex-users
"#;

        let config: OidcConfig = serde_yaml::from_str(yaml_content).unwrap();
        assert!(config.enabled);
        assert!(!config.auto_create_users);
        assert!(matches!(config.default_role, OidcDefaultRole::Maintainer));
        assert!(config.providers.contains_key("authentik"));

        let provider = config.providers.get("authentik").unwrap();
        assert_eq!(provider.display_name, "Authentik SSO");
        assert_eq!(provider.client_id, "codex");
        assert_eq!(provider.client_secret, Some("secret123".to_string()));
    }

    #[test]
    fn test_auth_config_includes_oidc() {
        let yaml_content = r#"
jwt_secret: "test-secret"
jwt_expiry_hours: 24
oidc:
  enabled: true
  auto_create_users: true
  default_role: reader
"#;

        let config: AuthConfig = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(config.jwt_secret, "test-secret");
        assert!(config.oidc.enabled);
        assert!(config.oidc.auto_create_users);
    }

    #[test]
    #[serial]
    fn test_auth_config_oidc_uses_defaults() {
        // When oidc is not specified, it should use defaults
        let yaml_content = r#"
jwt_secret: "test-secret"
jwt_expiry_hours: 24
"#;

        let config: AuthConfig = serde_yaml::from_str(yaml_content).unwrap();
        assert!(!config.oidc.enabled);
        assert!(config.oidc.auto_create_users);
        assert!(matches!(config.oidc.default_role, OidcDefaultRole::Reader));
    }

    #[test]
    fn test_full_config_with_oidc() {
        let yaml_content = r#"
database:
  db_type: sqlite
  sqlite:
    path: ./test.db
auth:
  jwt_secret: "test-secret"
  oidc:
    enabled: true
    providers:
      keycloak:
        display_name: "Keycloak"
        issuer_url: "https://keycloak.example.com/realms/codex"
        client_id: "codex"
        client_secret: "secret"
"#;

        let config: Config = serde_yaml::from_str(yaml_content).unwrap();
        assert!(config.auth.oidc.enabled);
        assert!(config.auth.oidc.providers.contains_key("keycloak"));
    }

    // ================================================================
    // data_dir and resolve_data_dir tests
    // ================================================================

    #[test]
    fn test_data_dir_default() {
        let config = Config::default();
        assert_eq!(config.data_dir, "data");
    }

    #[test]
    fn test_data_dir_from_yaml() {
        let yaml_content = r#"
data_dir: /var/lib/codex
database:
  db_type: sqlite
  sqlite:
    path: ./test.db
"#;
        let config: Config = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(config.data_dir, "/var/lib/codex");
    }

    #[test]
    fn test_resolve_data_dir_replaces_defaults() {
        let mut config = Config {
            data_dir: "/var/lib/codex".to_string(),
            ..Config::default()
        };
        // Set old defaults
        config.files.thumbnail_dir = "data/thumbnails".to_string();
        config.files.uploads_dir = "data/uploads".to_string();
        config.files.plugins_dir = "data/plugins".to_string();
        config.pdf.cache_dir = "data/cache".to_string();
        config.database.sqlite = Some(SQLiteConfig {
            path: "data/codex.db".to_string(),
            pragmas: None,
            ..SQLiteConfig::default()
        });

        config.resolve_data_dir();

        assert_eq!(config.files.thumbnail_dir, "/var/lib/codex/thumbnails");
        assert_eq!(config.files.uploads_dir, "/var/lib/codex/uploads");
        assert_eq!(config.files.plugins_dir, "/var/lib/codex/plugins");
        assert_eq!(config.pdf.cache_dir, "/var/lib/codex/cache");
        assert_eq!(
            config.database.sqlite.as_ref().unwrap().path,
            "/var/lib/codex/codex.db"
        );
    }

    #[test]
    fn test_resolve_data_dir_preserves_explicit_overrides() {
        let mut config = Config {
            data_dir: "/var/lib/codex".to_string(),
            ..Config::default()
        };
        // Set custom (non-default) paths that should be preserved
        config.files.thumbnail_dir = "/custom/thumbs".to_string();
        config.files.uploads_dir = "/custom/uploads".to_string();
        config.files.plugins_dir = "/custom/plugins".to_string();
        config.pdf.cache_dir = "/custom/cache".to_string();
        config.database.sqlite = Some(SQLiteConfig {
            path: "/custom/db.sqlite".to_string(),
            pragmas: None,
            ..SQLiteConfig::default()
        });

        config.resolve_data_dir();

        // Non-default paths should NOT be replaced
        assert_eq!(config.files.thumbnail_dir, "/custom/thumbs");
        assert_eq!(config.files.uploads_dir, "/custom/uploads");
        assert_eq!(config.files.plugins_dir, "/custom/plugins");
        assert_eq!(config.pdf.cache_dir, "/custom/cache");
        assert_eq!(
            config.database.sqlite.as_ref().unwrap().path,
            "/custom/db.sqlite"
        );
    }

    #[test]
    fn test_resolve_data_dir_noop_with_default_data_dir() {
        let mut config = Config::default();
        // data_dir is "data" by default, so old defaults like "data/thumbnails"
        // should remain "data/thumbnails"
        let original_thumb = config.files.thumbnail_dir.clone();
        let original_uploads = config.files.uploads_dir.clone();
        let original_plugins = config.files.plugins_dir.clone();
        let original_cache = config.pdf.cache_dir.clone();

        config.resolve_data_dir();

        assert_eq!(config.files.thumbnail_dir, original_thumb);
        assert_eq!(config.files.uploads_dir, original_uploads);
        assert_eq!(config.files.plugins_dir, original_plugins);
        assert_eq!(config.pdf.cache_dir, original_cache);
    }

    #[test]
    fn test_resolve_data_dir_no_sqlite_config() {
        let mut config = Config {
            data_dir: "/var/lib/codex".to_string(),
            ..Config::default()
        };
        config.database.sqlite = None;

        // Should not panic when sqlite is None
        config.resolve_data_dir();

        assert_eq!(config.files.thumbnail_dir, "/var/lib/codex/thumbnails");
    }

    #[test]
    fn test_plugins_dir_in_files_config() {
        let yaml_content = r#"
files:
  thumbnail_dir: /tmp/thumbs
  uploads_dir: /tmp/uploads
  plugins_dir: /tmp/plugins
"#;
        let config: Config = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(config.files.plugins_dir, "/tmp/plugins");
    }
}
