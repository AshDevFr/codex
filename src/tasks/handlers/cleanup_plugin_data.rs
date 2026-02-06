//! Handler for CleanupPluginData task
//!
//! Periodically cleans up expired key-value data from plugin storage
//! (`user_plugin_data` table). Entries with a past `expires_at` timestamp
//! are deleted in bulk.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::info;

use crate::db::entities::tasks;
use crate::db::repositories::UserPluginDataRepository;
use crate::events::EventBroadcaster;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

/// Handler for cleaning up expired plugin storage data
#[derive(Default)]
pub struct CleanupPluginDataHandler;

impl CleanupPluginDataHandler {
    pub fn new() -> Self {
        Self
    }
}

impl TaskHandler for CleanupPluginDataHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            info!("Task {}: Starting plugin data cleanup", task.id);

            let deleted_count = UserPluginDataRepository::cleanup_expired(db).await?;

            info!(
                "Task {}: Plugin data cleanup complete - deleted {} expired entries",
                task.id, deleted_count
            );

            Ok(TaskResult::success_with_data(
                format!("Cleaned up {} expired plugin data entries", deleted_count),
                json!({
                    "deleted_count": deleted_count,
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
        let _handler = CleanupPluginDataHandler::new();
    }
}
