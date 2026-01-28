//! Plugins Repository
//!
//! Provides CRUD operations for external metadata provider plugins.
//! Credentials are encrypted at rest using AES-256-GCM.
//!
//! ## Key Features
//!
//! - Create, read, update, delete plugin configurations
//! - Encrypted credential storage
//! - Health status tracking with failure counting
//! - Filter by scope, enabled status, health status
//!
//! TODO: Remove allow(dead_code) once plugin features are fully implemented

#![allow(dead_code)]

use crate::db::entities::plugins::{self, Entity as Plugins, PluginPermission};
use crate::services::plugin::protocol::PluginScope;
use crate::services::CredentialEncryption;
use anyhow::{anyhow, Result};
use chrono::Utc;
use sea_orm::*;
use uuid::Uuid;

pub struct PluginsRepository;

impl PluginsRepository {
    // =========================================================================
    // Read Operations
    // =========================================================================

    /// Get all plugins
    pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<plugins::Model>> {
        let plugins = Plugins::find()
            .order_by_asc(plugins::Column::Name)
            .all(db)
            .await?;
        Ok(plugins)
    }

    /// Get all enabled plugins
    pub async fn get_enabled(db: &DatabaseConnection) -> Result<Vec<plugins::Model>> {
        let plugins = Plugins::find()
            .filter(plugins::Column::Enabled.eq(true))
            .order_by_asc(plugins::Column::Name)
            .all(db)
            .await?;
        Ok(plugins)
    }

