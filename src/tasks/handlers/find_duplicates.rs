use anyhow::Result;
use sea_orm::DatabaseConnection;
use tracing::info;

use crate::db::entities::tasks;
use crate::db::repositories::BookDuplicatesRepository;
use crate::tasks::types::TaskResult;

use super::TaskHandler;

/// Handler for finding duplicate books
pub struct FindDuplicatesHandler;

impl FindDuplicatesHandler {
    pub fn new() -> Self {
        Self
    }
}

impl TaskHandler for FindDuplicatesHandler {
    fn handle<'a>(
        &'a self,
        _task: &'a tasks::Model,
        db: &'a DatabaseConnection,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            info!("Starting duplicate detection scan");

            // Rebuild duplicates table from current books
            let count = BookDuplicatesRepository::rebuild_from_books(db).await?;

            info!(
                "Duplicate detection complete: {} duplicate groups found",
                count
            );

            Ok(TaskResult::success_with_data(
                format!("Found {} duplicate groups", count),
                serde_json::json!({ "duplicate_groups": count }),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_creation() {
        let _handler = FindDuplicatesHandler::new();
    }
}
