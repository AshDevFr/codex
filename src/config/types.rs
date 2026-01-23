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
    pub files: FilesConfig,
    #[serde(default)]
    pub pdf: PdfConfig,
    #[serde(default)]
    pub komga_api: KomgaApiConfig,
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
            database: DatabaseConfig {
                db_type,
                postgres: postgres_config,
                sqlite: sqlite_config,
            },
            application: ApplicationConfig {
                host: env_string_opt("CODEX_APPLICATION_HOST")
                    .unwrap_or_else(|| "0.0.0.0".to_string()),
                port: env_or("CODEX_APPLICATION_PORT", 8080),
            },
            logging: LoggingConfig::default(),
            auth: AuthConfig::default(),
            api: ApiConfig::default(),
            email: EmailConfig::default(),
            task: TaskConfig::default(),
            scanner: ScannerConfig::default(),
            files: FilesConfig::default(),
            pdf: PdfConfig::default(),
            komga_api: KomgaApiConfig::default(),
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
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        Self {
            host: env_string_opt("CODEX_APPLICATION_HOST").unwrap_or_else(|| "0.0.0.0".to_string()),
            port: env_or("CODEX_APPLICATION_PORT", 8080),
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct FilesConfig {
    /// Full path to thumbnail cache directory
    /// This is a startup-time setting - changes require a restart
    pub thumbnail_dir: String,

    /// Full path to uploads directory for user-uploaded files (covers, etc.)
    /// This is a startup-time setting - changes require a restart
    pub uploads_dir: String,
}

impl Default for FilesConfig {
    fn default() -> Self {
        Self {
            thumbnail_dir: env_string_opt("CODEX_FILES_THUMBNAIL_DIR")
                .unwrap_or_else(|| "data/thumbnails".to_string()),
            uploads_dir: env_string_opt("CODEX_FILES_UPLOADS_DIR")
                .unwrap_or_else(|| "data/uploads".to_string()),
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
    pub verification_url_base: String,
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
            verification_url_base: env_string_opt("CODEX_EMAIL_VERIFICATION_URL_BASE")
                .unwrap_or_else(|| "http://localhost:8080".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        };

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
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
            },
            logging: LoggingConfig::default(),
            auth: AuthConfig::default(),
            api: ApiConfig::default(),
            email: EmailConfig::default(),
            task: TaskConfig::default(),
            scanner: ScannerConfig::default(),
            files: FilesConfig::default(),
            pdf: PdfConfig::default(),
            komga_api: KomgaApiConfig::default(),
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
}
