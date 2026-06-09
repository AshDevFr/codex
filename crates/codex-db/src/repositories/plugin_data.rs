//! Plugin Data Repository (system-scoped)
//!
//! Key-value storage operations for system plugins, keyed by `plugin_id`.
//! The per-user counterpart is [`super::user_plugin_data`]; this one exists
//! so plugins with no user context (e.g. release sources) get a durable KV
//! bucket — used, for example, to persist a release feed cursor.

#![allow(dead_code)]

use crate::entities::plugin_data::{self, Entity as PluginData};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::*;
use uuid::Uuid;

pub struct PluginDataRepository;

impl PluginDataRepository {
    // =========================================================================
    // Read Operations
    // =========================================================================

    /// Get a value by key for a plugin. Returns `None` if the key doesn't
    /// exist or the entry has expired (expired entries are auto-deleted).
    pub async fn get(
        db: &DatabaseConnection,
        plugin_id: Uuid,
        key: &str,
    ) -> Result<Option<plugin_data::Model>> {
        let entry = PluginData::find()
            .filter(plugin_data::Column::PluginId.eq(plugin_id))
            .filter(plugin_data::Column::Key.eq(key))
            .one(db)
            .await?;

        match entry {
            Some(e) if e.is_expired() => {
                PluginData::delete_by_id(e.id).exec(db).await?;
                Ok(None)
            }
            other => Ok(other),
        }
    }

    /// List all (non-expired) keys for a plugin.
    pub async fn list_keys(
        db: &DatabaseConnection,
        plugin_id: Uuid,
    ) -> Result<Vec<plugin_data::Model>> {
        let entries = PluginData::find()
            .filter(plugin_data::Column::PluginId.eq(plugin_id))
            .filter(
                Condition::any()
                    .add(plugin_data::Column::ExpiresAt.is_null())
                    .add(plugin_data::Column::ExpiresAt.gt(Utc::now())),
            )
            .order_by_asc(plugin_data::Column::Key)
            .all(db)
            .await?;
        Ok(entries)
    }

    // =========================================================================
    // Write Operations
    // =========================================================================

    /// Set a value by key (upsert — creates or updates).
    pub async fn set(
        db: &DatabaseConnection,
        plugin_id: Uuid,
        key: &str,
        data: serde_json::Value,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<plugin_data::Model> {
        let now = Utc::now();

        let existing = PluginData::find()
            .filter(plugin_data::Column::PluginId.eq(plugin_id))
            .filter(plugin_data::Column::Key.eq(key))
            .one(db)
            .await?;

        match existing {
            Some(entry) => {
                let mut active_model: plugin_data::ActiveModel = entry.into();
                active_model.data = Set(data);
                active_model.expires_at = Set(expires_at);
                active_model.updated_at = Set(now);
                Ok(active_model.update(db).await?)
            }
            None => {
                let entry = plugin_data::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    plugin_id: Set(plugin_id),
                    key: Set(key.to_string()),
                    data: Set(data),
                    expires_at: Set(expires_at),
                    created_at: Set(now),
                    updated_at: Set(now),
                };
                Ok(entry.insert(db).await?)
            }
        }
    }

    // =========================================================================
    // Delete Operations
    // =========================================================================

