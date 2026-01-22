mod env_override;
mod loader;
mod types;

// Re-export all config types for external use (used by integration tests)
#[allow(unused_imports)]
pub use types::{
    ApiConfig, ApplicationConfig, AuthConfig, Config, DatabaseConfig, DatabaseType, EmailConfig,
    FilesConfig, KomgaApiConfig, LoggingConfig, PdfConfig, PostgresConfig, SQLiteConfig,
    ScannerConfig, TaskConfig,
};

pub use env_override::EnvOverride;
