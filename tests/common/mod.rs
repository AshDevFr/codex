// Test helper modules organized by functionality
// Allow unused imports/dead code - different test files use different subsets of helpers
// Allow duplicate_mod - this module is intentionally loaded by multiple test files via #[path]
#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(clippy::duplicate_mod)]

pub mod db;
pub mod files;
pub mod fixtures;
pub mod http;

// Re-export commonly used items for convenience
pub use db::*;
pub use files::*;
pub use fixtures::*;
pub use http::*;

// Helper function to trigger scans via task queue (replacement for ScanManager)
pub async fn trigger_scan_task(
    db: &sea_orm::DatabaseConnection,
    library_id: uuid::Uuid,
    mode: codex::scanner::ScanMode,
) -> anyhow::Result<uuid::Uuid> {
    use codex::db::entities::{prelude::*, tasks};
    use codex::db::repositories::TaskRepository;
    use codex::tasks::types::TaskType;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    // Check if there's already a scan task pending or processing for this library
    let existing_scan = Tasks::find()
        .filter(tasks::Column::TaskType.eq("scan_library"))
        .filter(tasks::Column::LibraryId.eq(library_id))
        .filter(
            tasks::Column::Status
                .eq("pending")
                .or(tasks::Column::Status.eq("processing")),
        )
        .one(db)
        .await?;

    if existing_scan.is_some() {
        return Err(anyhow::anyhow!(
            "Library {} is already being scanned",
            library_id
        ));
    }

    let task_type = TaskType::ScanLibrary {
        library_id,
        mode: mode.to_string(),
    };

    TaskRepository::enqueue(db, task_type, None).await
}
