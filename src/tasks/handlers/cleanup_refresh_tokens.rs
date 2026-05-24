//! Handler for `CleanupRefreshTokens` task.
//!
//! Deletes refresh-token rows that are past their `expires_at` or have been
//! revoked for longer than the configured grace period. The grace period
//! retains revoked rows briefly so audit / theft-detection traces stay
//! readable in incident response.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::info;

use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;
use codex_db::entities::tasks;
use codex_db::repositories::RefreshTokenRepository;
use codex_events::EventBroadcaster;

/// Days a revoked refresh-token row sticks around before cleanup deletes it.
const REVOKED_GRACE_DAYS: i64 = 30;

#[derive(Default)]
pub struct CleanupRefreshTokensHandler;

impl CleanupRefreshTokensHandler {
    pub fn new() -> Self {
        Self
    }
}

impl TaskHandler for CleanupRefreshTokensHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            info!("Task {}: Starting refresh-token cleanup", task.id);

            let deleted = RefreshTokenRepository::cleanup_expired(db, REVOKED_GRACE_DAYS).await?;

            info!(
                "Task {}: Refresh-token cleanup complete - deleted {} rows",
                task.id, deleted
            );

            Ok(TaskResult::success_with_data(
                format!("Cleaned up {} expired refresh tokens", deleted),
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
    use codex_db::entities::users;
    use codex_db::repositories::{NewRefreshToken, UserRepository};
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
        };

        let db = Database::new(&config).await.unwrap();
        db.run_migrations().await.unwrap();
        (db.sea_orm_connection().clone(), temp_dir)
    }

    async fn create_user(db: &DatabaseConnection) -> users::Model {
        let now = Utc::now();
        let model = users::Model {
            id: Uuid::new_v4(),
            username: format!("u-{}", Uuid::new_v4()),
            email: format!("{}@ex.com", Uuid::new_v4()),
            password_hash: "h".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: true,
            permissions: serde_json::json!([]),
            created_at: now,
            updated_at: now,
            last_login_at: None,
        };
        UserRepository::create(db, &model).await.unwrap()
    }

    #[tokio::test]
    async fn cleanup_deletes_expired_and_old_revoked() {
        let (db, _tmp) = setup().await;
        let user = create_user(&db).await;
        let now = Utc::now();

        // A live token (must be retained).
        let live_id = RefreshTokenRepository::create(
            &db,
            NewRefreshToken {
                user_id: user.id,
                family_id: Uuid::new_v4(),
                token_hash: "a".repeat(64),
                issued_at: now,
                expires_at: now + Duration::days(30),
                user_agent: None,
                ip_address: None,
            },
        )
        .await
        .unwrap()
        .id;

        // An expired but un-revoked token (deletes via expires_at branch).
        RefreshTokenRepository::create(
            &db,
            NewRefreshToken {
                user_id: user.id,
                family_id: Uuid::new_v4(),
                token_hash: "b".repeat(64),
                issued_at: now - Duration::days(40),
                expires_at: now - Duration::days(10),
                user_agent: None,
                ip_address: None,
            },
        )
        .await
        .unwrap();

        // A recently-revoked token (must be retained - inside grace window).
        let recent_revoked = RefreshTokenRepository::create(
            &db,
            NewRefreshToken {
                user_id: user.id,
                family_id: Uuid::new_v4(),
                token_hash: "c".repeat(64),
                issued_at: now,
                expires_at: now + Duration::days(30),
                user_agent: None,
                ip_address: None,
            },
        )
        .await
        .unwrap();
        RefreshTokenRepository::revoke(&db, recent_revoked.id)
            .await
            .unwrap();

        let deleted = RefreshTokenRepository::cleanup_expired(&db, REVOKED_GRACE_DAYS)
            .await
            .unwrap();
        assert_eq!(deleted, 1, "only the expired row should be removed");

        // Live token still present.
        assert!(
            RefreshTokenRepository::get_by_id(&db, live_id)
                .await
                .unwrap()
                .is_some()
        );
        // Recently-revoked still present.
        assert!(
            RefreshTokenRepository::get_by_id(&db, recent_revoked.id)
                .await
                .unwrap()
                .is_some()
        );
    }
}
