//! User Plugin Data Repository
//!
//! Provides key-value storage operations for per-user plugin data.
//! Plugins use this to persist stateful data like taste profiles,
//! sync state, and cached recommendations.
//!
//! ## Key Features
//!
//! - Get/set/delete key-value pairs scoped per user-plugin instance
//! - Optional TTL (time-to-live) for cached data
//! - List all keys for a plugin instance
//! - Clear all data for a plugin instance
//! - Background cleanup of expired data

#![allow(dead_code)]

use crate::db::entities::user_plugin_data::{self, Entity as UserPluginData};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::*;
use uuid::Uuid;

pub struct UserPluginDataRepository;

impl UserPluginDataRepository {
    // =========================================================================
    // Read Operations
    // =========================================================================

    /// Get a value by key for a user plugin instance
    ///
    /// Returns None if the key doesn't exist or if the entry has expired.
    pub async fn get(
        db: &DatabaseConnection,
        user_plugin_id: Uuid,
        key: &str,
    ) -> Result<Option<user_plugin_data::Model>> {
        let entry = UserPluginData::find()
            .filter(user_plugin_data::Column::UserPluginId.eq(user_plugin_id))
            .filter(user_plugin_data::Column::Key.eq(key))
            .one(db)
            .await?;

        // Check if expired
        match entry {
            Some(e) if e.is_expired() => {
                // Auto-delete expired entry
                UserPluginData::delete_by_id(e.id).exec(db).await?;
                Ok(None)
            }
            other => Ok(other),
        }
    }

    /// List all keys for a user plugin instance (excluding expired)
    pub async fn list_keys(
        db: &DatabaseConnection,
        user_plugin_id: Uuid,
    ) -> Result<Vec<user_plugin_data::Model>> {
        let entries = UserPluginData::find()
            .filter(user_plugin_data::Column::UserPluginId.eq(user_plugin_id))
            .filter(
                Condition::any()
                    .add(user_plugin_data::Column::ExpiresAt.is_null())
                    .add(user_plugin_data::Column::ExpiresAt.gt(Utc::now())),
            )
            .order_by_asc(user_plugin_data::Column::Key)
            .all(db)
            .await?;
        Ok(entries)
    }

    // =========================================================================
    // Write Operations
    // =========================================================================

    /// Set a value by key (upsert - creates or updates)
    pub async fn set(
        db: &DatabaseConnection,
        user_plugin_id: Uuid,
        key: &str,
        data: serde_json::Value,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<user_plugin_data::Model> {
        let now = Utc::now();

        // Check if key already exists
        let existing = UserPluginData::find()
            .filter(user_plugin_data::Column::UserPluginId.eq(user_plugin_id))
            .filter(user_plugin_data::Column::Key.eq(key))
            .one(db)
            .await?;

        match existing {
            Some(entry) => {
                // Update existing entry
                let mut active_model: user_plugin_data::ActiveModel = entry.into();
                active_model.data = Set(data);
                active_model.expires_at = Set(expires_at);
                active_model.updated_at = Set(now);

                let result = active_model.update(db).await?;
                Ok(result)
            }
            None => {
                // Create new entry
                let entry = user_plugin_data::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    user_plugin_id: Set(user_plugin_id),
                    key: Set(key.to_string()),
                    data: Set(data),
                    expires_at: Set(expires_at),
                    created_at: Set(now),
                    updated_at: Set(now),
                };

                let result = entry.insert(db).await?;
                Ok(result)
            }
        }
    }

    // =========================================================================
    // Delete Operations
    // =========================================================================

