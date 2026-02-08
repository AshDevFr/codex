//! User Plugins Repository
//!
//! Provides CRUD operations for per-user plugin instances.
//! Handles per-user credentials (encrypted), OAuth tokens,
//! configuration overrides, and health status tracking.
//!
//! ## Key Features
//!
//! - Create, read, update, delete user plugin instances
//! - Per-user encrypted credential storage (simple tokens + OAuth)
//! - OAuth token management (store, refresh, clear)
//! - External identity tracking (username, avatar)
//! - Health status and failure tracking per user
//! - Sync timestamp tracking

#![allow(dead_code)]

use crate::db::entities::user_plugins::{self, Entity as UserPlugins};
use crate::services::CredentialEncryption;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use sea_orm::*;
use uuid::Uuid;

pub struct UserPluginsRepository;

impl UserPluginsRepository {
    // =========================================================================
    // Read Operations
    // =========================================================================

    /// Get a user plugin instance by ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<user_plugins::Model>> {
        let instance = UserPlugins::find_by_id(id).one(db).await?;
        Ok(instance)
    }

    /// Get a user's instance of a specific plugin
    pub async fn get_by_user_and_plugin(
        db: &DatabaseConnection,
        user_id: Uuid,
        plugin_id: Uuid,
    ) -> Result<Option<user_plugins::Model>> {
        let instance = UserPlugins::find()
            .filter(user_plugins::Column::UserId.eq(user_id))
            .filter(user_plugins::Column::PluginId.eq(plugin_id))
            .one(db)
            .await?;
        Ok(instance)
    }

    /// Get all enabled plugin instances for a user
    pub async fn get_enabled_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<user_plugins::Model>> {
        let instances = UserPlugins::find()
            .filter(user_plugins::Column::UserId.eq(user_id))
            .filter(user_plugins::Column::Enabled.eq(true))
            .order_by_asc(user_plugins::Column::CreatedAt)
            .all(db)
            .await?;
        Ok(instances)
    }

    /// Get all plugin instances for a user (enabled and disabled)
    pub async fn get_all_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<user_plugins::Model>> {
        let instances = UserPlugins::find()
            .filter(user_plugins::Column::UserId.eq(user_id))
            .order_by_asc(user_plugins::Column::CreatedAt)
            .all(db)
            .await?;
        Ok(instances)
    }

    /// Get all users who have a specific plugin enabled (for broadcast operations)
    pub async fn get_users_with_plugin(
        db: &DatabaseConnection,
        plugin_id: Uuid,
    ) -> Result<Vec<user_plugins::Model>> {
        let instances = UserPlugins::find()
            .filter(user_plugins::Column::PluginId.eq(plugin_id))
            .filter(user_plugins::Column::Enabled.eq(true))
            .all(db)
            .await?;
        Ok(instances)
    }

    /// Count the number of users who have enabled each plugin.
    /// Returns a map from plugin_id to user count (only includes plugins with at least one user).
    pub async fn count_users_per_plugin(
        db: &DatabaseConnection,
    ) -> Result<std::collections::HashMap<Uuid, u64>> {
        use sea_orm::QuerySelect;

        let results: Vec<(Uuid, i64)> = UserPlugins::find()
            .select_only()
            .column(user_plugins::Column::PluginId)
            .column_as(user_plugins::Column::Id.count(), "user_count")
            .group_by(user_plugins::Column::PluginId)
            .into_tuple()
            .all(db)
            .await?;

        Ok(results
            .into_iter()
            .map(|(plugin_id, count)| (plugin_id, count as u64))
            .collect())
    }

    // =========================================================================
    // Create Operations
    // =========================================================================

    /// Create a new user plugin instance (enable plugin for user)
    pub async fn create(
        db: &DatabaseConnection,
        plugin_id: Uuid,
        user_id: Uuid,
    ) -> Result<user_plugins::Model> {
        let now = Utc::now();
        let instance = user_plugins::ActiveModel {
            id: Set(Uuid::new_v4()),
            plugin_id: Set(plugin_id),
            user_id: Set(user_id),
            credentials: Set(None),
            config: Set(serde_json::json!({})),
            oauth_access_token: Set(None),
            oauth_refresh_token: Set(None),
            oauth_expires_at: Set(None),
            oauth_scope: Set(None),
            external_user_id: Set(None),
            external_username: Set(None),
            external_avatar_url: Set(None),
            enabled: Set(true),
            health_status: Set("unknown".to_string()),
            failure_count: Set(0),
            last_failure_at: Set(None),
            last_success_at: Set(None),
            last_sync_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let result = instance.insert(db).await?;
        Ok(result)
    }

    // =========================================================================
    // Update Operations
    // =========================================================================

    /// Update a user plugin instance's configuration
    pub async fn update_config(
        db: &DatabaseConnection,
        id: Uuid,
        config: serde_json::Value,
    ) -> Result<user_plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("User plugin not found: {}", id))?;

        let mut active_model: user_plugins::ActiveModel = existing.into();
        active_model.config = Set(config);
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Enable or disable a user plugin instance
    pub async fn set_enabled(
        db: &DatabaseConnection,
        id: Uuid,
        enabled: bool,
    ) -> Result<user_plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("User plugin not found: {}", id))?;

        let mut active_model: user_plugins::ActiveModel = existing.into();
        active_model.enabled = Set(enabled);
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    // =========================================================================
    // Credential Operations
    // =========================================================================

    /// Store encrypted simple credentials (API keys, tokens)
    pub async fn update_credentials(
        db: &DatabaseConnection,
        id: Uuid,
        credentials: &serde_json::Value,
    ) -> Result<user_plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("User plugin not found: {}", id))?;

        let encryption = CredentialEncryption::global()?;
        let encrypted = encryption.encrypt_json(credentials)?;

        let mut active_model: user_plugins::ActiveModel = existing.into();
        active_model.credentials = Set(Some(encrypted));
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Decrypt and return simple credentials
    pub async fn get_credentials(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<serde_json::Value>> {
        let instance = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("User plugin not found: {}", id))?;

        match instance.credentials {
            Some(encrypted) => {
                let encryption = CredentialEncryption::global()?;
                let decrypted: serde_json::Value = encryption.decrypt_json(&encrypted)?;
                Ok(Some(decrypted))
            }
            None => Ok(None),
        }
    }

    /// Clear credentials
    pub async fn clear_credentials(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<user_plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("User plugin not found: {}", id))?;

        let mut active_model: user_plugins::ActiveModel = existing.into();
        active_model.credentials = Set(None);
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    // =========================================================================
    // OAuth Token Operations
    // =========================================================================

    /// Store encrypted OAuth tokens after successful OAuth flow
    pub async fn update_oauth_tokens(
        db: &DatabaseConnection,
        id: Uuid,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_at: Option<DateTime<Utc>>,
        scope: Option<&str>,
    ) -> Result<user_plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("User plugin not found: {}", id))?;

        let encryption = CredentialEncryption::global()?;
        let encrypted_access = encryption.encrypt_string(access_token)?;
        let encrypted_refresh = match refresh_token {
            Some(rt) => Some(encryption.encrypt_string(rt)?),
            None => None,
        };

        let mut active_model: user_plugins::ActiveModel = existing.into();
        active_model.oauth_access_token = Set(Some(encrypted_access));
        active_model.oauth_refresh_token = Set(encrypted_refresh);
        active_model.oauth_expires_at = Set(expires_at);
        active_model.oauth_scope = Set(scope.map(|s| s.to_string()));
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Decrypt and return OAuth access token
    pub async fn get_oauth_access_token(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<String>> {
        let instance = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("User plugin not found: {}", id))?;

        match instance.oauth_access_token {
            Some(encrypted) => {
                let encryption = CredentialEncryption::global()?;
                let decrypted = encryption.decrypt_string(&encrypted)?;
                Ok(Some(decrypted))
            }
            None => Ok(None),
        }
    }

    /// Decrypt and return OAuth refresh token
    pub async fn get_oauth_refresh_token(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<String>> {
        let instance = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("User plugin not found: {}", id))?;

        match instance.oauth_refresh_token {
            Some(encrypted) => {
                let encryption = CredentialEncryption::global()?;
                let decrypted = encryption.decrypt_string(&encrypted)?;
                Ok(Some(decrypted))
            }
            None => Ok(None),
        }
    }

    /// Clear all OAuth tokens (disconnect)
    pub async fn clear_oauth_tokens(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<user_plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("User plugin not found: {}", id))?;

        let mut active_model: user_plugins::ActiveModel = existing.into();
        active_model.oauth_access_token = Set(None);
        active_model.oauth_refresh_token = Set(None);
        active_model.oauth_expires_at = Set(None);
        active_model.oauth_scope = Set(None);
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    // =========================================================================
    // External Identity Operations
    // =========================================================================

    /// Update external identity info (from external service)
    pub async fn update_external_identity(
        db: &DatabaseConnection,
        id: Uuid,
        external_user_id: Option<&str>,
        username: Option<&str>,
        avatar_url: Option<&str>,
    ) -> Result<user_plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("User plugin not found: {}", id))?;

        let mut active_model: user_plugins::ActiveModel = existing.into();
        active_model.external_user_id = Set(external_user_id.map(|s| s.to_string()));
        active_model.external_username = Set(username.map(|s| s.to_string()));
        active_model.external_avatar_url = Set(avatar_url.map(|s| s.to_string()));
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    // =========================================================================
    // Health Status Operations
    // =========================================================================

    /// Record a successful operation
    pub async fn record_success(db: &DatabaseConnection, id: Uuid) -> Result<user_plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("User plugin not found: {}", id))?;

        let mut active_model: user_plugins::ActiveModel = existing.into();
        active_model.health_status = Set("healthy".to_string());
        active_model.failure_count = Set(0);
        active_model.last_success_at = Set(Some(Utc::now()));
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Record a failure
    pub async fn record_failure(db: &DatabaseConnection, id: Uuid) -> Result<user_plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("User plugin not found: {}", id))?;

        let new_failure_count = existing.failure_count + 1;
        let health_status = if new_failure_count >= 3 {
            "unhealthy"
        } else {
            "degraded"
        };

        let mut active_model: user_plugins::ActiveModel = existing.into();
        active_model.health_status = Set(health_status.to_string());
        active_model.failure_count = Set(new_failure_count);
        active_model.last_failure_at = Set(Some(Utc::now()));
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Record a sync operation
    pub async fn record_sync(db: &DatabaseConnection, id: Uuid) -> Result<user_plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("User plugin not found: {}", id))?;

        let mut active_model: user_plugins::ActiveModel = existing.into();
        active_model.last_sync_at = Set(Some(Utc::now()));
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    // =========================================================================
    // Delete Operations
    // =========================================================================

    /// Delete a user plugin instance (disconnect plugin for user)
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = UserPlugins::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete all plugin instances for a user
    pub async fn delete_by_user_id(db: &DatabaseConnection, user_id: Uuid) -> Result<u64> {
        let result = UserPlugins::delete_many()
            .filter(user_plugins::Column::UserId.eq(user_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Delete all instances of a plugin (when plugin is removed)
    pub async fn delete_by_plugin_id(db: &DatabaseConnection, plugin_id: Uuid) -> Result<u64> {
        let result = UserPlugins::delete_many()
            .filter(user_plugins::Column::PluginId.eq(plugin_id))
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
    use crate::db::repositories::PluginsRepository;
    use crate::db::repositories::UserRepository;
    use crate::db::test_helpers::setup_test_db;

    async fn create_test_user(db: &DatabaseConnection) -> users::Model {
        let user = users::Model {
            id: Uuid::new_v4(),
            username: format!("upuser_{}", Uuid::new_v4()),
            email: format!("up_{}@example.com", Uuid::new_v4()),
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

    #[tokio::test]
    async fn test_create_user_plugin() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let plugin = create_test_plugin(&db).await;

        let instance = UserPluginsRepository::create(&db, plugin.id, user.id)
            .await
            .unwrap();

        assert_eq!(instance.plugin_id, plugin.id);
        assert_eq!(instance.user_id, user.id);
        assert!(instance.enabled);
        assert_eq!(instance.health_status, "unknown");
        assert_eq!(instance.failure_count, 0);
        assert!(instance.credentials.is_none());
        assert!(instance.oauth_access_token.is_none());
    }

    #[tokio::test]
    async fn test_get_by_user_and_plugin() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let plugin = create_test_plugin(&db).await;

        // Should not exist initially
        let not_found = UserPluginsRepository::get_by_user_and_plugin(&db, user.id, plugin.id)
            .await
            .unwrap();
        assert!(not_found.is_none());

        // Create and find
        UserPluginsRepository::create(&db, plugin.id, user.id)
            .await
            .unwrap();

        let found = UserPluginsRepository::get_by_user_and_plugin(&db, user.id, plugin.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.user_id, user.id);
        assert_eq!(found.plugin_id, plugin.id);
    }

    #[tokio::test]
    async fn test_get_enabled_for_user() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let plugin1 = create_test_plugin(&db).await;
        let plugin2 = create_test_plugin(&db).await;

        let instance1 = UserPluginsRepository::create(&db, plugin1.id, user.id)
            .await
            .unwrap();
        UserPluginsRepository::create(&db, plugin2.id, user.id)
            .await
            .unwrap();

        // Disable one
        UserPluginsRepository::set_enabled(&db, instance1.id, false)
            .await
            .unwrap();

        let enabled = UserPluginsRepository::get_enabled_for_user(&db, user.id)
            .await
            .unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].plugin_id, plugin2.id);
    }

    #[tokio::test]
    async fn test_get_all_for_user() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let plugin1 = create_test_plugin(&db).await;
        let plugin2 = create_test_plugin(&db).await;

        UserPluginsRepository::create(&db, plugin1.id, user.id)
            .await
            .unwrap();
        UserPluginsRepository::create(&db, plugin2.id, user.id)
            .await
            .unwrap();

        let all = UserPluginsRepository::get_all_for_user(&db, user.id)
            .await
            .unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_get_users_with_plugin() {
        let db = setup_test_db().await;
        let user1 = create_test_user(&db).await;
        let user2 = create_test_user(&db).await;
        let plugin = create_test_plugin(&db).await;

        UserPluginsRepository::create(&db, plugin.id, user1.id)
            .await
            .unwrap();
        UserPluginsRepository::create(&db, plugin.id, user2.id)
            .await
            .unwrap();

        let users = UserPluginsRepository::get_users_with_plugin(&db, plugin.id)
            .await
            .unwrap();
        assert_eq!(users.len(), 2);
    }

    #[tokio::test]
    async fn test_update_config() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let plugin = create_test_plugin(&db).await;

        let instance = UserPluginsRepository::create(&db, plugin.id, user.id)
            .await
            .unwrap();

        let config = serde_json::json!({"auto_sync": true, "sync_interval": 3600});
        let updated = UserPluginsRepository::update_config(&db, instance.id, config.clone())
            .await
            .unwrap();

        assert_eq!(updated.config, config);
    }

    #[tokio::test]
    async fn test_set_enabled() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let plugin = create_test_plugin(&db).await;

        let instance = UserPluginsRepository::create(&db, plugin.id, user.id)
            .await
            .unwrap();
        assert!(instance.enabled);

        let disabled = UserPluginsRepository::set_enabled(&db, instance.id, false)
            .await
            .unwrap();
        assert!(!disabled.enabled);

        let enabled = UserPluginsRepository::set_enabled(&db, instance.id, true)
            .await
            .unwrap();
        assert!(enabled.enabled);
    }

    #[tokio::test]
    async fn test_update_external_identity() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let plugin = create_test_plugin(&db).await;

        let instance = UserPluginsRepository::create(&db, plugin.id, user.id)
            .await
            .unwrap();

        let updated = UserPluginsRepository::update_external_identity(
            &db,
            instance.id,
            Some("12345"),
            Some("@testuser"),
            Some("https://example.com/avatar.png"),
        )
        .await
        .unwrap();

        assert_eq!(updated.external_user_id.as_deref(), Some("12345"));
        assert_eq!(updated.external_username.as_deref(), Some("@testuser"));
        assert_eq!(
            updated.external_avatar_url.as_deref(),
            Some("https://example.com/avatar.png")
        );
    }

    #[tokio::test]
    async fn test_record_success() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let plugin = create_test_plugin(&db).await;

        let instance = UserPluginsRepository::create(&db, plugin.id, user.id)
            .await
            .unwrap();

        let updated = UserPluginsRepository::record_success(&db, instance.id)
            .await
            .unwrap();

        assert_eq!(updated.health_status, "healthy");
        assert_eq!(updated.failure_count, 0);
        assert!(updated.last_success_at.is_some());
    }

    #[tokio::test]
    async fn test_record_failure_escalation() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let plugin = create_test_plugin(&db).await;

        let instance = UserPluginsRepository::create(&db, plugin.id, user.id)
            .await
            .unwrap();

        // First failure → degraded
        let updated = UserPluginsRepository::record_failure(&db, instance.id)
            .await
            .unwrap();
        assert_eq!(updated.health_status, "degraded");
        assert_eq!(updated.failure_count, 1);

        // Second failure → still degraded
        let updated = UserPluginsRepository::record_failure(&db, instance.id)
            .await
            .unwrap();
        assert_eq!(updated.health_status, "degraded");
        assert_eq!(updated.failure_count, 2);

        // Third failure → unhealthy
        let updated = UserPluginsRepository::record_failure(&db, instance.id)
            .await
            .unwrap();
        assert_eq!(updated.health_status, "unhealthy");
        assert_eq!(updated.failure_count, 3);
    }

    #[tokio::test]
    async fn test_record_sync() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let plugin = create_test_plugin(&db).await;

        let instance = UserPluginsRepository::create(&db, plugin.id, user.id)
            .await
            .unwrap();
        assert!(instance.last_sync_at.is_none());

        let updated = UserPluginsRepository::record_sync(&db, instance.id)
            .await
            .unwrap();
        assert!(updated.last_sync_at.is_some());
    }

    #[tokio::test]
    async fn test_delete() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let plugin = create_test_plugin(&db).await;

        let instance = UserPluginsRepository::create(&db, plugin.id, user.id)
            .await
            .unwrap();

        let deleted = UserPluginsRepository::delete(&db, instance.id)
            .await
            .unwrap();
        assert!(deleted);

        let not_found = UserPluginsRepository::get_by_id(&db, instance.id)
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_delete_by_user_id() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let plugin1 = create_test_plugin(&db).await;
        let plugin2 = create_test_plugin(&db).await;

        UserPluginsRepository::create(&db, plugin1.id, user.id)
            .await
            .unwrap();
        UserPluginsRepository::create(&db, plugin2.id, user.id)
            .await
            .unwrap();

        let deleted_count = UserPluginsRepository::delete_by_user_id(&db, user.id)
            .await
            .unwrap();
        assert_eq!(deleted_count, 2);

        let remaining = UserPluginsRepository::get_all_for_user(&db, user.id)
            .await
            .unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_unique_constraint() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let plugin = create_test_plugin(&db).await;

        // First creation should succeed
        UserPluginsRepository::create(&db, plugin.id, user.id)
            .await
            .unwrap();

        // Second creation for same user+plugin should fail
        let result = UserPluginsRepository::create(&db, plugin.id, user.id).await;
        assert!(result.is_err());
    }
}
