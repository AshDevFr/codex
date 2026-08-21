//! Handler for CleanupPluginData task
//!
//! Periodically cleans up expired key-value data from plugin storage, both
//! the per-user `user_plugin_data` table and the system-scoped `plugin_data`
//! table, plus abandoned OAuth connect flows in `user_plugin_oauth_states`.
//! Entries with a past `expires_at` timestamp are deleted in bulk.
//!
//! The OAuth sweep works on the table rather than on an `OAuthStateManager`
//! handle. It used to take the handle, and that never worked in a split
//! deployment: `serve` built the manager holding the real flows while the
//! standalone `worker` that runs this task was never given one, so the sweep
//! ran against nothing while the flows accumulated in another process.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::info;

use crate::handlers::TaskHandler;
use crate::types::TaskResult;
use codex_db::entities::tasks;
use codex_db::repositories::{
    PluginDataRepository, UserPluginDataRepository, UserPluginOAuthStateRepository,
};
use codex_events::EventBroadcaster;

/// Handler for cleaning up expired plugin storage data and OAuth state
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

            let deleted_count = UserPluginDataRepository::cleanup_expired(db).await?
                + PluginDataRepository::cleanup_expired(db).await?;

            // Clean up abandoned OAuth connect flows.
            let oauth_cleaned = UserPluginOAuthStateRepository::delete_expired(db).await?;
            let oauth_remaining = UserPluginOAuthStateRepository::count(db).await?;

            info!(
                "Task {}: Plugin data cleanup complete - deleted {} expired storage entries, \
                 {} expired OAuth flows ({} still pending)",
                task.id, deleted_count, oauth_cleaned, oauth_remaining
            );

            Ok(TaskResult::success_with_data(
                format!(
                    "Cleaned up {} expired plugin data entries, {} expired OAuth flows",
                    deleted_count, oauth_cleaned
                ),
                json!({
                    "deleted_count": deleted_count,
                    "oauth_flows_cleaned": oauth_cleaned,
                    "oauth_flows_remaining": oauth_remaining,
                }),
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
    use codex_db::entities::{tasks, users};
    use codex_db::repositories::{NewUserPluginOAuthState, PluginsRepository, UserRepository};
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

    async fn create_test_user(db: &DatabaseConnection) -> users::Model {
        let now = Utc::now();
        let user = users::Model {
            id: Uuid::new_v4(),
            username: format!("u-{}", Uuid::new_v4()),
            email: format!("{}@example.com", Uuid::new_v4()),
            password_hash: "h".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: true,
            permissions: json!([]),
            created_at: now,
            updated_at: now,
            last_login_at: None,
        };
        UserRepository::create(db, &user).await.unwrap()
    }

    async fn create_test_plugin(db: &DatabaseConnection) -> Uuid {
        PluginsRepository::create(
            db,
            &format!("cleanup_plugin_{}", Uuid::new_v4()),
            "Cleanup Test Plugin",
            Some("A test plugin"),
            "user",
            "node",
            vec!["index.js".to_string()],
            vec![],
            None,
            vec![],
            vec![],
            vec![],
            None,
            "env",
            None,
            true,
            None,
            None,
            None,
            None, // log_level
        )
        .await
        .unwrap()
        .id
    }

    /// Persist an OAuth flow directly, as another process would have.
    async fn insert_flow(
        db: &DatabaseConnection,
        plugin_id: Uuid,
        user_id: Uuid,
        expires_in_secs: i64,
    ) {
        let now = Utc::now();
        UserPluginOAuthStateRepository::create(
            db,
            NewUserPluginOAuthState {
                state: Uuid::new_v4().to_string(),
                plugin_id,
                user_id,
                pkce_verifier: None,
                pkce_challenge: None,
                redirect_uri: "https://example.com/callback".to_string(),
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
            task_type: "cleanup_plugin_data".to_string(),
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
            scheduled_for: now,
            created_at: now,
            started_at: Some(now),
            completed_at: None,
        }
    }

    #[test]
    fn test_handler_creation() {
        let _handler = CleanupPluginDataHandler::new();
    }

    /// The sweep has to reach flows this process never created.
    ///
    /// This is the regression guard for the reason the OAuth sweep moved off an
    /// in-process handle: `serve` creates the flows and the standalone `worker`
    /// runs this task, so a handler that could only see its own process's
    /// memory swept nothing at all in a split deployment.
    #[tokio::test]
    async fn cleanup_removes_expired_oauth_flows_created_elsewhere() {
        let (db, _temp) = setup().await;
        let plugin_id = create_test_plugin(&db).await;
        let user = create_test_user(&db).await;

        insert_flow(&db, plugin_id, user.id, 300).await;
        insert_flow(&db, plugin_id, user.id, -1).await;

        let result = CleanupPluginDataHandler::new()
            .handle(&task_row(), &db, None)
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(
            UserPluginOAuthStateRepository::count(&db).await.unwrap(),
            1,
            "only the expired flow should have been swept"
        );
    }

    #[tokio::test]
    async fn cleanup_leaves_live_oauth_flows_alone() {
        let (db, _temp) = setup().await;
        let plugin_id = create_test_plugin(&db).await;
        let user = create_test_user(&db).await;

        insert_flow(&db, plugin_id, user.id, 300).await;

        CleanupPluginDataHandler::new()
            .handle(&task_row(), &db, None)
            .await
            .unwrap();

        assert_eq!(UserPluginOAuthStateRepository::count(&db).await.unwrap(), 1);
    }
}
