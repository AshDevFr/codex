use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::info;

use crate::db::entities::tasks;
use crate::db::repositories::{BookDuplicatesRepository, SeriesDuplicatesRepository};
use crate::events::EventBroadcaster;
use crate::tasks::types::TaskResult;

use super::TaskHandler;

/// Handler for finding duplicate books
pub struct FindDuplicatesHandler;

impl Default for FindDuplicatesHandler {
    fn default() -> Self {
        Self::new()
    }
}

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
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            info!("Starting duplicate detection scan");

            let book_groups = BookDuplicatesRepository::rebuild_from_books(db).await?;
            let series_groups = SeriesDuplicatesRepository::rebuild_from_series(db).await?;

            info!(
                "Duplicate detection complete: {} book groups, {} series groups",
                book_groups, series_groups
            );

            Ok(TaskResult::success_with_data(
                format!(
                    "Found {} book and {} series duplicate groups",
                    book_groups, series_groups
                ),
                serde_json::json!({
                    "duplicate_groups": book_groups,
                    "book_duplicate_groups": book_groups,
                    "series_duplicate_groups": series_groups,
                }),
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
