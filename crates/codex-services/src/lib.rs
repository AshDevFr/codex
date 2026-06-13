pub mod auth_tracking;
pub mod book_export_collector;

// OTel meter / instrument plumbing for plugin and task lifecycle events.
// Behind the `observability` feature; the stub keeps callsites cfg-free.
#[cfg(feature = "observability")]
pub mod metrics;
#[cfg(not(feature = "observability"))]
#[path = "metrics_stub.rs"]
pub mod metrics;

pub mod cleanup_subscriber;
pub mod content_filter;
pub mod email;
pub mod export_storage;
pub mod file_cleanup;
pub mod filter;
pub mod idp_bearer;
pub mod image_decode;
pub mod inflight_thumbnails;
pub mod library_jobs;
pub mod metadata;
pub mod oidc;
pub mod pdf_cache;
pub mod pdf_handle_cache;
pub mod pdf_handle_cache_subscriber;
pub mod plugin;
pub mod plugin_file_storage;
pub mod plugin_metrics;
pub mod rate_limiter;
pub mod read_progress;
pub mod refresh_token;
pub mod release;
pub mod scheduler_handle;
pub mod series_export_collector;
pub mod series_export_writer;
pub mod settings;
pub mod task_listener;
pub mod task_metrics;
pub mod thumbnail;
pub mod user_plugin;

pub use auth_tracking::AuthTrackingService;
pub use cleanup_subscriber::CleanupEventSubscriber;
pub use export_storage::ExportStorage;
pub use file_cleanup::{CleanupStats, FileCleanupService, OrphanedFileType};
pub use filter::FilterService;
pub use idp_bearer::{IdpBearerError, IdpBearerValidator, ValidatedIdpToken};
pub use inflight_thumbnails::InflightThumbnailTracker;
pub use oidc::OidcService;
pub use pdf_cache::{CacheStats, CleanupResult, PdfPageCache};
pub use pdf_handle_cache::{HandleCacheEntrySnapshot, HandleCacheSnapshot, PdfHandleCache};
pub use pdf_handle_cache_subscriber::PdfHandleCacheSubscriber;
pub use rate_limiter::RateLimiterService;
pub use read_progress::ReadProgressService;
#[allow(unused_imports)]
pub use refresh_token::{IssuedRefreshToken, RefreshTokenError, RefreshTokenService};
pub use settings::SettingsService;
pub use task_listener::TaskListener;
pub use task_metrics::TaskMetricsService;
pub use thumbnail::ThumbnailService;

// Historical alias. The canonical location is `codex_utils::credential_encryption`.
#[allow(unused_imports)]
pub use codex_utils::credential_encryption::CredentialEncryption;
pub use plugin_file_storage::{PluginCleanupStats, PluginFileStorage, PluginStorageStats};
pub use plugin_metrics::{PluginHealthStatus, PluginMetricsService};
