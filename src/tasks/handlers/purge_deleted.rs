use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use tracing::{error, info};

use crate::db::entities::tasks;
use crate::db::repositories::BookRepository;
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

pub struct PurgeDeletedHandler;

impl PurgeDeletedHandler {
    pub fn new() -> Self {
        Self
    }
}

impl TaskHandler for PurgeDeletedHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let library_id = task
                .library_id
                .ok_or_else(|| anyhow::anyhow!("Missing library_id"))?;

            info!(
                "Task {}: Purging deleted books from library {}",
                task.id, library_id
            );

            match BookRepository::purge_deleted_in_library(db, library_id).await {
                Ok(deleted_count) => {
                    info!(
                        "Task {}: Purged {} deleted books from library {}",
                        task.id, deleted_count, library_id
                    );

                    Ok(TaskResult::success_with_data(
                        format!("Purged {} deleted books", deleted_count),
                        json!({
                            "deleted_count": deleted_count,
                            "library_id": library_id,
                        }),
                    ))
                }
                Err(e) => {
                    error!("Task {}: Purge failed: {}", task.id, e);
                    Err(e)
                }
            }
        })
    }
}
