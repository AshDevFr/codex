mod env_override;
mod loader;
mod types;

// Re-export all config types for external use (used by integration tests)
#[allow(unused_imports)]
pub use types::{
    ApiConfig, ApplicationConfig, AuthConfig, Config, DatabaseConfig, DatabaseType, EmailConfig,
    FilesConfig, KomgaApiConfig, KoreaderApiConfig, LoggingConfig, OidcConfig, OidcDefaultRole,
    OidcProviderConfig, PdfConfig, PostgresConfig, RateLimitConfig, SQLiteConfig, ScannerConfig,
    SchedulerConfig, TaskConfig,
};

pub use env_override::EnvOverride;
