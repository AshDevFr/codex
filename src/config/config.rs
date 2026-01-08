use super::env_override::{env_bool_or, env_or, env_string_opt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub scanner: ScannerConfig,
    #[serde(default)]
    pub email: EmailConfig,
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
            DatabaseType::Postgres => (
                Some(PostgresConfig {
                    host: env_string_opt("CODEX_DATABASE_POSTGRES_HOST")
                        .unwrap_or_else(|| "localhost".to_string()),
                    port: env_or("CODEX_DATABASE_POSTGRES_PORT", 5432),
                    username: env_string_opt("CODEX_DATABASE_POSTGRES_USERNAME")
                        .unwrap_or_else(|| "codex".to_string()),
                    password: env_string_opt("CODEX_DATABASE_POSTGRES_PASSWORD")
                        .unwrap_or_else(|| "codex".to_string()),
                    database_name: env_string_opt("CODEX_DATABASE_POSTGRES_DATABASE_NAME")
                        .unwrap_or_else(|| "codex".to_string()),
                }),
                None,
            ),
            DatabaseType::SQLite => (
                None,
                Some(SQLiteConfig {
                    path: env_string_opt("CODEX_DATABASE_SQLITE_PATH")
                        .unwrap_or_else(|| "codex.db".to_string()),
                    pragmas: Some(pragmas),
                }),
            ),
        };

        // Build logging level from environment
        let log_level = env::var("CODEX_LOGGING_LEVEL")
            .ok()
            .and_then(|l| match l.to_lowercase().as_str() {
                "error" => Some(LogLevel::Error),
                "warn" => Some(LogLevel::Warn),
                "info" => Some(LogLevel::Info),
                "debug" => Some(LogLevel::Debug),
                "trace" => Some(LogLevel::Trace),
                _ => None,
            })
            .unwrap_or(LogLevel::Info);

        Self {
            database: DatabaseConfig {
                db_type,
                postgres: postgres_config,
                sqlite: sqlite_config,
            },
            application: ApplicationConfig {
                name: env_string_opt("CODEX_APPLICATION_NAME")
                    .unwrap_or_else(|| "Codex".to_string()),
                host: env_string_opt("CODEX_APPLICATION_HOST")
                    .unwrap_or_else(|| "127.0.0.1".to_string()),
                port: env_or("CODEX_APPLICATION_PORT", 8080),
            },
            logging: LoggingConfig {
                level: log_level,
                file: env_string_opt("CODEX_LOGGING_FILE"),
                console: env_bool_or("CODEX_LOGGING_CONSOLE", true),
            },
            auth: AuthConfig::default(),
            api: ApiConfig::default(),
            scanner: ScannerConfig::default(),
            email: EmailConfig::default(),
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
    pub enable_swagger: bool,
    pub swagger_path: String,
    pub cors_enabled: bool,
    pub cors_origins: Vec<String>,
    pub max_page_size: usize,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            base_path: env_string_opt("CODEX_API_BASE_PATH")
                .unwrap_or_else(|| "/api/v1".to_string()),
            enable_swagger: env_bool_or("CODEX_API_ENABLE_SWAGGER", false),
            swagger_path: env_string_opt("CODEX_API_SWAGGER_PATH")
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    Postgres,
    SQLite,
}

impl Default for DatabaseType {
    fn default() -> Self {
        DatabaseType::SQLite
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database_name: String,
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
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct SQLiteConfig {
    pub path: String,
    pub pragmas: Option<HashMap<String, String>>,
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
                .unwrap_or_else(|| "codex.db".to_string()),
            pragmas: Some(pragmas),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct ApplicationConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        Self {
            name: env_string_opt("CODEX_APPLICATION_NAME").unwrap_or_else(|| "Codex".to_string()),
            host: env_string_opt("CODEX_APPLICATION_HOST")
                .unwrap_or_else(|| "127.0.0.1".to_string()),
            port: env_or("CODEX_APPLICATION_PORT", 8080),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: LogLevel,
    pub file: Option<String>,
    pub console: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            file: None,
            console: true,
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
    pub max_concurrent_scans: usize,
    pub scan_timeout_minutes: u64,
    pub retry_failed_files: bool,
    /// Number of concurrent analysis tasks to run after scan (0 = disabled)
    pub auto_analyze_concurrency: usize,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_scans: env_or("CODEX_SCANNER_MAX_CONCURRENT_SCANS", 2),
            scan_timeout_minutes: env_or("CODEX_SCANNER_SCAN_TIMEOUT_MINUTES", 120),
            retry_failed_files: env_bool_or("CODEX_SCANNER_RETRY_FAILED_FILES", false),
            auto_analyze_concurrency: env_or("CODEX_SCANNER_AUTO_ANALYZE_CONCURRENCY", 4),
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

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            smtp_host: env_string_opt("CODEX_EMAIL_SMTP_HOST")
                .unwrap_or_else(|| "localhost".to_string()),
            smtp_port: env_or("CODEX_EMAIL_SMTP_PORT", 587),
            smtp_username: env_string_opt("CODEX_EMAIL_SMTP_USERNAME")
                .unwrap_or_else(|| "".to_string()),
            smtp_password: env_string_opt("CODEX_EMAIL_SMTP_PASSWORD")
                .unwrap_or_else(|| "".to_string()),
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
        };

        assert!(config.pragmas.is_some());
        assert_eq!(config.pragmas.unwrap().get("journal_mode").unwrap(), "WAL");
    }

    #[test]
    fn test_application_config() {
        let config = ApplicationConfig {
            name: "Codex".to_string(),
            host: "0.0.0.0".to_string(),
            port: 8080,
        };

        assert_eq!(config.name, "Codex");
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
            }),
        };

        assert!(matches!(config.db_type, DatabaseType::SQLite));
        assert!(config.postgres.is_none());
        assert!(config.sqlite.is_some());
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
                }),
            },
            application: ApplicationConfig {
                name: "Codex".to_string(),
                host: "127.0.0.1".to_string(),
                port: 3000,
            },
            logging: LoggingConfig::default(),
            auth: AuthConfig::default(),
            api: ApiConfig::default(),
            scanner: ScannerConfig::default(),
            email: EmailConfig::default(),
        };

        assert_eq!(config.application.name, "Codex");
        assert_eq!(config.application.port, 3000);
        assert!(matches!(config.database.db_type, DatabaseType::SQLite));
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
        assert!(!config.enable_swagger); // Disabled by default
        assert_eq!(config.swagger_path, "/docs");
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
            enable_swagger: true,
            swagger_path: "/api-docs".to_string(),
            cors_enabled: false,
            cors_origins: vec!["https://example.com".to_string()],
            max_page_size: 200,
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("/api/v2"));
        assert!(yaml.contains("true")); // enable_swagger

        let deserialized: ApiConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.base_path, "/api/v2");
        assert!(deserialized.enable_swagger);
        assert_eq!(deserialized.max_page_size, 200);
    }
}
