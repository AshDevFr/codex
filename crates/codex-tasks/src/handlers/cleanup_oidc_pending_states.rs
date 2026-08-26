//! Handler for the `CleanupOidcPendingStates` task.
//!
//! An OIDC login that is started and then abandoned leaves a row behind, since
//! only a completed callback consumes one. Without a periodic sweep the table
//! grows for as long as the deployment runs.
//!
//! The sweep works on the table directly rather than on a service handle, so
//! it does not matter which process runs it. That matters because `serve` and
//! `worker` are separate deployments: a sweep that reached into one process's
//! memory would be cleaning a structure that the process holding the real
//! entries never shares.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::info;

use crate::handlers::TaskHandler;
use crate::types::TaskResult;
use codex_db::entities::tasks;
use codex_db::repositories::OidcPendingStateRepository;
use codex_events::EventBroadcaster;

/// Handler for deleting expired OIDC pending-login states.
#[derive(Default)]
pub struct CleanupOidcPendingStatesHandler;

impl CleanupOidcPendingStatesHandler {
    pub fn new() -> Self {
        Self
    }
}

impl TaskHandler for CleanupOidcPendingStatesHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            info!("Task {}: Starting OIDC pending-state cleanup", task.id);

            let deleted = OidcPendingStateRepository::delete_expired(db).await?;

            info!(
                "Task {}: OIDC pending-state cleanup complete - deleted {} rows",
                task.id, deleted
            );

            Ok(TaskResult::success_with_data(
                format!("Cleaned up {} expired OIDC pending states", deleted),
                json!({ "deleted_count": deleted }),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use codex_config::{DatabaseConfig, DatabaseType, SQLiteConfig};
    use codex_db::Database;
    use codex_db::repositories::NewOidcPendingState;
    use std::collections::HashMap;
    use tempfile::TempDir;
    use uuid::Uuid;

    async fn setup() -> (DatabaseConnection, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let mut pragmas = HashMap::new();
        pragmas.insert("foreign_keys".to_string(), "ON".to_string());

        let config = DatabaseConfig {
            db_type: DatabaseType::SQLite,
            postgres: None,
            sqlite: Some(SQLiteConfig {
                path: db_path.to_str().unwrap().to_string(),
                pragmas: Some(pragmas),
                ..SQLiteConfig::default()
            }),
            ..DatabaseConfig::default()
        };

        let db = Database::new(&config).await.unwrap();
        db.run_migrations().await.unwrap();
        (db.sea_orm_connection().clone(), temp_dir)
    }

    async fn insert(db: &DatabaseConnection, state: &str, expires_in_secs: i64) {
        let now = Utc::now();
        OidcPendingStateRepository::create(
            db,
            NewOidcPendingState {
                state: state.to_string(),
                provider_name: "authentik".to_string(),
                pkce_verifier: "v".to_string(),
                nonce: "n".to_string(),
                redirect_uri: None,
                created_at: now,
                expires_at: now + Duration::seconds(expires_in_secs),
            },
        )
        .await
        .unwrap();
    }

    fn task_row() -> tasks::Model {
        let now = Utc::now();
        tasks::Model {
            id: Uuid::new_v4(),
            task_type: "cleanup_oidc_pending_states".to_string(),
            library_id: None,
            series_id: None,
            book_id: None,
            params: None,
            status: "running".to_string(),
            priority: 100,
            locked_by: None,
            locked_until: None,
            attempts: 0,
            max_attempts: 1,
            last_error: None,
            reschedule_count: 0,
            max_reschedules: 0,
            result: None,
            progress: None,
            scheduled_for: now,
            created_at: now,
            started_at: Some(now),
            completed_at: None,
        }
    }

    #[tokio::test]
    async fn cleanup_deletes_only_expired_states() {
        let (db, _temp) = setup().await;
        insert(&db, "live", 300).await;
        insert(&db, "stale", -1).await;

        let result = CleanupOidcPendingStatesHandler::new()
            .handle(&task_row(), &db, None)
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(OidcPendingStateRepository::count(&db).await.unwrap(), 1);
        assert!(
            OidcPendingStateRepository::consume(&db, "live")
                .await
                .unwrap()
                .is_some(),
            "an unexpired login must survive the sweep"
        );
    }

    #[tokio::test]
    async fn cleanup_on_empty_table_succeeds() {
        let (db, _temp) = setup().await;

        let result = CleanupOidcPendingStatesHandler::new()
            .handle(&task_row(), &db, None)
            .await
            .unwrap();

        assert!(result.success);
    }
}
