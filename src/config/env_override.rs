use super::config::{
    ApiConfig, ApplicationConfig, AuthConfig, Config, DatabaseConfig, DatabaseType, LogLevel,
    LoggingConfig, PostgresConfig, SQLiteConfig, ScannerConfig, TaskConfig,
};
use std::env;

/// Trait for applying environment variable overrides to configuration structs
pub trait EnvOverride {
    /// Apply environment variable overrides with a given prefix
    fn apply_env_overrides(&mut self, prefix: &str);
}

impl EnvOverride for TaskConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        if let Ok(worker_count) = env::var(format!("{}_WORKER_COUNT", prefix)) {
            if let Ok(count) = worker_count.parse::<u32>() {
                self.worker_count = count;
            }
        }
    }
}

impl EnvOverride for ScannerConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        let env_key = format!("{}_MAX_CONCURRENT_SCANS", prefix);
        if let Ok(max_scans) = env::var(&env_key) {
            if let Ok(count) = max_scans.parse::<usize>() {
                self.max_concurrent_scans = count;
            }
        }
    }
}

impl EnvOverride for Config {
    fn apply_env_overrides(&mut self, prefix: &str) {
        self.application
            .apply_env_overrides(&format!("{}_APPLICATION", prefix));
        self.database
            .apply_env_overrides(&format!("{}_DATABASE", prefix));
        self.logging
            .apply_env_overrides(&format!("{}_LOGGING", prefix));
        self.auth.apply_env_overrides(&format!("{}_AUTH", prefix));
        self.api.apply_env_overrides(&format!("{}_API", prefix));
        self.task.apply_env_overrides(&format!("{}_TASK", prefix));
        self.scanner
            .apply_env_overrides(&format!("{}_SCANNER", prefix));
    }
}

impl EnvOverride for ApplicationConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        // Note: application.name moved to database settings
        if let Ok(host) = env::var(format!("{}_HOST", prefix)) {
            self.host = host;
        }
        if let Ok(port) = env::var(format!("{}_PORT", prefix)) {
            if let Ok(port_num) = port.parse() {
                self.port = port_num;
            }
        }
    }
}

impl EnvOverride for DatabaseConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        // Check for database type override (db_type in YAML)
        if let Ok(db_type) = env::var(format!("{}_DB_TYPE", prefix)) {
            if db_type.eq_ignore_ascii_case("postgres")
                || db_type.eq_ignore_ascii_case("postgresql")
            {
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
        if let Ok(level_str) = env::var(format!("{}_LEVEL", prefix)) {
            if let Some(level) = match level_str.to_lowercase().as_str() {
                "error" => Some(LogLevel::Error),
                "warn" => Some(LogLevel::Warn),
                "info" => Some(LogLevel::Info),
                "debug" => Some(LogLevel::Debug),
                "trace" => Some(LogLevel::Trace),
                _ => None,
            } {
                self.level = level;
            }
        }

        if let Ok(console_str) = env::var(format!("{}_CONSOLE", prefix)) {
            if let Ok(console_bool) = console_str.parse() {
                self.console = console_bool;
            }
        }

        if let Ok(log_file) = env::var(format!("{}_FILE", prefix)) {
            self.file = if log_file.is_empty() {
                None
            } else {
                Some(log_file)
            };
        }
    }
}

impl EnvOverride for AuthConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        // Check for JWT secret override - print warning if using insecure default
        if let Ok(jwt_secret) = env::var(format!("{}_JWT_SECRET", prefix)) {
            self.jwt_secret = jwt_secret;
        } else if self.jwt_secret == "INSECURE_DEFAULT_SECRET_CHANGE_IN_PRODUCTION" {
            eprintln!("WARNING: CODEX_AUTH_JWT_SECRET not set, using insecure default for development only!");
        }

        if let Ok(jwt_expiry) = env::var(format!("{}_JWT_EXPIRY_HOURS", prefix)) {
            if let Ok(hours) = jwt_expiry.parse() {
                self.jwt_expiry_hours = hours;
            }
        }
        if let Ok(refresh_enabled) = env::var(format!("{}_REFRESH_TOKEN_ENABLED", prefix)) {
            self.refresh_token_enabled =
                refresh_enabled.eq_ignore_ascii_case("true") || refresh_enabled == "1";
        }
        if let Ok(refresh_expiry) = env::var(format!("{}_REFRESH_TOKEN_EXPIRY_DAYS", prefix)) {
            if let Ok(days) = refresh_expiry.parse() {
                self.refresh_token_expiry_days = days;
            }
        }
        if let Ok(memory_cost) = env::var(format!("{}_ARGON2_MEMORY_COST", prefix)) {
            if let Ok(cost) = memory_cost.parse() {
                self.argon2_memory_cost = cost;
            }
        }
        if let Ok(time_cost) = env::var(format!("{}_ARGON2_TIME_COST", prefix)) {
            if let Ok(cost) = time_cost.parse() {
                self.argon2_time_cost = cost;
            }
        }
        if let Ok(parallelism) = env::var(format!("{}_ARGON2_PARALLELISM", prefix)) {
            if let Ok(p) = parallelism.parse() {
                self.argon2_parallelism = p;
            }
        }
    }
}