    /// Get a plugin by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<plugins::Model>> {
        let plugin = Plugins::find_by_id(id).one(db).await?;
        Ok(plugin)
    }

    /// Get a plugin by name
    pub async fn get_by_name(
        db: &DatabaseConnection,
        name: &str,
    ) -> Result<Option<plugins::Model>> {
        let plugin = Plugins::find()
            .filter(plugins::Column::Name.eq(name))
            .one(db)
            .await?;
        Ok(plugin)
    }

    /// Get enabled plugins that support a specific scope
    ///
    /// Note: This performs in-memory filtering since JSON array queries vary by database.
    /// For small plugin counts (typical), this is efficient enough.
    pub async fn get_enabled_by_scope(
        db: &DatabaseConnection,
        scope: &PluginScope,
    ) -> Result<Vec<plugins::Model>> {
        let enabled = Self::get_enabled(db).await?;
        let filtered = enabled.into_iter().filter(|p| p.has_scope(scope)).collect();
        Ok(filtered)
    }

    /// Get enabled plugins that support a specific scope AND apply to a specific library
    ///
    /// This filters plugins by:
    /// 1. Enabled status
    /// 2. Scope support
    /// 3. Library filtering (empty library_ids = all libraries, or library must be in the list)
    pub async fn get_enabled_by_scope_and_library(
        db: &DatabaseConnection,
        scope: &PluginScope,
        library_id: Uuid,
    ) -> Result<Vec<plugins::Model>> {
        let enabled = Self::get_enabled(db).await?;
        let filtered = enabled
            .into_iter()
            .filter(|p| p.has_scope(scope) && p.applies_to_library(library_id))
            .collect();
        Ok(filtered)
    }

    /// Get plugins by health status
    pub async fn get_by_health_status(
        db: &DatabaseConnection,
        status: &str,
    ) -> Result<Vec<plugins::Model>> {
        let plugins = Plugins::find()
            .filter(plugins::Column::HealthStatus.eq(status))
            .order_by_asc(plugins::Column::Name)
            .all(db)
            .await?;
        Ok(plugins)
    }

    /// Get plugins that are disabled due to failures (auto-disabled)
    pub async fn get_auto_disabled(db: &DatabaseConnection) -> Result<Vec<plugins::Model>> {
        let plugins = Plugins::find()
            .filter(plugins::Column::Enabled.eq(false))
            .filter(plugins::Column::DisabledReason.is_not_null())
            .order_by_desc(plugins::Column::LastFailureAt)
            .all(db)
            .await?;
        Ok(plugins)
    }

    /// Get plugins by type (system or user)
    pub async fn get_by_type(
        db: &DatabaseConnection,
        plugin_type: &str,
    ) -> Result<Vec<plugins::Model>> {
        let plugins = Plugins::find()
            .filter(plugins::Column::PluginType.eq(plugin_type))
            .order_by_asc(plugins::Column::Name)
            .all(db)
            .await?;
        Ok(plugins)
    }

    /// Get enabled plugins by type
    pub async fn get_enabled_by_type(
        db: &DatabaseConnection,
        plugin_type: &str,
    ) -> Result<Vec<plugins::Model>> {
        let plugins = Plugins::find()
            .filter(plugins::Column::PluginType.eq(plugin_type))
            .filter(plugins::Column::Enabled.eq(true))
            .order_by_asc(plugins::Column::Name)
            .all(db)
            .await?;
        Ok(plugins)
    }

    /// Get all system plugins (admin-configured)
    pub async fn get_system_plugins(db: &DatabaseConnection) -> Result<Vec<plugins::Model>> {
        Self::get_by_type(db, "system").await
    }

    /// Get all user plugins (per-user instances)
    pub async fn get_user_plugins(db: &DatabaseConnection) -> Result<Vec<plugins::Model>> {
        Self::get_by_type(db, "user").await
    }

    // =========================================================================
    // Create Operations
    // =========================================================================

    /// Create a new plugin
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        db: &DatabaseConnection,
        name: &str,
        display_name: &str,
        description: Option<&str>,
        plugin_type: &str,
        command: &str,
        args: Vec<String>,
        env: Vec<(String, String)>,
        working_directory: Option<&str>,
        permissions: Vec<PluginPermission>,
        scopes: Vec<PluginScope>,
        library_ids: Vec<Uuid>,
        credentials: Option<&serde_json::Value>,
        credential_delivery: &str,
        config: Option<serde_json::Value>,
        enabled: bool,
        created_by: Option<Uuid>,
        rate_limit_requests_per_minute: Option<i32>,
    ) -> Result<plugins::Model> {
        let now = Utc::now();

        // Encrypt credentials if provided
        let encrypted_credentials = if let Some(creds) = credentials {
            let encryption = CredentialEncryption::global()?;
            Some(encryption.encrypt_json(creds)?)
        } else {
            None
        };

        // Convert permissions, scopes, and library_ids to JSON
        let permissions_json = serde_json::to_value(&permissions)?;
        let scopes_json = serde_json::to_value(&scopes)?;
        let library_ids_json: serde_json::Value = library_ids
            .iter()
            .map(|id| serde_json::Value::String(id.to_string()))
            .collect();
        let args_json = serde_json::to_value(&args)?;
        let env_json: serde_json::Value = env
            .into_iter()
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect::<serde_json::Map<String, serde_json::Value>>()
            .into();

        let plugin = plugins::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name.to_string()),
            display_name: Set(display_name.to_string()),
            description: Set(description.map(|s| s.to_string())),
            plugin_type: Set(plugin_type.to_string()),
            command: Set(command.to_string()),
            args: Set(args_json),
            env: Set(env_json),
            working_directory: Set(working_directory.map(|s| s.to_string())),
            permissions: Set(permissions_json),
            scopes: Set(scopes_json),
            library_ids: Set(library_ids_json),
            credentials: Set(encrypted_credentials),
            credential_delivery: Set(credential_delivery.to_string()),
            config: Set(config.unwrap_or(serde_json::json!({}))),
            manifest: Set(None),
            enabled: Set(enabled),
            health_status: Set("unknown".to_string()),
            failure_count: Set(0),
            last_failure_at: Set(None),
            last_success_at: Set(None),
            disabled_reason: Set(None),
            rate_limit_requests_per_minute: Set(rate_limit_requests_per_minute),
            created_at: Set(now),
            updated_at: Set(now),
            created_by: Set(created_by),
            updated_by: Set(created_by),
        };

        let result = plugin.insert(db).await?;
        Ok(result)
    }

    // =========================================================================
    // Update Operations
    // =========================================================================

    /// Update a plugin's basic information
    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        display_name: Option<String>,
        description: Option<Option<String>>,
        command: Option<String>,
        args: Option<Vec<String>>,
        env: Option<Vec<(String, String)>>,
        working_directory: Option<Option<String>>,
        permissions: Option<Vec<PluginPermission>>,
        scopes: Option<Vec<PluginScope>>,
        library_ids: Option<Vec<Uuid>>,
        credential_delivery: Option<String>,
        config: Option<serde_json::Value>,
        updated_by: Option<Uuid>,
        rate_limit_requests_per_minute: Option<Option<i32>>,
    ) -> Result<plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Plugin not found: {}", id))?;

        let mut active_model: plugins::ActiveModel = existing.into();
        active_model.updated_at = Set(Utc::now());
        active_model.updated_by = Set(updated_by);

        if let Some(name) = display_name {
            active_model.display_name = Set(name);
        }

        if let Some(desc) = description {
            active_model.description = Set(desc);
        }

        if let Some(cmd) = command {
            active_model.command = Set(cmd);
        }

        if let Some(a) = args {
            active_model.args = Set(serde_json::to_value(&a)?);
        }

        if let Some(e) = env {
            let env_json: serde_json::Value = e
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect::<serde_json::Map<String, serde_json::Value>>()
                .into();
            active_model.env = Set(env_json);
        }

        if let Some(wd) = working_directory {
            active_model.working_directory = Set(wd);
        }

        if let Some(perms) = permissions {
            active_model.permissions = Set(serde_json::to_value(&perms)?);
        }

        if let Some(s) = scopes {
            active_model.scopes = Set(serde_json::to_value(&s)?);
        }

        if let Some(lib_ids) = library_ids {
            let library_ids_json: serde_json::Value = lib_ids
                .iter()
                .map(|id| serde_json::Value::String(id.to_string()))
                .collect();
            active_model.library_ids = Set(library_ids_json);
        }

        if let Some(delivery) = credential_delivery {
            active_model.credential_delivery = Set(delivery);
        }

        if let Some(cfg) = config {
            active_model.config = Set(cfg);
        }

        if let Some(rate_limit) = rate_limit_requests_per_minute {
            active_model.rate_limit_requests_per_minute = Set(rate_limit);
        }

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Update plugin credentials
    pub async fn update_credentials(
        db: &DatabaseConnection,
        id: Uuid,
        credentials: Option<&serde_json::Value>,
        updated_by: Option<Uuid>,
    ) -> Result<plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Plugin not found: {}", id))?;

        let mut active_model: plugins::ActiveModel = existing.into();
        active_model.updated_at = Set(Utc::now());
        active_model.updated_by = Set(updated_by);

        let encrypted = if let Some(creds) = credentials {
            let encryption = CredentialEncryption::global()?;
            Some(encryption.encrypt_json(creds)?)
        } else {
            None
        };
        active_model.credentials = Set(encrypted);

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Update cached manifest from plugin
    pub async fn update_manifest(
        db: &DatabaseConnection,
        id: Uuid,
        manifest: Option<serde_json::Value>,
    ) -> Result<plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Plugin not found: {}", id))?;

        let mut active_model: plugins::ActiveModel = existing.into();
        active_model.manifest = Set(manifest);
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Enable a plugin
    pub async fn enable(
        db: &DatabaseConnection,
        id: Uuid,
        updated_by: Option<Uuid>,
    ) -> Result<plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Plugin not found: {}", id))?;

        let mut active_model: plugins::ActiveModel = existing.into();
        active_model.enabled = Set(true);
        active_model.updated_at = Set(Utc::now());
        active_model.updated_by = Set(updated_by);
        // Reset health status when enabling
        active_model.health_status = Set("unknown".to_string());
        // Clear disabled reason
        active_model.disabled_reason = Set(None);

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Disable a plugin (manual disable by admin)
    pub async fn disable(
        db: &DatabaseConnection,
        id: Uuid,
        updated_by: Option<Uuid>,
    ) -> Result<plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Plugin not found: {}", id))?;

        let mut active_model: plugins::ActiveModel = existing.into();
        active_model.enabled = Set(false);
        active_model.updated_at = Set(Utc::now());
        active_model.updated_by = Set(updated_by);
        active_model.health_status = Set("disabled".to_string());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Auto-disable a plugin due to repeated failures
    pub async fn auto_disable(
        db: &DatabaseConnection,
        id: Uuid,
        reason: &str,
    ) -> Result<plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Plugin not found: {}", id))?;

        let mut active_model: plugins::ActiveModel = existing.into();
        active_model.enabled = Set(false);
        active_model.updated_at = Set(Utc::now());
        active_model.health_status = Set("disabled".to_string());
        active_model.disabled_reason = Set(Some(reason.to_string()));

        let result = active_model.update(db).await?;
        Ok(result)
    }

    // =========================================================================
    // Health Status Operations
    // =========================================================================

    /// Record a successful operation
    pub async fn record_success(db: &DatabaseConnection, id: Uuid) -> Result<plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Plugin not found: {}", id))?;

        let mut active_model: plugins::ActiveModel = existing.into();
        active_model.health_status = Set("healthy".to_string());
        active_model.failure_count = Set(0);
        active_model.last_success_at = Set(Some(Utc::now()));
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Record a failed operation and increment failure count
    pub async fn record_failure(
        db: &DatabaseConnection,
        id: Uuid,
        error_message: Option<&str>,
    ) -> Result<plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Plugin not found: {}", id))?;

        let new_failure_count = existing.failure_count + 1;

        let mut active_model: plugins::ActiveModel = existing.into();
        active_model.health_status = Set("unhealthy".to_string());
        active_model.failure_count = Set(new_failure_count);
        active_model.last_failure_at = Set(Some(Utc::now()));
        active_model.disabled_reason = Set(error_message.map(|s| s.to_string()));
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Reset failure count (e.g., after manual re-enable)
    pub async fn reset_failure_count(
        db: &DatabaseConnection,
        id: Uuid,
        updated_by: Option<Uuid>,
    ) -> Result<plugins::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Plugin not found: {}", id))?;

        let mut active_model: plugins::ActiveModel = existing.into();
        active_model.failure_count = Set(0);
        active_model.health_status = Set("unknown".to_string());
        active_model.disabled_reason = Set(None);
        active_model.updated_at = Set(Utc::now());
        active_model.updated_by = Set(updated_by);

        let result = active_model.update(db).await?;
        Ok(result)
    }

    // =========================================================================
    // Delete Operations
    // =========================================================================

    /// Delete a plugin
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = Plugins::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    // =========================================================================
    // Credential Operations
    // =========================================================================

    /// Get decrypted credentials for a plugin
    pub async fn get_credentials(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<serde_json::Value>> {
        let plugin = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Plugin not found: {}", id))?;

        if let Some(encrypted) = plugin.credentials {
            let encryption = CredentialEncryption::global()?;
            let decrypted: serde_json::Value = encryption.decrypt_json(&encrypted)?;
            Ok(Some(decrypted))
        } else {
            Ok(None)
        }
    }

    /// Check if a plugin has credentials set
    pub fn has_credentials(plugin: &plugins::Model) -> bool {
        plugin.credentials.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::setup_test_db;
    use std::env;

    fn setup_test_encryption_key() {
        // Set a test encryption key if not already set
        if env::var("CODEX_ENCRYPTION_KEY").is_err() {
            env::set_var(
                "CODEX_ENCRYPTION_KEY",
                "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
            );
        }
    }

    #[tokio::test]
    async fn test_create_plugin_basic() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let plugin = PluginsRepository::create(
            &db,
            "test_plugin",
            "Test Plugin",
            Some("A test plugin"),
            "system",
            "node",
            vec!["dist/index.js".to_string()],
            vec![],
            None,
            vec![PluginPermission::MetadataWriteSummary],
            vec![PluginScope::SeriesDetail],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            false,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        assert_eq!(plugin.name, "test_plugin");
        assert_eq!(plugin.display_name, "Test Plugin");
        assert_eq!(plugin.description, Some("A test plugin".to_string()));
        assert_eq!(plugin.plugin_type, "system");
        assert_eq!(plugin.command, "node");
        assert!(!plugin.enabled);
        assert_eq!(plugin.health_status, "unknown");
        assert_eq!(plugin.failure_count, 0);
        assert!(plugin.credentials.is_none());
    }

    #[tokio::test]
    async fn test_create_plugin_with_credentials() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let credentials = serde_json::json!({
            "api_key": "secret-key-123"
        });

        let plugin = PluginsRepository::create(
            &db,
            "test_plugin",
            "Test Plugin",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![],
            vec![], // library_ids - empty means all libraries
            Some(&credentials),
            "env",
            None,
            true,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        assert!(plugin.credentials.is_some());
        assert!(plugin.enabled);

        // Verify credentials can be decrypted
        let decrypted = PluginsRepository::get_credentials(&db, plugin.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(decrypted["api_key"], "secret-key-123");
    }

    #[tokio::test]
    async fn test_get_by_name() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        PluginsRepository::create(
            &db,
            "mangabaka",
            "MangaBaka",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            false,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        let found = PluginsRepository::get_by_name(&db, "mangabaka")
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "mangabaka");

        let not_found = PluginsRepository::get_by_name(&db, "nonexistent")
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_enable_disable() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let plugin = PluginsRepository::create(
            &db,
            "test",
            "Test",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            false,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        assert!(!plugin.enabled);

        // Enable
        let enabled = PluginsRepository::enable(&db, plugin.id, None)
            .await
            .unwrap();
        assert!(enabled.enabled);
        assert_eq!(enabled.health_status, "unknown");

        // Disable
        let disabled = PluginsRepository::disable(&db, plugin.id, None)
            .await
            .unwrap();
        assert!(!disabled.enabled);
        assert_eq!(disabled.health_status, "disabled");
    }

    #[tokio::test]
    async fn test_record_success_and_failure() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let plugin = PluginsRepository::create(
            &db,
            "test",
            "Test",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            true,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        // Record failure
        let failed = PluginsRepository::record_failure(&db, plugin.id, Some("Connection timeout"))
            .await
            .unwrap();
        assert_eq!(failed.failure_count, 1);
        assert_eq!(failed.health_status, "unhealthy");
        assert!(failed.last_failure_at.is_some());

        // Record another failure
        let failed2 = PluginsRepository::record_failure(&db, plugin.id, None)
            .await
            .unwrap();
        assert_eq!(failed2.failure_count, 2);

        // Record success - resets failure count
        let success = PluginsRepository::record_success(&db, plugin.id)
            .await
            .unwrap();
        assert_eq!(success.failure_count, 0);
        assert_eq!(success.health_status, "healthy");
        assert!(success.last_success_at.is_some());
    }

    #[tokio::test]
    async fn test_auto_disable() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let plugin = PluginsRepository::create(
            &db,
            "test",
            "Test",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            true,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        let disabled = PluginsRepository::auto_disable(
            &db,
            plugin.id,
            "Disabled after 3 consecutive failures",
        )
        .await
        .unwrap();

        assert!(!disabled.enabled);
        assert_eq!(disabled.health_status, "disabled");
        assert_eq!(
            disabled.disabled_reason,
            Some("Disabled after 3 consecutive failures".to_string())
        );

        // Check get_auto_disabled
        let auto_disabled = PluginsRepository::get_auto_disabled(&db).await.unwrap();
        assert_eq!(auto_disabled.len(), 1);
        assert_eq!(auto_disabled[0].id, plugin.id);
    }

    #[tokio::test]
    async fn test_reset_failure_count() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let plugin = PluginsRepository::create(
            &db,
            "test",
            "Test",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            true,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        // Record some failures
        PluginsRepository::record_failure(&db, plugin.id, None)
            .await
            .unwrap();
        PluginsRepository::record_failure(&db, plugin.id, None)
            .await
            .unwrap();

        let failed = PluginsRepository::get_by_id(&db, plugin.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(failed.failure_count, 2);

        // Reset
        let reset = PluginsRepository::reset_failure_count(&db, plugin.id, None)
            .await
            .unwrap();
        assert_eq!(reset.failure_count, 0);
        assert_eq!(reset.health_status, "unknown");
        assert!(reset.disabled_reason.is_none());
    }

    #[tokio::test]
    async fn test_update_credentials() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let plugin = PluginsRepository::create(
            &db,
            "test",
            "Test",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![],
            vec![], // library_ids - empty means all libraries
            Some(&serde_json::json!({"key": "original"})),
            "env",
            None,
            false,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        // Update credentials
        let new_creds = serde_json::json!({"key": "updated"});
        PluginsRepository::update_credentials(&db, plugin.id, Some(&new_creds), None)
            .await
            .unwrap();

        let decrypted = PluginsRepository::get_credentials(&db, plugin.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(decrypted["key"], "updated");

        // Clear credentials
        PluginsRepository::update_credentials(&db, plugin.id, None, None)
            .await
            .unwrap();

        let cleared = PluginsRepository::get_credentials(&db, plugin.id)
            .await
            .unwrap();
        assert!(cleared.is_none());
    }

    #[tokio::test]
    async fn test_update_manifest() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let plugin = PluginsRepository::create(
            &db,
            "test",
            "Test",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            false,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        assert!(plugin.manifest.is_none());

        let manifest = serde_json::json!({
            "name": "test",
            "version": "1.0.0"
        });

        let updated = PluginsRepository::update_manifest(&db, plugin.id, Some(manifest.clone()))
            .await
            .unwrap();

        assert!(updated.manifest.is_some());
        assert_eq!(updated.manifest.unwrap()["version"], "1.0.0");
    }

    #[tokio::test]
    async fn test_delete() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let plugin = PluginsRepository::create(
            &db,
            "test",
            "Test",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            false,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        let deleted = PluginsRepository::delete(&db, plugin.id).await.unwrap();
        assert!(deleted);

        let found = PluginsRepository::get_by_id(&db, plugin.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_get_enabled() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        PluginsRepository::create(
            &db,
            "enabled1",
            "Enabled 1",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            true,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        PluginsRepository::create(
            &db,
            "disabled1",
            "Disabled 1",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            false,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        let enabled = PluginsRepository::get_enabled(&db).await.unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "enabled1");
    }

    #[tokio::test]
    async fn test_get_enabled_by_scope() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        PluginsRepository::create(
            &db,
            "series_plugin",
            "Series Plugin",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![PluginScope::SeriesDetail, PluginScope::SeriesBulk],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            true,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        PluginsRepository::create(
            &db,
            "library_plugin",
            "Library Plugin",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![PluginScope::LibraryDetail],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            true,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        let series_plugins =
            PluginsRepository::get_enabled_by_scope(&db, &PluginScope::SeriesDetail)
                .await
                .unwrap();
        assert_eq!(series_plugins.len(), 1);
        assert_eq!(series_plugins[0].name, "series_plugin");

        let library_plugins =
            PluginsRepository::get_enabled_by_scope(&db, &PluginScope::LibraryDetail)
                .await
                .unwrap();
        assert_eq!(library_plugins.len(), 1);
        assert_eq!(library_plugins[0].name, "library_plugin");

        let bulk_plugins = PluginsRepository::get_enabled_by_scope(&db, &PluginScope::SeriesBulk)
            .await
            .unwrap();
        assert_eq!(bulk_plugins.len(), 1);
    }

    #[tokio::test]
    async fn test_permissions_and_scopes() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let plugin = PluginsRepository::create(
            &db,
            "test",
            "Test",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![
                PluginPermission::MetadataWriteSummary,
                PluginPermission::MetadataWriteGenres,
            ],
            vec![PluginScope::SeriesDetail],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            false,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        // Test permissions parsing
        let permissions = plugin.permissions_vec();
        assert_eq!(permissions.len(), 2);
        assert!(permissions.contains(&PluginPermission::MetadataWriteSummary));
        assert!(permissions.contains(&PluginPermission::MetadataWriteGenres));

        // Test has_permission
        assert!(plugin.has_permission(&PluginPermission::MetadataWriteSummary));
        assert!(!plugin.has_permission(&PluginPermission::MetadataWriteTitle));

        // Test scopes parsing
        let scopes = plugin.scopes_vec();
        assert_eq!(scopes.len(), 1);
        assert!(scopes.contains(&PluginScope::SeriesDetail));

        // Test has_scope
        assert!(plugin.has_scope(&PluginScope::SeriesDetail));
        assert!(!plugin.has_scope(&PluginScope::LibraryDetail));
    }

    #[tokio::test]
    async fn test_wildcard_permission() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let plugin = PluginsRepository::create(
            &db,
            "test",
            "Test",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![PluginPermission::MetadataWriteAll],
            vec![],
            vec![], // library_ids - empty means all libraries
            None,
            "env",
            None,
            false,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        // Wildcard should grant all write permissions
        assert!(plugin.has_permission(&PluginPermission::MetadataWriteTitle));
        assert!(plugin.has_permission(&PluginPermission::MetadataWriteSummary));
        assert!(plugin.has_permission(&PluginPermission::MetadataWriteGenres));
        assert!(plugin.has_permission(&PluginPermission::MetadataWriteTags));

        // But not read permissions or library permissions
        assert!(!plugin.has_permission(&PluginPermission::MetadataRead));
        assert!(!plugin.has_permission(&PluginPermission::LibraryRead));
    }

    #[tokio::test]
    async fn test_get_enabled_by_scope_and_library() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let manga_library_id = Uuid::new_v4();
        let comics_library_id = Uuid::new_v4();

        // Create a plugin that applies to all libraries
        PluginsRepository::create(
            &db,
            "all_libraries_plugin",
            "All Libraries Plugin",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![PluginScope::SeriesDetail],
            vec![], // empty = all libraries
            None,
            "env",
            None,
            true,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        // Create a plugin that only applies to manga library
        PluginsRepository::create(
            &db,
            "manga_only_plugin",
            "Manga Only Plugin",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![PluginScope::SeriesDetail],
            vec![manga_library_id], // only manga library
            None,
            "env",
            None,
            true,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        // Create a plugin that applies to both manga and comics
        PluginsRepository::create(
            &db,
            "manga_comics_plugin",
            "Manga & Comics Plugin",
            None,
            "system",
            "node",
            vec![],
            vec![],
            None,
            vec![],
            vec![PluginScope::SeriesDetail],
            vec![manga_library_id, comics_library_id],
            None,
            "env",
            None,
            true,
            None,
            Some(60), // rate_limit_requests_per_minute
        )
        .await
        .unwrap();

        // Query for manga library - should get all 3 plugins
        let manga_plugins = PluginsRepository::get_enabled_by_scope_and_library(
            &db,
            &PluginScope::SeriesDetail,
            manga_library_id,
        )
        .await
        .unwrap();
        assert_eq!(manga_plugins.len(), 3);

        // Query for comics library - should get 2 plugins (all_libraries and manga_comics)
        let comics_plugins = PluginsRepository::get_enabled_by_scope_and_library(
            &db,
            &PluginScope::SeriesDetail,
            comics_library_id,
        )
        .await
        .unwrap();
        assert_eq!(comics_plugins.len(), 2);
        let names: Vec<&str> = comics_plugins.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"all_libraries_plugin"));
        assert!(names.contains(&"manga_comics_plugin"));
        assert!(!names.contains(&"manga_only_plugin"));

        // Query for an unknown library - should only get the all_libraries plugin
        let unknown_library_id = Uuid::new_v4();
        let unknown_plugins = PluginsRepository::get_enabled_by_scope_and_library(
            &db,
            &PluginScope::SeriesDetail,
            unknown_library_id,
        )
        .await
        .unwrap();
        assert_eq!(unknown_plugins.len(), 1);
        assert_eq!(unknown_plugins[0].name, "all_libraries_plugin");
    }
}
