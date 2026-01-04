use std::env;
use super::{Config, DatabaseConfig, DatabaseType, PostgresConfig, SQLiteConfig, ApplicationConfig, LoggingConfig, LogLevel};

/// Trait for applying environment variable overrides to configuration structs
pub trait EnvOverride {
    /// Apply environment variable overrides with a given prefix
    fn apply_env_overrides(&mut self, prefix: &str);
}

impl EnvOverride for Config {
    fn apply_env_overrides(&mut self, prefix: &str) {
        self.application.apply_env_overrides(&format!("{}_APPLICATION", prefix));
        self.database.apply_env_overrides(&format!("{}_DATABASE", prefix));
        self.logging.apply_env_overrides(&format!("{}_LOGGING", prefix));
    }
}

impl EnvOverride for ApplicationConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        if let Ok(name) = env::var(format!("{}_NAME", prefix)) {
            self.name = name;
        }
        if let Ok(host) = env::var(format!("{}_HOST", prefix)) {
            self.host = host;
        }
        if let Ok(port) = env::var(format!("{}_PORT", prefix)) {
            if let Ok(port_num) = port.parse() {
                self.port = port_num;
            }
        }
        if let Ok(debug) = env::var(format!("{}_DEBUG", prefix)) {
            self.debug = debug.eq_ignore_ascii_case("true") || debug == "1";
        }
    }
}

impl EnvOverride for DatabaseConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        // Check for database type override (db_type in YAML)
        if let Ok(db_type) = env::var(format!("{}_DB_TYPE", prefix)) {
            if db_type.eq_ignore_ascii_case("postgres") || db_type.eq_ignore_ascii_case("postgresql") {
                self.db_type = DatabaseType::Postgres;
            } else if db_type.eq_ignore_ascii_case("sqlite") {
                self.db_type = DatabaseType::SQLite;
            }
        }

        // Apply PostgreSQL overrides if config exists
        if let Some(ref mut pg_config) = self.postgres {
            pg_config.apply_env_overrides(&format!("{}_POSTGRES", prefix));
        }

        // Apply SQLite overrides if config exists
        if let Some(ref mut sqlite_config) = self.sqlite {
            sqlite_config.apply_env_overrides(&format!("{}_SQLITE", prefix));
        }
    }
}

impl EnvOverride for PostgresConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        if let Ok(host) = env::var(format!("{}_HOST", prefix)) {
            self.host = host;
        }
        if let Ok(port) = env::var(format!("{}_PORT", prefix)) {
            if let Ok(port_num) = port.parse() {
                self.port = port_num;
            }
        }
        if let Ok(username) = env::var(format!("{}_USERNAME", prefix)) {
            self.username = username;
        }
        if let Ok(password) = env::var(format!("{}_PASSWORD", prefix)) {
            self.password = password;
        }
        if let Ok(database_name) = env::var(format!("{}_DATABASE_NAME", prefix)) {
            self.database_name = database_name;
        }
    }
}

impl EnvOverride for SQLiteConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        if let Ok(path) = env::var(format!("{}_PATH", prefix)) {
            self.path = path;
        }
        // Note: Pragmas are typically not overridden via env vars due to their complex nature
    }
}

impl EnvOverride for LoggingConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        if let Ok(log_level) = env::var(format!("{}_LEVEL", prefix)) {
            self.level = match log_level.to_lowercase().as_str() {
                "error" => LogLevel::Error,
                "warn" => LogLevel::Warn,
                "info" => LogLevel::Info,
                "debug" => LogLevel::Debug,
                "trace" => LogLevel::Trace,
                _ => self.level.clone(),
            };
        }
        if let Ok(log_file) = env::var(format!("{}_FILE", prefix)) {
            self.file = if log_file.is_empty() {
                None
            } else {
                Some(log_file)
            };
        }
        if let Ok(log_console) = env::var(format!("{}_CONSOLE", prefix)) {
            self.console = log_console.eq_ignore_ascii_case("true") || log_console == "1";
        }
    }
}

/// Helper function to get environment variable with fallback
pub fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Helper function to get boolean environment variable with fallback
pub fn env_bool_or(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(default)
}

/// Helper function to get optional string environment variable
pub fn env_string_opt(key: &str) -> Option<String> {
    env::var(key).ok().filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_env_bool_or() {
        env::set_var("TEST_BOOL_TRUE", "true");
        env::set_var("TEST_BOOL_1", "1");
        env::set_var("TEST_BOOL_FALSE", "false");

        assert!(env_bool_or("TEST_BOOL_TRUE", false));
        assert!(env_bool_or("TEST_BOOL_1", false));
        assert!(!env_bool_or("TEST_BOOL_FALSE", false));
        assert!(env_bool_or("NONEXISTENT", true));

        env::remove_var("TEST_BOOL_TRUE");
        env::remove_var("TEST_BOOL_1");
        env::remove_var("TEST_BOOL_FALSE");
    }

    #[test]
    fn test_env_or() {
        env::set_var("TEST_PORT", "9090");
        assert_eq!(env_or("TEST_PORT", 8080u16), 9090);
        assert_eq!(env_or("NONEXISTENT", 8080u16), 8080);
        env::remove_var("TEST_PORT");
    }

    #[test]
    fn test_env_string_opt() {
        env::set_var("TEST_STRING", "value");
        env::set_var("TEST_EMPTY", "");

        assert_eq!(env_string_opt("TEST_STRING"), Some("value".to_string()));
        assert_eq!(env_string_opt("TEST_EMPTY"), None);
        assert_eq!(env_string_opt("NONEXISTENT"), None);

        env::remove_var("TEST_STRING");
        env::remove_var("TEST_EMPTY");
    }

    #[test]
    fn test_application_config_override() {
        env::set_var("CODEX_APPLICATION_HOST", "0.0.0.0");
        env::set_var("CODEX_APPLICATION_PORT", "9090");
        env::set_var("CODEX_APPLICATION_DEBUG", "true");

        let mut config = ApplicationConfig {
            name: "Codex".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8080,
            debug: false,
        };

        config.apply_env_overrides("CODEX_APPLICATION");

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 9090);
        assert!(config.debug);

        env::remove_var("CODEX_APPLICATION_HOST");
        env::remove_var("CODEX_APPLICATION_PORT");
        env::remove_var("CODEX_APPLICATION_DEBUG");
    }
}
