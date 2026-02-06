//! Authentication tracking batching service
//!
//! Collects API key last_used and user last_login timestamp updates in memory
//! and flushes them to the database periodically to reduce database load
//! during high-traffic authentication scenarios.

use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::time::{Duration as TokioDuration, interval};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};
use uuid::Uuid;

use crate::db::repositories::{ApiKeyRepository, UserRepository};

/// Default flush interval in seconds (longer than read progress since timestamps
/// don't need to be as precise)
const DEFAULT_FLUSH_INTERVAL_SECS: u64 = 60;

/// Authentication tracking batching service
///
/// Collects API key last_used and user last_login timestamp updates in memory
/// and periodically flushes them to the database. This reduces the number of
/// database operations during high-traffic API authentication scenarios.
#[derive(Clone)]
pub struct AuthTrackingService {
    /// In-memory buffer of pending API key last_used updates: key_id -> timestamp
    api_key_buffer: Arc<DashMap<Uuid, DateTime<Utc>>>,
    /// In-memory buffer of pending user last_login updates: user_id -> timestamp
    user_login_buffer: Arc<DashMap<Uuid, DateTime<Utc>>>,
    /// Database connection for flushing
    db: DatabaseConnection,
}

impl AuthTrackingService {
    /// Create a new auth tracking service
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            api_key_buffer: Arc::new(DashMap::new()),
            user_login_buffer: Arc::new(DashMap::new()),
            db,
        }
    }

    /// Record an API key usage
    ///
    /// Updates are buffered in memory and flushed periodically.
    /// If the same API key is used multiple times before flush, only the
    /// latest timestamp is recorded (deduplication).
    pub fn record_api_key_used(&self, key_id: Uuid) {
        self.api_key_buffer.insert(key_id, Utc::now());
    }

    /// Record a user login
    ///
    /// Updates are buffered in memory and flushed periodically.
    /// If the same user logs in multiple times before flush, only the
    /// latest timestamp is recorded (deduplication).
    pub fn record_user_login(&self, user_id: Uuid) {
        self.user_login_buffer.insert(user_id, Utc::now());
    }

    /// Get the current number of pending API key updates
    #[cfg(test)]
    pub fn pending_api_key_count(&self) -> usize {
        self.api_key_buffer.len()
    }

    /// Get the current number of pending user login updates
    #[cfg(test)]
    pub fn pending_user_login_count(&self) -> usize {
        self.user_login_buffer.len()
    }

    /// Flush all pending API key last_used updates to the database
    async fn flush_api_keys(&self) -> Result<usize> {
        // Take all pending entries
        let entries: Vec<_> = self
            .api_key_buffer
            .iter()
            .map(|entry| (*entry.key(), *entry.value()))
            .collect();

        if entries.is_empty() {
            return Ok(0);
        }

        // Clear buffer before processing
        for (key_id, _) in &entries {
            self.api_key_buffer.remove(key_id);
        }

        let count = entries.len();
        debug!("Flushing {} API key last_used updates", count);

        // Process each entry
        for (key_id, _timestamp) in entries {
            if let Err(e) = ApiKeyRepository::update_last_used(&self.db, key_id).await {
                error!(
                    "Failed to flush API key last_used for key {}: {}",
                    key_id, e
                );
                // Re-add to buffer for retry on next flush
                self.api_key_buffer.insert(key_id, Utc::now());
            }
        }

        Ok(count)
    }

    /// Flush all pending user last_login updates to the database
    async fn flush_user_logins(&self) -> Result<usize> {
        // Take all pending entries
        let entries: Vec<_> = self
            .user_login_buffer
            .iter()
            .map(|entry| (*entry.key(), *entry.value()))
            .collect();

        if entries.is_empty() {
            return Ok(0);
        }

        // Clear buffer before processing
        for (user_id, _) in &entries {
            self.user_login_buffer.remove(user_id);
        }

        let count = entries.len();
        debug!("Flushing {} user last_login updates", count);

        // Process each entry
        for (user_id, _timestamp) in entries {
            if let Err(e) = UserRepository::update_last_login(&self.db, user_id).await {
                error!(
                    "Failed to flush user last_login for user {}: {}",
                    user_id, e
                );
                // Re-add to buffer for retry on next flush
                self.user_login_buffer.insert(user_id, Utc::now());
            }
        }

        Ok(count)
    }

    /// Flush all pending updates to the database
    pub async fn flush(&self) -> Result<(usize, usize)> {
        let api_key_count = self.flush_api_keys().await?;
        let user_login_count = self.flush_user_logins().await?;
        Ok((api_key_count, user_login_count))
    }

    /// Start the background flush job
    ///
    /// Accepts a `CancellationToken` for graceful shutdown support.
    /// Returns a `JoinHandle` that can be awaited on shutdown.
    pub fn start_background_flush(
        self: Arc<Self>,
        cancel_token: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut flush_interval =
                interval(TokioDuration::from_secs(DEFAULT_FLUSH_INTERVAL_SECS));

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        // Final flush before shutdown
                        debug!("Auth tracking service shutting down, performing final flush");
                        if let Err(e) = self.flush().await {
                            error!("Failed to flush auth tracking during shutdown: {}", e);
                        }
                        break;
                    }
                    _ = flush_interval.tick() => {
                        if let Err(e) = self.flush().await {
                            error!("Failed to flush auth tracking: {}", e);
                        }
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::{api_keys, users};
    use crate::db::repositories::{ApiKeyRepository, UserRepository};
    use crate::db::test_helpers::setup_test_db;
    use crate::utils::password;
    use std::time::Duration;

    async fn create_test_user(db: &DatabaseConnection) -> users::Model {
        let password_hash = password::hash_password("password").unwrap();
        let user = users::Model {
            id: Uuid::new_v4(),
            username: format!("testuser_{}", Uuid::new_v4()),
            email: format!("test_{}@example.com", Uuid::new_v4()),
            password_hash,
            role: "admin".to_string(),
            is_active: true,
            email_verified: false,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        UserRepository::create(db, &user).await.unwrap()
    }

    async fn create_test_api_key(db: &DatabaseConnection, user_id: Uuid) -> api_keys::Model {
        let key_hash = password::hash_password("test_key_secret").unwrap();
        let api_key = api_keys::Model {
            id: Uuid::new_v4(),
            user_id,
            name: format!("Test Key {}", Uuid::new_v4()),
            key_hash,
            key_prefix: format!("codex_{}", &Uuid::new_v4().to_string()[..8]),
            permissions: serde_json::json!([]),
            is_active: true,
            expires_at: None,
            last_used_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        ApiKeyRepository::create(db, &api_key).await.unwrap()
    }

    #[tokio::test]
    async fn test_record_api_key_used_buffers_updates() {
        let db = setup_test_db().await;
        let service = AuthTrackingService::new(db.clone());

        let user = create_test_user(&db).await;
        let api_key = create_test_api_key(&db, user.id).await;

        // Record usage
        service.record_api_key_used(api_key.id);

        // Should be buffered
        assert_eq!(service.pending_api_key_count(), 1);

        // Should not be in database yet
        let db_key = ApiKeyRepository::get_by_prefix(&db, &api_key.key_prefix)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert!(db_key.last_used_at.is_none());
    }

    #[tokio::test]
    async fn test_record_user_login_buffers_updates() {
        let db = setup_test_db().await;
        let service = AuthTrackingService::new(db.clone());

        let user = create_test_user(&db).await;

        // Record login
        service.record_user_login(user.id);

        // Should be buffered
        assert_eq!(service.pending_user_login_count(), 1);

        // Should not be in database yet
        let db_user = UserRepository::get_by_id(&db, user.id)
            .await
            .unwrap()
            .unwrap();
        assert!(db_user.last_login_at.is_none());
    }

    #[tokio::test]
    async fn test_flush_writes_api_key_to_database() {
        let db = setup_test_db().await;
        let service = AuthTrackingService::new(db.clone());

        let user = create_test_user(&db).await;
        let api_key = create_test_api_key(&db, user.id).await;

        // Record usage
        service.record_api_key_used(api_key.id);

        // Flush
        let (api_key_count, user_login_count) = service.flush().await.unwrap();
        assert_eq!(api_key_count, 1);
        assert_eq!(user_login_count, 0);
        assert_eq!(service.pending_api_key_count(), 0);

        // Should be in database now
        let db_key = ApiKeyRepository::get_by_prefix(&db, &api_key.key_prefix)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert!(db_key.last_used_at.is_some());
    }

    #[tokio::test]
    async fn test_flush_writes_user_login_to_database() {
        let db = setup_test_db().await;
        let service = AuthTrackingService::new(db.clone());

        let user = create_test_user(&db).await;

        // Record login
        service.record_user_login(user.id);

        // Flush
        let (api_key_count, user_login_count) = service.flush().await.unwrap();
        assert_eq!(api_key_count, 0);
        assert_eq!(user_login_count, 1);
        assert_eq!(service.pending_user_login_count(), 0);

        // Should be in database now
        let db_user = UserRepository::get_by_id(&db, user.id)
            .await
            .unwrap()
            .unwrap();
        assert!(db_user.last_login_at.is_some());
    }

    #[tokio::test]
    async fn test_multiple_updates_deduplicated() {
        let db = setup_test_db().await;
        let service = AuthTrackingService::new(db.clone());

        let user = create_test_user(&db).await;
        let api_key = create_test_api_key(&db, user.id).await;

        // Record multiple usages
        service.record_api_key_used(api_key.id);
        service.record_api_key_used(api_key.id);
        service.record_api_key_used(api_key.id);

        // Should only have one entry (deduplicated)
        assert_eq!(service.pending_api_key_count(), 1);

        // Record multiple logins
        service.record_user_login(user.id);
        service.record_user_login(user.id);
        service.record_user_login(user.id);

        // Should only have one entry (deduplicated)
        assert_eq!(service.pending_user_login_count(), 1);
    }

    #[tokio::test]
    async fn test_multiple_keys_and_users() {
        let db = setup_test_db().await;
        let service = AuthTrackingService::new(db.clone());

        let user1 = create_test_user(&db).await;
        let user2 = create_test_user(&db).await;
        let api_key1 = create_test_api_key(&db, user1.id).await;
        let api_key2 = create_test_api_key(&db, user2.id).await;

        // Record various usages
        service.record_api_key_used(api_key1.id);
        service.record_api_key_used(api_key2.id);
        service.record_user_login(user1.id);
        service.record_user_login(user2.id);

        assert_eq!(service.pending_api_key_count(), 2);
        assert_eq!(service.pending_user_login_count(), 2);

        // Flush all
        let (api_key_count, user_login_count) = service.flush().await.unwrap();
        assert_eq!(api_key_count, 2);
        assert_eq!(user_login_count, 2);

        // Verify all updated
        let db_key1 = ApiKeyRepository::get_by_prefix(&db, &api_key1.key_prefix)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let db_key2 = ApiKeyRepository::get_by_prefix(&db, &api_key2.key_prefix)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let db_user1 = UserRepository::get_by_id(&db, user1.id)
            .await
            .unwrap()
            .unwrap();
        let db_user2 = UserRepository::get_by_id(&db, user2.id)
            .await
            .unwrap()
            .unwrap();

        assert!(db_key1.last_used_at.is_some());
        assert!(db_key2.last_used_at.is_some());
        assert!(db_user1.last_login_at.is_some());
        assert!(db_user2.last_login_at.is_some());
    }

    #[tokio::test]
    async fn test_background_flush_graceful_shutdown() {
        let db = setup_test_db().await;
        let service = Arc::new(AuthTrackingService::new(db.clone()));

        let user = create_test_user(&db).await;
        let api_key = create_test_api_key(&db, user.id).await;

        // Start background flush
        let cancel_token = CancellationToken::new();
        let handle = service.clone().start_background_flush(cancel_token.clone());

        // Record usage
        service.record_api_key_used(api_key.id);
        service.record_user_login(user.id);

        // Cancel and wait for shutdown (should trigger final flush)
        cancel_token.cancel();
        tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("Background task should complete")
            .expect("Task should not panic");

        // Verify data was flushed
        let db_key = ApiKeyRepository::get_by_prefix(&db, &api_key.key_prefix)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let db_user = UserRepository::get_by_id(&db, user.id)
            .await
            .unwrap()
            .unwrap();

        assert!(db_key.last_used_at.is_some());
        assert!(db_user.last_login_at.is_some());
    }

    #[tokio::test]
    async fn test_flush_empty_buffers() {
        let db = setup_test_db().await;
        let service = AuthTrackingService::new(db);

        // Flushing empty buffers should succeed
        let (api_key_count, user_login_count) = service.flush().await.unwrap();
        assert_eq!(api_key_count, 0);
        assert_eq!(user_login_count, 0);
    }
}