impl EnvOverride for ApiConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        if let Ok(base_path) = env::var(format!("{}_BASE_PATH", prefix)) {
            self.base_path = base_path;
        }
        if let Ok(enable_swagger) = env::var(format!("{}_ENABLE_SWAGGER", prefix)) {
            self.enable_swagger =
                enable_swagger.eq_ignore_ascii_case("true") || enable_swagger == "1";
        }
        if let Ok(swagger_path) = env::var(format!("{}_SWAGGER_PATH", prefix)) {
            self.swagger_path = swagger_path;
        }
        if let Ok(cors_enabled) = env::var(format!("{}_CORS_ENABLED", prefix)) {
            self.cors_enabled = cors_enabled.eq_ignore_ascii_case("true") || cors_enabled == "1";
        }
        if let Ok(cors_origins) = env::var(format!("{}_CORS_ORIGINS", prefix)) {
            self.cors_origins = cors_origins
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }
        if let Ok(max_page_size) = env::var(format!("{}_MAX_PAGE_SIZE", prefix)) {
            if let Ok(size) = max_page_size.parse() {
                self.max_page_size = size;
            }
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
    use serial_test::serial;
    use std::env;

    #[test]
    #[serial]
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
    #[serial]
    fn test_env_or() {
        env::set_var("TEST_PORT", "9090");
        assert_eq!(env_or("TEST_PORT", 8080u16), 9090);
        assert_eq!(env_or("NONEXISTENT", 8080u16), 8080);
        env::remove_var("TEST_PORT");
    }

    #[test]
    #[serial]
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
    #[serial]
    fn test_application_config_override() {
        env::set_var("CODEX_APPLICATION_HOST", "0.0.0.0");
        env::set_var("CODEX_APPLICATION_PORT", "9090");

        let mut config = ApplicationConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
        };

        config.apply_env_overrides("CODEX_APPLICATION");

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 9090);

        env::remove_var("CODEX_APPLICATION_HOST");
        env::remove_var("CODEX_APPLICATION_PORT");
    }

    #[test]
    #[serial]
    fn test_task_config_env_override() {
        // Clear any existing env vars first to avoid interference from other tests
        env::remove_var("CODEX_TASK_WORKER_COUNT");

        env::set_var("CODEX_TASK_WORKER_COUNT", "8");

        let mut config = TaskConfig::default();
        config.apply_env_overrides("CODEX_TASK");

        assert_eq!(config.worker_count, 8);

        env::remove_var("CODEX_TASK_WORKER_COUNT");
    }

    #[test]
    #[serial]
    fn test_task_config_env_override_invalid() {
        // Clear any existing env vars first to avoid interference from other tests
        env::remove_var("CODEX_TASK_WORKER_COUNT");

        env::set_var("CODEX_TASK_WORKER_COUNT", "invalid");

        let mut config = TaskConfig { worker_count: 4 };
        config.apply_env_overrides("CODEX_TASK");

        // Should keep original value if env var is invalid
        assert_eq!(config.worker_count, 4);

        env::remove_var("CODEX_TASK_WORKER_COUNT");
    }

    #[test]
    #[serial]
    fn test_scanner_config_env_override() {
        // Clear any existing env vars first
        env::remove_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS");

        // Create config with explicit values (not using default which reads env vars)
        let mut config = ScannerConfig {
            max_concurrent_scans: 2,
        };

        // Set env vars and apply overrides
        env::set_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS", "6");
        config.apply_env_overrides("CODEX_SCANNER");

        assert_eq!(config.max_concurrent_scans, 6);

        env::remove_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS");
    }

    #[test]
    #[serial]
    fn test_scanner_config_env_override_partial() {
        // Clear any existing env vars first
        env::remove_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS");

        // Create config with explicit values (not using default which reads env vars)
        let mut config = ScannerConfig {
            max_concurrent_scans: 2,
        };

        env::set_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS", "10");
        config.apply_env_overrides("CODEX_SCANNER");

        assert_eq!(config.max_concurrent_scans, 10);

        env::remove_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS");
    }

    #[test]
    #[serial]
    fn test_config_env_override_task_and_scanner() {
        // Clear any existing env vars first
        env::remove_var("CODEX_TASK_WORKER_COUNT");
        env::remove_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS");

        // Create config with explicit values to avoid reading env vars in default()
        // We'll use a helper to create a minimal config
        use crate::config::{
            ApiConfig, ApplicationConfig, AuthConfig, DatabaseConfig, DatabaseType, EmailConfig,
            LoggingConfig, SQLiteConfig, ThumbnailConfig,
        };
        let mut config = Config {
            database: DatabaseConfig {
                db_type: DatabaseType::SQLite,
                postgres: None,
                sqlite: Some(SQLiteConfig {
                    path: "./test.db".to_string(),
                    pragmas: None,
                }),
            },
            application: ApplicationConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
            },
            logging: LoggingConfig::default(),
            auth: AuthConfig::default(),
            api: ApiConfig::default(),
            email: EmailConfig::default(),
            task: TaskConfig { worker_count: 4 },
            scanner: ScannerConfig {
                max_concurrent_scans: 2,
            },
            thumbnail: ThumbnailConfig::default(),
        };

        // Set env vars BEFORE applying overrides
        env::set_var("CODEX_TASK_WORKER_COUNT", "12");
        env::set_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS", "5");

        // Verify env vars are set before applying (capture values for debugging)
        let task_var_before = env::var("CODEX_TASK_WORKER_COUNT").ok();
        let scanner_max_var_before = env::var("CODEX_SCANNER_MAX_CONCURRENT_SCANS").ok();

        // Double-check env vars are still set right before applying (to catch race conditions)
        // This ensures we capture the value at the exact moment we need it
        let scanner_max_at_call = env::var("CODEX_SCANNER_MAX_CONCURRENT_SCANS").ok();

        // If env vars are not set at this point, it's a race condition with another test
        assert!(
            scanner_max_at_call.is_some(),
            "Environment variable CODEX_SCANNER_MAX_CONCURRENT_SCANS was cleared by another test (race condition). Value before: {:?}, value at call: {:?}",
            scanner_max_var_before, scanner_max_at_call
        );

        // Apply overrides - Config::apply_env_overrides("CODEX") will call:
        // - task.apply_env_overrides("CODEX_TASK") -> looks for CODEX_TASK_WORKER_COUNT
        // - scanner.apply_env_overrides("CODEX_SCANNER") -> looks for CODEX_SCANNER_MAX_CONCURRENT_SCANS
        // Store value before applying to verify it changes
        let scanner_value_before = config.scanner.max_concurrent_scans;
        config.apply_env_overrides("CODEX");
        let scanner_value_after = config.scanner.max_concurrent_scans;

        // Debug: Check env var after applying (to catch race conditions)
        let scanner_max_var_after = env::var("CODEX_SCANNER_MAX_CONCURRENT_SCANS").ok();

        // Verify the overrides were applied
        assert_eq!(
            config.task.worker_count, 12,
            "Task worker count should be overridden to 12 (env var before: {:?})",
            task_var_before
        );
        // Debug: Check what the scanner config looks like after applying
        let env_key_used = format!("CODEX_SCANNER_MAX_CONCURRENT_SCANS");
        let env_value_when_checked = env::var(&env_key_used).ok();

        assert_eq!(
            scanner_value_after, 5,
            "Scanner max_concurrent_scans should be overridden to 5 (got: {}, was: {}, env var before: {:?}, env var at call: {:?}, env var after: {:?}, env key used: {:?}, env value when checked: {:?})",
            scanner_value_after, scanner_value_before, scanner_max_var_before, scanner_max_at_call, scanner_max_var_after, env_key_used, env_value_when_checked
        );

        env::remove_var("CODEX_TASK_WORKER_COUNT");
        env::remove_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS");
    }
}
