use anyhow::Result;
use sea_orm::DatabaseConnection;

use crate::db::entities::tasks;
use crate::tasks::types::TaskResult;

pub mod analyze_book;
pub mod analyze_series;
pub mod find_duplicates;
pub mod generate_thumbnails;
pub mod purge_deleted;
pub mod scan_library;

pub use analyze_book::AnalyzeBookHandler;
pub use analyze_series::AnalyzeSeriesHandler;
pub use find_duplicates::FindDuplicatesHandler;
pub use generate_thumbnails::GenerateThumbnailsHandler;
pub use purge_deleted::PurgeDeletedHandler;
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
    ) -> Pin<Box<dyn Future<Output = Result<TaskResult>> + Send + 'a>>;
}
