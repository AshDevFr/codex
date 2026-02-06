use super::types::{
    ApiConfig, ApplicationConfig, AuthConfig, Config, DatabaseConfig, DatabaseType, KomgaApiConfig,
    LogLevel, LoggingConfig, OidcConfig, OidcDefaultRole, OidcProviderConfig, PostgresConfig,
    RateLimitConfig, SQLiteConfig, ScannerConfig, TaskConfig,
};
use std::collections::HashMap;
use std::env;

/// Trait for applying environment variable overrides to configuration structs
pub trait EnvOverride {
    /// Apply environment variable overrides with a given prefix
    fn apply_env_overrides(&mut self, prefix: &str);
}

impl EnvOverride for TaskConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        if let Ok(worker_count) = env::var(format!("{}_WORKER_COUNT", prefix))
            && let Ok(count) = worker_count.parse::<u32>()
        {
            self.worker_count = count;
        }
    }
}

impl EnvOverride for ScannerConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        let env_key = format!("{}_MAX_CONCURRENT_SCANS", prefix);
        if let Ok(max_scans) = env::var(&env_key)
            && let Ok(count) = max_scans.parse::<usize>()
        {
            self.max_concurrent_scans = count;
        }
    }
}

impl EnvOverride for KomgaApiConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        if let Ok(enabled) = env::var(format!("{}_ENABLED", prefix)) {
            self.enabled = enabled.eq_ignore_ascii_case("true") || enabled == "1";
        }
        if let Ok(prefix_value) = env::var(format!("{}_PREFIX", prefix))
            && !prefix_value.is_empty()
        {
            self.prefix = prefix_value;
        }
    }
}

impl EnvOverride for RateLimitConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        if let Ok(enabled) = env::var(format!("{}_ENABLED", prefix)) {
            self.enabled = enabled.eq_ignore_ascii_case("true") || enabled == "1";
        }
        if let Ok(anonymous_rps) = env::var(format!("{}_ANONYMOUS_RPS", prefix))
            && let Ok(rps) = anonymous_rps.parse::<u32>()
        {
            self.anonymous_rps = rps;
        }
        if let Ok(anonymous_burst) = env::var(format!("{}_ANONYMOUS_BURST", prefix))
            && let Ok(burst) = anonymous_burst.parse::<u32>()
        {
            self.anonymous_burst = burst;
        }
        if let Ok(authenticated_rps) = env::var(format!("{}_AUTHENTICATED_RPS", prefix))
            && let Ok(rps) = authenticated_rps.parse::<u32>()
        {
            self.authenticated_rps = rps;
        }
        if let Ok(authenticated_burst) = env::var(format!("{}_AUTHENTICATED_BURST", prefix))
            && let Ok(burst) = authenticated_burst.parse::<u32>()
        {
            self.authenticated_burst = burst;
        }
        if let Ok(exempt_paths) = env::var(format!("{}_EXEMPT_PATHS", prefix)) {
            self.exempt_paths = exempt_paths
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if let Ok(cleanup_interval) = env::var(format!("{}_CLEANUP_INTERVAL_SECS", prefix))
            && let Ok(secs) = cleanup_interval.parse::<u64>()
        {
            self.cleanup_interval_secs = secs;
        }
        if let Ok(bucket_ttl) = env::var(format!("{}_BUCKET_TTL_SECS", prefix))
            && let Ok(secs) = bucket_ttl.parse::<u64>()
        {
            self.bucket_ttl_secs = secs;
        }
    }
}

impl EnvOverride for OidcProviderConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        if let Ok(display_name) = env::var(format!("{}_DISPLAY_NAME", prefix)) {
            self.display_name = display_name;
        }
        if let Ok(issuer_url) = env::var(format!("{}_ISSUER_URL", prefix)) {
            self.issuer_url = issuer_url;
        }
        if let Ok(client_id) = env::var(format!("{}_CLIENT_ID", prefix)) {
            self.client_id = client_id;
        }
        if let Ok(client_secret) = env::var(format!("{}_CLIENT_SECRET", prefix)) {
            self.client_secret = Some(client_secret);
        }
        if let Ok(client_secret_env) = env::var(format!("{}_CLIENT_SECRET_ENV", prefix)) {
            self.client_secret_env = Some(client_secret_env);
        }
        if let Ok(scopes) = env::var(format!("{}_SCOPES", prefix)) {
            self.scopes = scopes
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if let Ok(groups_claim) = env::var(format!("{}_GROUPS_CLAIM", prefix)) {
            self.groups_claim = groups_claim;
        }
        if let Ok(username_claim) = env::var(format!("{}_USERNAME_CLAIM", prefix)) {
            self.username_claim = username_claim;
        }
        if let Ok(email_claim) = env::var(format!("{}_EMAIL_CLAIM", prefix)) {
            self.email_claim = email_claim;
        }
    }
}

