use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::db::entities::tasks;
use crate::events::EventBroadcaster;
use crate::tasks::types::TaskResult;

pub mod analyze_book;
pub mod analyze_series;
pub mod cleanup_book_files;
pub mod cleanup_orphaned_files;
pub mod cleanup_pdf_cache;
pub mod cleanup_plugin_data;
pub mod cleanup_series_files;
pub mod find_duplicates;
pub mod generate_series_thumbnail;
pub mod generate_series_thumbnails;
pub mod generate_thumbnail;
pub mod generate_thumbnails;
pub mod plugin_auto_match;
pub mod purge_deleted;
pub mod reprocess_series_titles;
pub mod scan_library;

pub use analyze_book::AnalyzeBookHandler;
pub use analyze_series::AnalyzeSeriesHandler;
pub use cleanup_book_files::CleanupBookFilesHandler;
pub use cleanup_orphaned_files::CleanupOrphanedFilesHandler;
pub use cleanup_pdf_cache::CleanupPdfCacheHandler;
pub use cleanup_plugin_data::CleanupPluginDataHandler;
pub use cleanup_series_files::CleanupSeriesFilesHandler;
pub use find_duplicates::FindDuplicatesHandler;
pub use generate_series_thumbnail::GenerateSeriesThumbnailHandler;
pub use generate_series_thumbnails::GenerateSeriesThumbnailsHandler;
pub use generate_thumbnail::GenerateThumbnailHandler;
pub use generate_thumbnails::GenerateThumbnailsHandler;
pub use plugin_auto_match::PluginAutoMatchHandler;
pub use purge_deleted::PurgeDeletedHandler;
pub use reprocess_series_titles::{ReprocessSeriesTitleHandler, ReprocessSeriesTitlesHandler};
pub use scan_library::ScanLibraryHandler;

use std::future::Future;
use std::pin::Pin;

/// Trait for task handlers
pub trait TaskHandler: Send + Sync {
    /// Handle a task and return the result
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> Pin<Box<dyn Future<Output = Result<TaskResult>> + Send + 'a>>;
}
