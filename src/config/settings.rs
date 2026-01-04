use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::env_override::{env_or, env_bool_or, env_string_opt};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub database: DatabaseConfig,
    pub application: ApplicationConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
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
                    host: env_string_opt("CODEX_DATABASE_POSTGRES_HOST").unwrap_or_else(|| "localhost".to_string()),
                    port: env_or("CODEX_DATABASE_POSTGRES_PORT", 5432),
                    username: env_string_opt("CODEX_DATABASE_POSTGRES_USERNAME").unwrap_or_else(|| "codex".to_string()),
                    password: env_string_opt("CODEX_DATABASE_POSTGRES_PASSWORD").unwrap_or_else(|| "codex".to_string()),
                    database_name: env_string_opt("CODEX_DATABASE_POSTGRES_DATABASE_NAME").unwrap_or_else(|| "codex".to_string()),
                }),
                None,
            ),
            DatabaseType::SQLite => (
                None,
                Some(SQLiteConfig {
                    path: env_string_opt("CODEX_DATABASE_SQLITE_PATH").unwrap_or_else(|| "codex.db".to_string()),
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
                name: env_string_opt("CODEX_APPLICATION_NAME").unwrap_or_else(|| "Codex".to_string()),
                host: env_string_opt("CODEX_APPLICATION_HOST").unwrap_or_else(|| "127.0.0.1".to_string()),
                port: env_or("CODEX_APPLICATION_PORT", 8080),
                debug: env_bool_or("CODEX_APPLICATION_DEBUG", false),
            },
            logging: LoggingConfig {
                level: log_level,
                file: env_string_opt("CODEX_LOGGING_FILE"),
                console: env_bool_or("CODEX_LOGGING_CONSOLE", true),
            },
        }
    }
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub db_type: DatabaseType,

    // Postgres Specific
    pub postgres: Option<PostgresConfig>,

    // SQLite Specific
    pub sqlite: Option<SQLiteConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    Postgres,
    SQLite,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SQLiteConfig {
    pub path: String,
    pub pragmas: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApplicationConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub debug: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: LogLevel,
    pub file: Option<String>,
    #[serde(default = "default_console")]
    pub console: bool,
}

fn default_log_level() -> LogLevel {
    LogLevel::Info
}

fn default_console() -> bool {
    true
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
            debug: true,
        };

        assert_eq!(config.name, "Codex");
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert!(config.debug);
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
                debug: false,
            },
            logging: LoggingConfig::default(),
        };

        assert_eq!(config.application.name, "Codex");
        assert_eq!(config.application.port, 3000);
        assert!(matches!(config.database.db_type, DatabaseType::SQLite));
    }
}
