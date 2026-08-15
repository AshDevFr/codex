//! Codex configuration types, loaders, and environment-override plumbing.
//!
//! Extracted from the monolithic `codex` crate as the first workspace leaf in
//! the workspace-split plan. Has no dependencies on other Codex crates.

mod env_audit;
mod env_override;
mod keys;
mod loader;
mod redact;
mod types;

#[allow(unused_imports)]
pub use types::{
    ApiConfig, ApplicationConfig, AuthConfig, Config, DatabaseConfig, DatabaseType, EmailConfig,
    FilesConfig, KomgaApiConfig, KoreaderApiConfig, LoggingConfig, ObservabilityBrowserConfig,
    ObservabilityConfig, ObservabilityMetricsConfig, ObservabilityTracesConfig, OidcConfig,
    OidcDefaultRole, OidcProviderConfig, OtlpConfig, OtlpProtocol, PdfConfig, PdfHandleCacheConfig,
    PostgresConfig, RateLimitConfig, SQLiteConfig, ScannerConfig, SchedulerConfig, TaskConfig,
};

pub use env_audit::{
    ENV_PREFIX, Finding, NON_CONFIG_VARS, audit, audit_env, audit_env_with_config, classify,
    secret_env_targets, v1_name_for, v2_name_for,
};
pub use env_override::EnvOverride;
pub use keys::{KeyRegistry, registry};
pub use redact::{REDACTED, UNSET, redacted_value, redacted_yaml};