    /// Delete a value by key
    ///
    /// Returns true if the key existed and was deleted.
    pub async fn delete(db: &DatabaseConnection, user_plugin_id: Uuid, key: &str) -> Result<bool> {
        let result = UserPluginData::delete_many()
            .filter(user_plugin_data::Column::UserPluginId.eq(user_plugin_id))
            .filter(user_plugin_data::Column::Key.eq(key))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Clear all data for a user plugin instance
    ///
    /// Returns the number of entries deleted.
    pub async fn clear_all(db: &DatabaseConnection, user_plugin_id: Uuid) -> Result<u64> {
        let result = UserPluginData::delete_many()
            .filter(user_plugin_data::Column::UserPluginId.eq(user_plugin_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Cleanup expired data across all user plugins
    ///
    /// This is intended to be called periodically by a background task.
    /// Returns the number of expired entries deleted.
    pub async fn cleanup_expired(db: &DatabaseConnection) -> Result<u64> {
        let result = UserPluginData::delete_many()
            .filter(user_plugin_data::Column::ExpiresAt.is_not_null())
            .filter(user_plugin_data::Column::ExpiresAt.lte(Utc::now()))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::plugins;
    use crate::db::entities::users;
    use crate::db::repositories::{PluginsRepository, UserPluginsRepository, UserRepository};
    use crate::db::test_helpers::setup_test_db;
    use chrono::Duration;

    async fn create_test_user(db: &DatabaseConnection) -> users::Model {
        let user = users::Model {
            id: Uuid::new_v4(),
            username: format!("upduser_{}", Uuid::new_v4()),
            email: format!("upd_{}@example.com", Uuid::new_v4()),
            password_hash: "hash123".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: false,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        UserRepository::create(db, &user).await.unwrap()
    }

    async fn create_test_plugin(db: &DatabaseConnection) -> plugins::Model {
        PluginsRepository::create(
            db,
            &format!("test_plugin_{}", Uuid::new_v4()),
            "Test Plugin",
            Some("A test user plugin"),
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
        )
        .await
        .unwrap()
    }

    async fn create_test_user_plugin(
        db: &DatabaseConnection,
    ) -> (
        users::Model,
        plugins::Model,
        crate::db::entities::user_plugins::Model,
    ) {
        let user = create_test_user(db).await;
        let plugin = create_test_plugin(db).await;
        let user_plugin = UserPluginsRepository::create(db, plugin.id, user.id)
            .await
            .unwrap();
        (user, plugin, user_plugin)
    }

    #[tokio::test]
    async fn test_set_and_get() {
        let db = setup_test_db().await;
        let (_, _, user_plugin) = create_test_user_plugin(&db).await;

        let data = serde_json::json!({"score": 0.95, "genres": ["action", "drama"]});
        UserPluginDataRepository::set(&db, user_plugin.id, "taste_profile", data.clone(), None)
            .await
            .unwrap();

        let entry = UserPluginDataRepository::get(&db, user_plugin.id, "taste_profile")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(entry.key, "taste_profile");
        assert_eq!(entry.data, data);
        assert!(entry.expires_at.is_none());
    }

    #[tokio::test]
    async fn test_set_upsert() {
        let db = setup_test_db().await;
        let (_, _, user_plugin) = create_test_user_plugin(&db).await;

        // Set initial value
        let data1 = serde_json::json!({"version": 1});
        UserPluginDataRepository::set(&db, user_plugin.id, "sync_state", data1, None)
            .await
            .unwrap();

        // Upsert with new value
        let data2 = serde_json::json!({"version": 2});
        UserPluginDataRepository::set(&db, user_plugin.id, "sync_state", data2.clone(), None)
            .await
            .unwrap();

        let entry = UserPluginDataRepository::get(&db, user_plugin.id, "sync_state")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(entry.data, data2);
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let db = setup_test_db().await;
        let (_, _, user_plugin) = create_test_user_plugin(&db).await;

        let entry = UserPluginDataRepository::get(&db, user_plugin.id, "nonexistent")
            .await
            .unwrap();
        assert!(entry.is_none());
    }

    #[tokio::test]
    async fn test_get_expired_auto_deletes() {
        let db = setup_test_db().await;
        let (_, _, user_plugin) = create_test_user_plugin(&db).await;

        // Set a value that has already expired
        let data = serde_json::json!({"cached": true});
        let expired_at = Utc::now() - Duration::hours(1);
        UserPluginDataRepository::set(
            &db,
            user_plugin.id,
            "recommendations",
            data,
            Some(expired_at),
        )
        .await
        .unwrap();

        // Get should return None and auto-delete
        let entry = UserPluginDataRepository::get(&db, user_plugin.id, "recommendations")
            .await
            .unwrap();
        assert!(entry.is_none());
    }

    #[tokio::test]
    async fn test_set_with_ttl() {
        let db = setup_test_db().await;
        let (_, _, user_plugin) = create_test_user_plugin(&db).await;

        let data = serde_json::json!({"recs": [1, 2, 3]});
        let expires_at = Utc::now() + Duration::hours(24);
        UserPluginDataRepository::set(
            &db,
            user_plugin.id,
            "recommendations",
            data.clone(),
            Some(expires_at),
        )
        .await
        .unwrap();

        let entry = UserPluginDataRepository::get(&db, user_plugin.id, "recommendations")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(entry.data, data);
        assert!(entry.expires_at.is_some());
    }

    #[tokio::test]
    async fn test_list_keys() {
        let db = setup_test_db().await;
        let (_, _, user_plugin) = create_test_user_plugin(&db).await;

        UserPluginDataRepository::set(&db, user_plugin.id, "alpha", serde_json::json!(1), None)
            .await
            .unwrap();
        UserPluginDataRepository::set(&db, user_plugin.id, "beta", serde_json::json!(2), None)
            .await
            .unwrap();
        // Add an expired entry that should be excluded
        UserPluginDataRepository::set(
            &db,
            user_plugin.id,
            "expired",
            serde_json::json!(3),
            Some(Utc::now() - Duration::hours(1)),
        )
        .await
        .unwrap();

        let keys = UserPluginDataRepository::list_keys(&db, user_plugin.id)
            .await
            .unwrap();

        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].key, "alpha");
        assert_eq!(keys[1].key, "beta");
    }

    #[tokio::test]
    async fn test_delete_key() {
        let db = setup_test_db().await;
        let (_, _, user_plugin) = create_test_user_plugin(&db).await;

        UserPluginDataRepository::set(&db, user_plugin.id, "to_delete", serde_json::json!(1), None)
            .await
            .unwrap();

        let deleted = UserPluginDataRepository::delete(&db, user_plugin.id, "to_delete")
            .await
            .unwrap();
        assert!(deleted);

        let entry = UserPluginDataRepository::get(&db, user_plugin.id, "to_delete")
            .await
            .unwrap();
        assert!(entry.is_none());

        // Deleting non-existent key returns false
        let deleted = UserPluginDataRepository::delete(&db, user_plugin.id, "nonexistent")
            .await
            .unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_clear_all() {
        let db = setup_test_db().await;
        let (_, _, user_plugin) = create_test_user_plugin(&db).await;

        UserPluginDataRepository::set(&db, user_plugin.id, "key1", serde_json::json!(1), None)
            .await
            .unwrap();
        UserPluginDataRepository::set(&db, user_plugin.id, "key2", serde_json::json!(2), None)
            .await
            .unwrap();
        UserPluginDataRepository::set(&db, user_plugin.id, "key3", serde_json::json!(3), None)
            .await
            .unwrap();

        let count = UserPluginDataRepository::clear_all(&db, user_plugin.id)
            .await
            .unwrap();
        assert_eq!(count, 3);

        let keys = UserPluginDataRepository::list_keys(&db, user_plugin.id)
            .await
            .unwrap();
        assert!(keys.is_empty());
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let db = setup_test_db().await;
        let (_, _, up1) = create_test_user_plugin(&db).await;

        // Create a second user plugin for isolation test
        let user2 = create_test_user(&db).await;
        let plugin2 = create_test_plugin(&db).await;
        let up2 = UserPluginsRepository::create(&db, plugin2.id, user2.id)
            .await
            .unwrap();

        // Set expired entries across both user plugins
        UserPluginDataRepository::set(
            &db,
            up1.id,
            "expired1",
            serde_json::json!(1),
            Some(Utc::now() - Duration::hours(1)),
        )
        .await
        .unwrap();
        UserPluginDataRepository::set(
            &db,
            up2.id,
            "expired2",
            serde_json::json!(2),
            Some(Utc::now() - Duration::hours(2)),
        )
        .await
        .unwrap();
        // Non-expired entry should survive
        UserPluginDataRepository::set(
            &db,
            up1.id,
            "still_valid",
            serde_json::json!(3),
            Some(Utc::now() + Duration::hours(24)),
        )
        .await
        .unwrap();
        // Entry with no expiry should survive
        UserPluginDataRepository::set(&db, up1.id, "permanent", serde_json::json!(4), None)
            .await
            .unwrap();

        let cleaned = UserPluginDataRepository::cleanup_expired(&db)
            .await
            .unwrap();
        assert_eq!(cleaned, 2);

        // Verify remaining entries
        let keys = UserPluginDataRepository::list_keys(&db, up1.id)
            .await
            .unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[tokio::test]
    async fn test_data_isolation_between_user_plugins() {
        let db = setup_test_db().await;
        let (_, _, up1) = create_test_user_plugin(&db).await;

        let user2 = create_test_user(&db).await;
        let plugin2 = create_test_plugin(&db).await;
        let up2 = UserPluginsRepository::create(&db, plugin2.id, user2.id)
            .await
            .unwrap();

        // Set same key in different user plugins
        UserPluginDataRepository::set(
            &db,
            up1.id,
            "shared_key",
            serde_json::json!({"owner": "user1"}),
            None,
        )
        .await
        .unwrap();
        UserPluginDataRepository::set(
            &db,
            up2.id,
            "shared_key",
            serde_json::json!({"owner": "user2"}),
            None,
        )
        .await
        .unwrap();

        // Each should see their own data
        let data1 = UserPluginDataRepository::get(&db, up1.id, "shared_key")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(data1.data, serde_json::json!({"owner": "user1"}));

        let data2 = UserPluginDataRepository::get(&db, up2.id, "shared_key")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(data2.data, serde_json::json!({"owner": "user2"}));
    }
}
