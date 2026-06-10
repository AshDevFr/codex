//! Codex configuration types, loaders, and environment-override plumbing.
//!
//! Extracted from the monolithic `codex` crate as the first workspace leaf in
//! the workspace-split plan. Has no dependencies on other Codex crates.

mod env_override;
mod loader;
mod types;

#[allow(unused_imports)]
pub use types::{
    ApiConfig, ApplicationConfig, AuthConfig, Config, DatabaseConfig, DatabaseType, EmailConfig,
    FilesConfig, KomgaApiConfig, KoreaderApiConfig, LoggingConfig, ObservabilityBrowserConfig,
    ObservabilityConfig, ObservabilityMetricsConfig, ObservabilityTracesConfig, OidcConfig,
    OidcDefaultRole, OidcProviderConfig, OtlpConfig, OtlpProtocol, PdfConfig, PdfHandleCacheConfig,
    PostgresConfig, RateLimitConfig, SQLiteConfig, ScannerConfig, SchedulerConfig, TaskConfig,
};

pub use env_override::EnvOverride;