impl EnvOverride for OidcConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        if let Ok(enabled) = env::var(format!("{}_ENABLED", prefix)) {
            self.enabled = enabled.eq_ignore_ascii_case("true") || enabled == "1";
        }
        if let Ok(auto_create) = env::var(format!("{}_AUTO_CREATE_USERS", prefix)) {
            self.auto_create_users = auto_create.eq_ignore_ascii_case("true") || auto_create == "1";
        }
        if let Ok(default_role) = env::var(format!("{}_DEFAULT_ROLE", prefix)) {
            self.default_role = match default_role.to_lowercase().as_str() {
                "admin" => OidcDefaultRole::Admin,
                "maintainer" => OidcDefaultRole::Maintainer,
                _ => OidcDefaultRole::Reader,
            };
        }

        // Apply overrides to existing providers
        for (provider_name, provider_config) in self.providers.iter_mut() {
            let provider_prefix = format!(
                "{}_PROVIDERS_{}",
                prefix,
                provider_name.to_uppercase().replace('-', "_")
            );
            provider_config.apply_env_overrides(&provider_prefix);
        }

        // Check for dynamically configured providers via environment variables
        // Format: CODEX_AUTH_OIDC_PROVIDERS_<NAME>_ISSUER_URL (required to detect a new provider)
        // This allows adding providers purely through environment variables
        for (key, _) in env::vars() {
            let providers_prefix = format!("{}_PROVIDERS_", prefix);
            if key.starts_with(&providers_prefix) && key.ends_with("_ISSUER_URL") {
                // Extract provider name from key
                let provider_name_upper = key
                    .strip_prefix(&providers_prefix)
                    .and_then(|s| s.strip_suffix("_ISSUER_URL"))
                    .unwrap_or("");
                if provider_name_upper.is_empty() {
                    continue;
                }

                let provider_name = provider_name_upper.to_lowercase().replace('_', "-");
                let provider_prefix = format!("{}_PROVIDERS_{}", prefix, provider_name_upper);

                // Only create new provider if it doesn't already exist
                self.providers
                    .entry(provider_name.clone())
                    .or_insert_with(|| {
                        // Create provider with defaults and then apply env overrides
                        let mut new_provider = OidcProviderConfig {
                            display_name: provider_name,
                            issuer_url: String::new(),
                            client_id: String::new(),
                            client_secret: None,
                            client_secret_env: None,
                            scopes: vec!["email".to_string(), "profile".to_string()],
                            role_mapping: HashMap::new(),
                            groups_claim: "groups".to_string(),
                            username_claim: "preferred_username".to_string(),
                            email_claim: "email".to_string(),
                        };
                        new_provider.apply_env_overrides(&provider_prefix);
                        new_provider
                    });
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
        self.komga_api
            .apply_env_overrides(&format!("{}_KOMGA_API", prefix));
        self.rate_limit
            .apply_env_overrides(&format!("{}_RATE_LIMIT", prefix));
    }
}

impl EnvOverride for ApplicationConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        // Note: application.name moved to database settings
        if let Ok(host) = env::var(format!("{}_HOST", prefix)) {
            self.host = host;
        }
        if let Ok(port) = env::var(format!("{}_PORT", prefix))
            && let Ok(port_num) = port.parse()
        {
            self.port = port_num;
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
        if let Ok(port) = env::var(format!("{}_PORT", prefix))
            && let Ok(port_num) = port.parse()
        {
            self.port = port_num;
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
        if let Ok(level_str) = env::var(format!("{}_LEVEL", prefix))
            && let Some(level) = match level_str.to_lowercase().as_str() {
                "error" => Some(LogLevel::Error),
                "warn" => Some(LogLevel::Warn),
                "info" => Some(LogLevel::Info),
                "debug" => Some(LogLevel::Debug),
                "trace" => Some(LogLevel::Trace),
                _ => None,
            }
        {
            self.level = level;
        }

        if let Ok(console_str) = env::var(format!("{}_CONSOLE", prefix))
            && let Ok(console_bool) = console_str.parse()
        {
            self.console = console_bool;
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
            eprintln!(
                "WARNING: CODEX_AUTH_JWT_SECRET not set, using insecure default for development only!"
            );
        }

        if let Ok(jwt_expiry) = env::var(format!("{}_JWT_EXPIRY_HOURS", prefix))
            && let Ok(hours) = jwt_expiry.parse()
        {
            self.jwt_expiry_hours = hours;
        }
        if let Ok(refresh_enabled) = env::var(format!("{}_REFRESH_TOKEN_ENABLED", prefix)) {
            self.refresh_token_enabled =
                refresh_enabled.eq_ignore_ascii_case("true") || refresh_enabled == "1";
        }
        if let Ok(refresh_expiry) = env::var(format!("{}_REFRESH_TOKEN_EXPIRY_DAYS", prefix))
            && let Ok(days) = refresh_expiry.parse()
        {
            self.refresh_token_expiry_days = days;
        }
        if let Ok(memory_cost) = env::var(format!("{}_ARGON2_MEMORY_COST", prefix))
            && let Ok(cost) = memory_cost.parse()
        {
            self.argon2_memory_cost = cost;
        }
        if let Ok(time_cost) = env::var(format!("{}_ARGON2_TIME_COST", prefix))
            && let Ok(cost) = time_cost.parse()
        {
            self.argon2_time_cost = cost;
        }
        if let Ok(parallelism) = env::var(format!("{}_ARGON2_PARALLELISM", prefix))
            && let Ok(p) = parallelism.parse()
        {
            self.argon2_parallelism = p;
        }

        // Apply OIDC configuration overrides
        self.oidc.apply_env_overrides(&format!("{}_OIDC", prefix));
    }
}