    /// Delete a value by key. Returns true if the key existed.
    pub async fn delete(db: &DatabaseConnection, plugin_id: Uuid, key: &str) -> Result<bool> {
        let result = PluginData::delete_many()
            .filter(plugin_data::Column::PluginId.eq(plugin_id))
            .filter(plugin_data::Column::Key.eq(key))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Clear all data for a plugin. Returns the number of entries deleted.
    pub async fn clear_all(db: &DatabaseConnection, plugin_id: Uuid) -> Result<u64> {
        let result = PluginData::delete_many()
            .filter(plugin_data::Column::PluginId.eq(plugin_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Cleanup expired data across all plugins. Intended for a background task.
    /// Returns the number of expired entries deleted.
    pub async fn cleanup_expired(db: &DatabaseConnection) -> Result<u64> {
        let result = PluginData::delete_many()
            .filter(plugin_data::Column::ExpiresAt.is_not_null())
            .filter(plugin_data::Column::ExpiresAt.lte(Utc::now()))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::PluginsRepository;
    use crate::test_helpers::setup_test_db;
    use chrono::Duration;

    /// Create a system plugin row and return its id.
    async fn make_plugin(db: &DatabaseConnection, name: &str) -> Uuid {
        let plugin = PluginsRepository::create(
            db,
            name,
            name,
            None,
            "system",
            "node",
            vec!["x".to_string()],
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
        )
        .await
        .expect("create plugin");
        plugin.id
    }

    #[tokio::test]
    async fn set_get_roundtrip() {
        let db = setup_test_db().await;
        let conn = &db;
        let plugin_id = make_plugin(conn, "release-x").await;

        PluginDataRepository::set(
            conn,
            plugin_id,
            "feed_cursor",
            serde_json::json!("abc"),
            None,
        )
        .await
        .unwrap();
        let got = PluginDataRepository::get(conn, plugin_id, "feed_cursor")
            .await
            .unwrap()
            .expect("entry");
        assert_eq!(got.data, serde_json::json!("abc"));
    }

    #[tokio::test]
    async fn set_upserts_in_place() {
        let db = setup_test_db().await;
        let conn = &db;
        let plugin_id = make_plugin(conn, "release-x").await;

        PluginDataRepository::set(conn, plugin_id, "k", serde_json::json!(1), None)
            .await
            .unwrap();
        PluginDataRepository::set(conn, plugin_id, "k", serde_json::json!(2), None)
            .await
            .unwrap();
        let keys = PluginDataRepository::list_keys(conn, plugin_id)
            .await
            .unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].data, serde_json::json!(2));
    }

    #[tokio::test]
    async fn data_is_isolated_per_plugin() {
        let db = setup_test_db().await;
        let conn = &db;
        let a = make_plugin(conn, "plugin-a").await;
        let b = make_plugin(conn, "plugin-b").await;

        PluginDataRepository::set(conn, a, "k", serde_json::json!("a"), None)
            .await
            .unwrap();
        assert!(
            PluginDataRepository::get(conn, b, "k")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn expired_entries_are_hidden_and_cleaned() {
        let db = setup_test_db().await;
        let conn = &db;
        let plugin_id = make_plugin(conn, "release-x").await;

        let past = Utc::now() - Duration::hours(1);
        PluginDataRepository::set(conn, plugin_id, "k", serde_json::json!(1), Some(past))
            .await
            .unwrap();

        // Hidden on read (and auto-deleted)...
        assert!(
            PluginDataRepository::get(conn, plugin_id, "k")
                .await
                .unwrap()
                .is_none()
        );
        // ...and counted by the cleanup sweep when present.
        PluginDataRepository::set(conn, plugin_id, "k2", serde_json::json!(1), Some(past))
            .await
            .unwrap();
        let removed = PluginDataRepository::cleanup_expired(conn).await.unwrap();
        assert!(removed >= 1);
    }

    #[tokio::test]
    async fn delete_and_clear() {
        let db = setup_test_db().await;
        let conn = &db;
        let plugin_id = make_plugin(conn, "release-x").await;

        PluginDataRepository::set(conn, plugin_id, "k1", serde_json::json!(1), None)
            .await
            .unwrap();
        PluginDataRepository::set(conn, plugin_id, "k2", serde_json::json!(2), None)
            .await
            .unwrap();

        assert!(
            PluginDataRepository::delete(conn, plugin_id, "k1")
                .await
                .unwrap()
        );
        assert!(
            !PluginDataRepository::delete(conn, plugin_id, "missing")
                .await
                .unwrap()
        );

        let cleared = PluginDataRepository::clear_all(conn, plugin_id)
            .await
            .unwrap();
        assert_eq!(cleared, 1);
    }
}