impl EnvOverride for ApiConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        if let Ok(base_path) = env::var(format!("{}_BASE_PATH", prefix)) {
            self.base_path = base_path;
        }
        if let Ok(enable_api_docs) = env::var(format!("{}_ENABLE_API_DOCS", prefix)) {
            self.enable_api_docs =
                enable_api_docs.eq_ignore_ascii_case("true") || enable_api_docs == "1";
        }
        if let Ok(api_docs_path) = env::var(format!("{}_API_DOCS_PATH", prefix)) {
            self.api_docs_path = api_docs_path;
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
        if let Ok(max_page_size) = env::var(format!("{}_MAX_PAGE_SIZE", prefix))
            && let Ok(size) = max_page_size.parse()
        {
            self.max_page_size = size;
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

    // SAFETY: These tests run serially (via #[serial]) so there's no concurrent access to env vars.
    // env::set_var and env::remove_var are unsafe in Rust 2024 due to potential data races,
    // but serial execution ensures safety here.
    fn set_var(key: &str, value: &str) {
        unsafe { env::set_var(key, value) }
    }

    fn remove_var(key: &str) {
        unsafe { env::remove_var(key) }
    }

    #[test]
    #[serial]
    fn test_env_bool_or() {
        set_var("TEST_BOOL_TRUE", "true");
        set_var("TEST_BOOL_1", "1");
        set_var("TEST_BOOL_FALSE", "false");

        assert!(env_bool_or("TEST_BOOL_TRUE", false));
        assert!(env_bool_or("TEST_BOOL_1", false));
        assert!(!env_bool_or("TEST_BOOL_FALSE", false));
        assert!(env_bool_or("NONEXISTENT", true));

        remove_var("TEST_BOOL_TRUE");
        remove_var("TEST_BOOL_1");
        remove_var("TEST_BOOL_FALSE");
    }

    #[test]
    #[serial]
    fn test_env_or() {
        set_var("TEST_PORT", "9090");
        assert_eq!(env_or("TEST_PORT", 8080u16), 9090);
        assert_eq!(env_or("NONEXISTENT", 8080u16), 8080);
        remove_var("TEST_PORT");
    }

    #[test]
    #[serial]
    fn test_env_string_opt() {
        set_var("TEST_STRING", "value");
        set_var("TEST_EMPTY", "");

        assert_eq!(env_string_opt("TEST_STRING"), Some("value".to_string()));
        assert_eq!(env_string_opt("TEST_EMPTY"), None);
        assert_eq!(env_string_opt("NONEXISTENT"), None);

        remove_var("TEST_STRING");
        remove_var("TEST_EMPTY");
    }

    #[test]
    #[serial]
    fn test_application_config_override() {
        set_var("CODEX_APPLICATION_HOST", "0.0.0.0");
        set_var("CODEX_APPLICATION_PORT", "9090");

        let mut config = ApplicationConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
        };

        config.apply_env_overrides("CODEX_APPLICATION");

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 9090);

        remove_var("CODEX_APPLICATION_HOST");
        remove_var("CODEX_APPLICATION_PORT");
    }

    #[test]
    #[serial]
    fn test_task_config_env_override() {
        // Clear any existing env vars first to avoid interference from other tests
        remove_var("CODEX_TASK_WORKER_COUNT");

        set_var("CODEX_TASK_WORKER_COUNT", "8");

        let mut config = TaskConfig::default();
        config.apply_env_overrides("CODEX_TASK");

        assert_eq!(config.worker_count, 8);

        remove_var("CODEX_TASK_WORKER_COUNT");
    }

    #[test]
    #[serial]
    fn test_task_config_env_override_invalid() {
        // Clear any existing env vars first to avoid interference from other tests
        remove_var("CODEX_TASK_WORKER_COUNT");

        set_var("CODEX_TASK_WORKER_COUNT", "invalid");

        let mut config = TaskConfig { worker_count: 4 };
        config.apply_env_overrides("CODEX_TASK");

        // Should keep original value if env var is invalid
        assert_eq!(config.worker_count, 4);

        remove_var("CODEX_TASK_WORKER_COUNT");
    }

    #[test]
    #[serial]
    fn test_scanner_config_env_override() {
        // Clear any existing env vars first
        remove_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS");

        // Create config with explicit values (not using default which reads env vars)
        let mut config = ScannerConfig {
            max_concurrent_scans: 2,
        };

        // Set env vars and apply overrides
        set_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS", "6");
        config.apply_env_overrides("CODEX_SCANNER");

        assert_eq!(config.max_concurrent_scans, 6);

        remove_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS");
    }

    #[test]
    #[serial]
    fn test_scanner_config_env_override_partial() {
        // Clear any existing env vars first
        remove_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS");

        // Create config with explicit values (not using default which reads env vars)
        let mut config = ScannerConfig {
            max_concurrent_scans: 2,
        };

        set_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS", "10");
        config.apply_env_overrides("CODEX_SCANNER");

        assert_eq!(config.max_concurrent_scans, 10);

        remove_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS");
    }

    #[test]
    #[serial]
    fn test_config_env_override_task_and_scanner() {
        // Clear any existing env vars first
        remove_var("CODEX_TASK_WORKER_COUNT");
        remove_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS");

        // Create config with explicit values to avoid reading env vars in default()
        // We'll use a helper to create a minimal config
        use crate::config::{
            ApiConfig, ApplicationConfig, AuthConfig, DatabaseConfig, DatabaseType, EmailConfig,
            FilesConfig, KomgaApiConfig, LoggingConfig, PdfConfig, RateLimitConfig, SQLiteConfig,
        };
        let mut config = Config {
            database: DatabaseConfig {
                db_type: DatabaseType::SQLite,
                postgres: None,
                sqlite: Some(SQLiteConfig {
                    path: "./test.db".to_string(),
                    pragmas: None,
                    ..SQLiteConfig::default()
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
            files: FilesConfig::default(),
            pdf: PdfConfig::default(),
            komga_api: KomgaApiConfig::default(),
            rate_limit: RateLimitConfig::default(),
        };

        // Set env vars BEFORE applying overrides
        set_var("CODEX_TASK_WORKER_COUNT", "12");
        set_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS", "5");

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
            scanner_max_var_before,
            scanner_max_at_call
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
        let env_key_used = "CODEX_SCANNER_MAX_CONCURRENT_SCANS".to_string();
        let env_value_when_checked = env::var(&env_key_used).ok();

        assert_eq!(
            scanner_value_after,
            5,
            "Scanner max_concurrent_scans should be overridden to 5 (got: {}, was: {}, env var before: {:?}, env var at call: {:?}, env var after: {:?}, env key used: {:?}, env value when checked: {:?})",
            scanner_value_after,
            scanner_value_before,
            scanner_max_var_before,
            scanner_max_at_call,
            scanner_max_var_after,
            env_key_used,
            env_value_when_checked
        );

        remove_var("CODEX_TASK_WORKER_COUNT");
        remove_var("CODEX_SCANNER_MAX_CONCURRENT_SCANS");
    }

    #[test]
    #[serial]
    fn test_komga_api_config_env_override() {
        // Clear any existing env vars first
        remove_var("CODEX_KOMGA_API_ENABLED");
        remove_var("CODEX_KOMGA_API_PREFIX");

        // Create config with explicit values
        let mut config = KomgaApiConfig {
            enabled: false,
            prefix: "default".to_string(),
        };

        // Set env vars and apply overrides
        set_var("CODEX_KOMGA_API_ENABLED", "true");
        set_var("CODEX_KOMGA_API_PREFIX", "custom");
        config.apply_env_overrides("CODEX_KOMGA_API");

        assert!(config.enabled);
        assert_eq!(config.prefix, "custom");

        remove_var("CODEX_KOMGA_API_ENABLED");
        remove_var("CODEX_KOMGA_API_PREFIX");
    }

    #[test]
    #[serial]
    fn test_komga_api_config_env_override_enabled_with_1() {
        // Test that "1" is also accepted for enabled
        remove_var("CODEX_KOMGA_API_ENABLED");

        let mut config = KomgaApiConfig {
            enabled: false,
            prefix: "default".to_string(),
        };

        set_var("CODEX_KOMGA_API_ENABLED", "1");
        config.apply_env_overrides("CODEX_KOMGA_API");

        assert!(config.enabled);

        remove_var("CODEX_KOMGA_API_ENABLED");
    }

    #[test]
    #[serial]
    fn test_komga_api_config_env_override_partial() {
        // Test that partial env vars work (only enabled, not prefix)
        remove_var("CODEX_KOMGA_API_ENABLED");
        remove_var("CODEX_KOMGA_API_PREFIX");

        let mut config = KomgaApiConfig {
            enabled: false,
            prefix: "original".to_string(),
        };

        set_var("CODEX_KOMGA_API_ENABLED", "true");
        // Don't set PREFIX
        config.apply_env_overrides("CODEX_KOMGA_API");

        assert!(config.enabled);
        assert_eq!(config.prefix, "original"); // Should remain unchanged

        remove_var("CODEX_KOMGA_API_ENABLED");
    }

    #[test]
    #[serial]
    fn test_komga_api_config_env_override_empty_prefix_ignored() {
        // Test that empty PREFIX env var is ignored
        remove_var("CODEX_KOMGA_API_PREFIX");

        let mut config = KomgaApiConfig {
            enabled: false,
            prefix: "original".to_string(),
        };

        set_var("CODEX_KOMGA_API_PREFIX", "");
        config.apply_env_overrides("CODEX_KOMGA_API");

        assert_eq!(config.prefix, "original"); // Should remain unchanged

        remove_var("CODEX_KOMGA_API_PREFIX");
    }

    #[test]
    #[serial]
    fn test_config_komga_api_env_override_via_main_config() {
        // Test that komga_api env overrides work through Config::apply_env_overrides
        remove_var("CODEX_KOMGA_API_ENABLED");
        remove_var("CODEX_KOMGA_API_PREFIX");

        use crate::config::{
            ApiConfig, ApplicationConfig, AuthConfig, DatabaseConfig, DatabaseType, EmailConfig,
            FilesConfig, KomgaApiConfig, LoggingConfig, PdfConfig, RateLimitConfig, SQLiteConfig,
        };
        let mut config = Config {
            database: DatabaseConfig {
                db_type: DatabaseType::SQLite,
                postgres: None,
                sqlite: Some(SQLiteConfig {
                    path: "./test.db".to_string(),
                    pragmas: None,
                    ..SQLiteConfig::default()
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
            files: FilesConfig::default(),
            pdf: PdfConfig::default(),
            komga_api: KomgaApiConfig {
                enabled: false,
                prefix: "default".to_string(),
            },
            rate_limit: RateLimitConfig::default(),
        };

        set_var("CODEX_KOMGA_API_ENABLED", "true");
        set_var("CODEX_KOMGA_API_PREFIX", "mykomga");
        config.apply_env_overrides("CODEX");

        assert!(config.komga_api.enabled);
        assert_eq!(config.komga_api.prefix, "mykomga");

        remove_var("CODEX_KOMGA_API_ENABLED");
        remove_var("CODEX_KOMGA_API_PREFIX");
    }

    #[test]
    #[serial]
    fn test_rate_limit_config_env_override() {
        // Clear any existing env vars first
        remove_var("CODEX_RATE_LIMIT_ENABLED");
        remove_var("CODEX_RATE_LIMIT_ANONYMOUS_RPS");
        remove_var("CODEX_RATE_LIMIT_ANONYMOUS_BURST");
        remove_var("CODEX_RATE_LIMIT_AUTHENTICATED_RPS");
        remove_var("CODEX_RATE_LIMIT_AUTHENTICATED_BURST");
        remove_var("CODEX_RATE_LIMIT_EXEMPT_PATHS");
        remove_var("CODEX_RATE_LIMIT_CLEANUP_INTERVAL_SECS");
        remove_var("CODEX_RATE_LIMIT_BUCKET_TTL_SECS");

        // Create config with explicit values
        let mut config = RateLimitConfig {
            enabled: true,
            anonymous_rps: 10,
            anonymous_burst: 50,
            authenticated_rps: 50,
            authenticated_burst: 200,
            exempt_paths: vec!["/health".to_string()],
            cleanup_interval_secs: 60,
            bucket_ttl_secs: 300,
        };

        // Set env vars and apply overrides
        set_var("CODEX_RATE_LIMIT_ENABLED", "false");
        set_var("CODEX_RATE_LIMIT_ANONYMOUS_RPS", "20");
        set_var("CODEX_RATE_LIMIT_ANONYMOUS_BURST", "100");
        set_var("CODEX_RATE_LIMIT_AUTHENTICATED_RPS", "100");
        set_var("CODEX_RATE_LIMIT_AUTHENTICATED_BURST", "400");
        set_var("CODEX_RATE_LIMIT_EXEMPT_PATHS", "/health, /metrics");
        set_var("CODEX_RATE_LIMIT_CLEANUP_INTERVAL_SECS", "120");
        set_var("CODEX_RATE_LIMIT_BUCKET_TTL_SECS", "600");
        config.apply_env_overrides("CODEX_RATE_LIMIT");

        assert!(!config.enabled);
        assert_eq!(config.anonymous_rps, 20);
        assert_eq!(config.anonymous_burst, 100);
        assert_eq!(config.authenticated_rps, 100);
        assert_eq!(config.authenticated_burst, 400);
        assert_eq!(
            config.exempt_paths,
            vec!["/health".to_string(), "/metrics".to_string()]
        );
        assert_eq!(config.cleanup_interval_secs, 120);
        assert_eq!(config.bucket_ttl_secs, 600);

        remove_var("CODEX_RATE_LIMIT_ENABLED");
        remove_var("CODEX_RATE_LIMIT_ANONYMOUS_RPS");
        remove_var("CODEX_RATE_LIMIT_ANONYMOUS_BURST");
        remove_var("CODEX_RATE_LIMIT_AUTHENTICATED_RPS");
        remove_var("CODEX_RATE_LIMIT_AUTHENTICATED_BURST");
        remove_var("CODEX_RATE_LIMIT_EXEMPT_PATHS");
        remove_var("CODEX_RATE_LIMIT_CLEANUP_INTERVAL_SECS");
        remove_var("CODEX_RATE_LIMIT_BUCKET_TTL_SECS");
    }

    #[test]
    #[serial]
    fn test_rate_limit_config_env_override_enabled_with_1() {
        // Test that "1" is also accepted for enabled
        remove_var("CODEX_RATE_LIMIT_ENABLED");

        let mut config = RateLimitConfig {
            enabled: false,
            anonymous_rps: 10,
            anonymous_burst: 50,
            authenticated_rps: 50,
            authenticated_burst: 200,
            exempt_paths: vec![],
            cleanup_interval_secs: 60,
            bucket_ttl_secs: 300,
        };

        set_var("CODEX_RATE_LIMIT_ENABLED", "1");
        config.apply_env_overrides("CODEX_RATE_LIMIT");

        assert!(config.enabled);

        remove_var("CODEX_RATE_LIMIT_ENABLED");
    }

    #[test]
    #[serial]
    fn test_rate_limit_config_env_override_partial() {
        // Test that partial env vars work (only enabled and anonymous_rps)
        remove_var("CODEX_RATE_LIMIT_ENABLED");
        remove_var("CODEX_RATE_LIMIT_ANONYMOUS_RPS");

        let mut config = RateLimitConfig {
            enabled: true,
            anonymous_rps: 10,
            anonymous_burst: 50,
            authenticated_rps: 50,
            authenticated_burst: 200,
            exempt_paths: vec!["/original".to_string()],
            cleanup_interval_secs: 60,
            bucket_ttl_secs: 300,
        };

        set_var("CODEX_RATE_LIMIT_ENABLED", "false");
        set_var("CODEX_RATE_LIMIT_ANONYMOUS_RPS", "5");
        config.apply_env_overrides("CODEX_RATE_LIMIT");

        assert!(!config.enabled);
        assert_eq!(config.anonymous_rps, 5);
        // These should remain unchanged
        assert_eq!(config.anonymous_burst, 50);
        assert_eq!(config.authenticated_rps, 50);
        assert_eq!(config.exempt_paths, vec!["/original".to_string()]);

        remove_var("CODEX_RATE_LIMIT_ENABLED");
        remove_var("CODEX_RATE_LIMIT_ANONYMOUS_RPS");
    }

    #[test]
    #[serial]
    fn test_rate_limit_config_env_override_invalid_values_ignored() {
        // Test that invalid env var values are ignored
        remove_var("CODEX_RATE_LIMIT_ANONYMOUS_RPS");
        remove_var("CODEX_RATE_LIMIT_CLEANUP_INTERVAL_SECS");

        let mut config = RateLimitConfig {
            enabled: true,
            anonymous_rps: 10,
            anonymous_burst: 50,
            authenticated_rps: 50,
            authenticated_burst: 200,
            exempt_paths: vec![],
            cleanup_interval_secs: 60,
            bucket_ttl_secs: 300,
        };

        set_var("CODEX_RATE_LIMIT_ANONYMOUS_RPS", "invalid");
        set_var("CODEX_RATE_LIMIT_CLEANUP_INTERVAL_SECS", "not_a_number");
        config.apply_env_overrides("CODEX_RATE_LIMIT");

        // Should keep original values when env vars are invalid
        assert_eq!(config.anonymous_rps, 10);
        assert_eq!(config.cleanup_interval_secs, 60);

        remove_var("CODEX_RATE_LIMIT_ANONYMOUS_RPS");
        remove_var("CODEX_RATE_LIMIT_CLEANUP_INTERVAL_SECS");
    }

    // OIDC Configuration Environment Override Tests

    #[test]
    #[serial]
    fn test_oidc_config_env_override() {
        // Clear any existing env vars first
        remove_var("CODEX_AUTH_OIDC_ENABLED");
        remove_var("CODEX_AUTH_OIDC_AUTO_CREATE_USERS");
        remove_var("CODEX_AUTH_OIDC_DEFAULT_ROLE");

        use crate::config::{OidcConfig, OidcDefaultRole};

        let mut config = OidcConfig {
            enabled: false,
            auto_create_users: true,
            default_role: OidcDefaultRole::Reader,
            providers: std::collections::HashMap::new(),
        };

        // Set env vars and apply overrides
        set_var("CODEX_AUTH_OIDC_ENABLED", "true");
        set_var("CODEX_AUTH_OIDC_AUTO_CREATE_USERS", "false");
        set_var("CODEX_AUTH_OIDC_DEFAULT_ROLE", "admin");
        config.apply_env_overrides("CODEX_AUTH_OIDC");

        assert!(config.enabled);
        assert!(!config.auto_create_users);
        assert!(matches!(config.default_role, OidcDefaultRole::Admin));

        remove_var("CODEX_AUTH_OIDC_ENABLED");
        remove_var("CODEX_AUTH_OIDC_AUTO_CREATE_USERS");
        remove_var("CODEX_AUTH_OIDC_DEFAULT_ROLE");
    }

    #[test]
    #[serial]
    fn test_oidc_config_env_override_enabled_with_1() {
        remove_var("CODEX_AUTH_OIDC_ENABLED");

        use crate::config::{OidcConfig, OidcDefaultRole};

        let mut config = OidcConfig {
            enabled: false,
            auto_create_users: true,
            default_role: OidcDefaultRole::Reader,
            providers: std::collections::HashMap::new(),
        };

        set_var("CODEX_AUTH_OIDC_ENABLED", "1");
        config.apply_env_overrides("CODEX_AUTH_OIDC");

        assert!(config.enabled);

        remove_var("CODEX_AUTH_OIDC_ENABLED");
    }

    #[test]
    #[serial]
    fn test_oidc_config_env_override_default_role_variants() {
        use crate::config::{OidcConfig, OidcDefaultRole};

        // Test maintainer role
        remove_var("CODEX_AUTH_OIDC_DEFAULT_ROLE");

        let mut config = OidcConfig {
            enabled: false,
            auto_create_users: true,
            default_role: OidcDefaultRole::Reader,
            providers: std::collections::HashMap::new(),
        };

        set_var("CODEX_AUTH_OIDC_DEFAULT_ROLE", "maintainer");
        config.apply_env_overrides("CODEX_AUTH_OIDC");
        assert!(matches!(config.default_role, OidcDefaultRole::Maintainer));

        // Test reader role (explicit)
        set_var("CODEX_AUTH_OIDC_DEFAULT_ROLE", "reader");
        config.apply_env_overrides("CODEX_AUTH_OIDC");
        assert!(matches!(config.default_role, OidcDefaultRole::Reader));

        remove_var("CODEX_AUTH_OIDC_DEFAULT_ROLE");
    }

    #[test]
    #[serial]
    fn test_oidc_provider_config_env_override() {
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_DISPLAY_NAME");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_ISSUER_URL");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_ID");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_SECRET");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_SCOPES");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_GROUPS_CLAIM");

        use crate::config::OidcProviderConfig;

        let mut provider = OidcProviderConfig {
            display_name: "Original".to_string(),
            issuer_url: "https://original.example.com".to_string(),
            client_id: "original-client".to_string(),
            client_secret: None,
            client_secret_env: None,
            scopes: vec![],
            role_mapping: std::collections::HashMap::new(),
            groups_claim: "groups".to_string(),
            username_claim: "preferred_username".to_string(),
            email_claim: "email".to_string(),
        };

        set_var(
            "CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_DISPLAY_NAME",
            "Authentik SSO",
        );
        set_var(
            "CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_ISSUER_URL",
            "https://auth.example.com",
        );
        set_var(
            "CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_ID",
            "new-client-id",
        );
        set_var(
            "CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_SECRET",
            "secret123",
        );
        set_var(
            "CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_SCOPES",
            "email, profile, groups",
        );
        set_var(
            "CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_GROUPS_CLAIM",
            "custom_groups",
        );
        provider.apply_env_overrides("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK");

        assert_eq!(provider.display_name, "Authentik SSO");
        assert_eq!(provider.issuer_url, "https://auth.example.com");
        assert_eq!(provider.client_id, "new-client-id");
        assert_eq!(provider.client_secret, Some("secret123".to_string()));
        assert_eq!(
            provider.scopes,
            vec![
                "email".to_string(),
                "profile".to_string(),
                "groups".to_string()
            ]
        );
        assert_eq!(provider.groups_claim, "custom_groups");

        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_DISPLAY_NAME");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_ISSUER_URL");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_ID");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_SECRET");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_SCOPES");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_GROUPS_CLAIM");
    }

    #[test]
    #[serial]
    fn test_oidc_config_existing_provider_env_override() {
        // Test that env vars override existing provider config
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_ID");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_SECRET");

        use crate::config::{OidcConfig, OidcDefaultRole, OidcProviderConfig};

        let mut providers = std::collections::HashMap::new();
        providers.insert(
            "authentik".to_string(),
            OidcProviderConfig {
                display_name: "Authentik".to_string(),
                issuer_url: "https://auth.example.com".to_string(),
                client_id: "yaml-client".to_string(),
                client_secret: Some("yaml-secret".to_string()),
                client_secret_env: None,
                scopes: vec!["email".to_string()],
                role_mapping: std::collections::HashMap::new(),
                groups_claim: "groups".to_string(),
                username_claim: "preferred_username".to_string(),
                email_claim: "email".to_string(),
            },
        );

        let mut config = OidcConfig {
            enabled: true,
            auto_create_users: true,
            default_role: OidcDefaultRole::Reader,
            providers,
        };

        // Override client_id and client_secret via env vars
        set_var(
            "CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_ID",
            "env-client",
        );
        set_var(
            "CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_SECRET",
            "env-secret",
        );
        config.apply_env_overrides("CODEX_AUTH_OIDC");

        let provider = config.providers.get("authentik").unwrap();
        assert_eq!(provider.client_id, "env-client");
        assert_eq!(provider.client_secret, Some("env-secret".to_string()));
        // Non-overridden values should remain
        assert_eq!(provider.display_name, "Authentik");
        assert_eq!(provider.issuer_url, "https://auth.example.com");

        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_ID");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_SECRET");
    }

    #[test]
    #[serial]
    fn test_oidc_config_dynamic_provider_creation_via_env() {
        // Test that a new provider can be created purely through env vars
        // This requires ISSUER_URL to be set (used as detection mechanism)
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_NEWPROVIDER_ISSUER_URL");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_NEWPROVIDER_CLIENT_ID");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_NEWPROVIDER_CLIENT_SECRET");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_NEWPROVIDER_DISPLAY_NAME");

        use crate::config::{OidcConfig, OidcDefaultRole};

        let mut config = OidcConfig {
            enabled: true,
            auto_create_users: true,
            default_role: OidcDefaultRole::Reader,
            providers: std::collections::HashMap::new(),
        };

        // Set env vars to create a new provider
        set_var(
            "CODEX_AUTH_OIDC_PROVIDERS_NEWPROVIDER_ISSUER_URL",
            "https://new.example.com",
        );
        set_var(
            "CODEX_AUTH_OIDC_PROVIDERS_NEWPROVIDER_CLIENT_ID",
            "new-client",
        );
        set_var(
            "CODEX_AUTH_OIDC_PROVIDERS_NEWPROVIDER_CLIENT_SECRET",
            "new-secret",
        );
        set_var(
            "CODEX_AUTH_OIDC_PROVIDERS_NEWPROVIDER_DISPLAY_NAME",
            "New Provider",
        );
        config.apply_env_overrides("CODEX_AUTH_OIDC");

        // The provider should now exist (key is lowercase with hyphens)
        assert!(config.providers.contains_key("newprovider"));
        let provider = config.providers.get("newprovider").unwrap();
        assert_eq!(provider.issuer_url, "https://new.example.com");
        assert_eq!(provider.client_id, "new-client");
        assert_eq!(provider.client_secret, Some("new-secret".to_string()));
        assert_eq!(provider.display_name, "New Provider");

        remove_var("CODEX_AUTH_OIDC_PROVIDERS_NEWPROVIDER_ISSUER_URL");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_NEWPROVIDER_CLIENT_ID");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_NEWPROVIDER_CLIENT_SECRET");
        remove_var("CODEX_AUTH_OIDC_PROVIDERS_NEWPROVIDER_DISPLAY_NAME");
    }

    #[test]
    #[serial]
    fn test_auth_config_oidc_env_override_via_parent() {
        // Test that OIDC env overrides work through AuthConfig::apply_env_overrides
        remove_var("CODEX_AUTH_OIDC_ENABLED");
        remove_var("CODEX_AUTH_OIDC_AUTO_CREATE_USERS");

        use crate::config::{AuthConfig, OidcConfig, OidcDefaultRole};

        let mut config = AuthConfig {
            jwt_secret: "test-secret".to_string(),
            jwt_expiry_hours: 24,
            refresh_token_enabled: false,
            refresh_token_expiry_days: 30,
            email_confirmation_required: false,
            argon2_memory_cost: 19456,
            argon2_time_cost: 2,
            argon2_parallelism: 1,
            oidc: OidcConfig {
                enabled: false,
                auto_create_users: true,
                default_role: OidcDefaultRole::Reader,
                providers: std::collections::HashMap::new(),
            },
        };

        set_var("CODEX_AUTH_OIDC_ENABLED", "true");
        set_var("CODEX_AUTH_OIDC_AUTO_CREATE_USERS", "false");
        config.apply_env_overrides("CODEX_AUTH");

        assert!(config.oidc.enabled);
        assert!(!config.oidc.auto_create_users);

        remove_var("CODEX_AUTH_OIDC_ENABLED");
        remove_var("CODEX_AUTH_OIDC_AUTO_CREATE_USERS");
    }
}
