//! System Integrations Repository
//!
//! Provides CRUD operations for app-wide external service connections.
//! Credentials are encrypted at rest using AES-256-GCM.
//!
//! TODO: Remove allow(dead_code) once integration features are implemented

#![allow(dead_code)]

use crate::db::entities::{system_integrations, system_integrations::Entity as SystemIntegrations};
use crate::services::CredentialEncryption;
use anyhow::{anyhow, Result};
use chrono::Utc;
use sea_orm::*;
use uuid::Uuid;

pub struct SystemIntegrationsRepository;

impl SystemIntegrationsRepository {
    /// Get all system integrations
    pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<system_integrations::Model>> {
        let integrations = SystemIntegrations::find()
            .order_by_asc(system_integrations::Column::Name)
            .all(db)
            .await?;
        Ok(integrations)
    }

    /// Get all enabled system integrations
    pub async fn get_enabled(db: &DatabaseConnection) -> Result<Vec<system_integrations::Model>> {
        let integrations = SystemIntegrations::find()
            .filter(system_integrations::Column::Enabled.eq(true))
            .order_by_asc(system_integrations::Column::Name)
            .all(db)
            .await?;
        Ok(integrations)
    }

    /// Get integrations by type
    pub async fn get_by_type(
        db: &DatabaseConnection,
        integration_type: &str,
    ) -> Result<Vec<system_integrations::Model>> {
        let integrations = SystemIntegrations::find()
            .filter(system_integrations::Column::IntegrationType.eq(integration_type))
            .order_by_asc(system_integrations::Column::Name)
            .all(db)
            .await?;
        Ok(integrations)
    }

    /// Get enabled integrations by type
    pub async fn get_enabled_by_type(
        db: &DatabaseConnection,
        integration_type: &str,
    ) -> Result<Vec<system_integrations::Model>> {
        let integrations = SystemIntegrations::find()
            .filter(system_integrations::Column::IntegrationType.eq(integration_type))
            .filter(system_integrations::Column::Enabled.eq(true))
            .order_by_asc(system_integrations::Column::Name)
            .all(db)
            .await?;
        Ok(integrations)
    }

    /// Get a system integration by ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<system_integrations::Model>> {
        let integration = SystemIntegrations::find_by_id(id).one(db).await?;
        Ok(integration)
    }

    /// Get a system integration by name
    pub async fn get_by_name(
        db: &DatabaseConnection,
        name: &str,
    ) -> Result<Option<system_integrations::Model>> {
        let integration = SystemIntegrations::find()
            .filter(system_integrations::Column::Name.eq(name))
            .one(db)
            .await?;
        Ok(integration)
    }

    /// Create a new system integration
    #[allow(clippy::too_many_arguments)] // All fields are required for integration creation - matches database schema
    pub async fn create(
        db: &DatabaseConnection,
        name: &str,
        display_name: &str,
        integration_type: &str,
        credentials: Option<&serde_json::Value>,
        config: Option<serde_json::Value>,
        enabled: bool,
        created_by: Option<Uuid>,
    ) -> Result<system_integrations::Model> {
        let now = Utc::now();

        // Encrypt credentials if provided
        let encrypted_credentials = if let Some(creds) = credentials {
            let encryption = CredentialEncryption::global()?;
            Some(encryption.encrypt_json(creds)?)
        } else {
            None
        };

        let integration = system_integrations::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name.to_string()),
            display_name: Set(display_name.to_string()),
            integration_type: Set(integration_type.to_string()),
            credentials: Set(encrypted_credentials),
            config: Set(config.unwrap_or(serde_json::json!({}))),
            enabled: Set(enabled),
            health_status: Set("unknown".to_string()),
            last_health_check_at: Set(None),
            last_sync_at: Set(None),
            error_message: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            created_by: Set(created_by),
            updated_by: Set(created_by),
        };

        let result = integration.insert(db).await?;
        Ok(result)
    }

    /// Update a system integration
    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        display_name: Option<String>,
        credentials: Option<Option<&serde_json::Value>>, // Some(Some(value)) = set, Some(None) = clear, None = no change
        config: Option<serde_json::Value>,
        updated_by: Option<Uuid>,
    ) -> Result<system_integrations::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Integration not found: {}", id))?;

        let mut active_model: system_integrations::ActiveModel = existing.into();
        active_model.updated_at = Set(Utc::now());
        active_model.updated_by = Set(updated_by);

        if let Some(name) = display_name {
            active_model.display_name = Set(name);
        }

        if let Some(config_value) = config {
            active_model.config = Set(config_value);
        }

        if let Some(creds_option) = credentials {
            let encrypted = if let Some(creds) = creds_option {
                let encryption = CredentialEncryption::global()?;
                Some(encryption.encrypt_json(creds)?)
            } else {
                None
            };
            active_model.credentials = Set(encrypted);
        }

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Enable a system integration
    pub async fn enable(
        db: &DatabaseConnection,
        id: Uuid,
        updated_by: Option<Uuid>,
    ) -> Result<system_integrations::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Integration not found: {}", id))?;

        let mut active_model: system_integrations::ActiveModel = existing.into();
        active_model.enabled = Set(true);
        active_model.updated_at = Set(Utc::now());
        active_model.updated_by = Set(updated_by);
        // Reset health status when enabling
        active_model.health_status = Set("unknown".to_string());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Disable a system integration
    pub async fn disable(
        db: &DatabaseConnection,
        id: Uuid,
        updated_by: Option<Uuid>,
    ) -> Result<system_integrations::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Integration not found: {}", id))?;

        let mut active_model: system_integrations::ActiveModel = existing.into();
        active_model.enabled = Set(false);
        active_model.updated_at = Set(Utc::now());
        active_model.updated_by = Set(updated_by);
        active_model.health_status = Set("disabled".to_string());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Update health status after a health check
    pub async fn update_health_status(
        db: &DatabaseConnection,
        id: Uuid,
        status: &str,
        error_message: Option<String>,
    ) -> Result<system_integrations::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Integration not found: {}", id))?;

        let mut active_model: system_integrations::ActiveModel = existing.into();
        active_model.health_status = Set(status.to_string());
        active_model.last_health_check_at = Set(Some(Utc::now()));
        active_model.error_message = Set(error_message);
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Update last sync timestamp
    pub async fn update_last_sync(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<system_integrations::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Integration not found: {}", id))?;

        let mut active_model: system_integrations::ActiveModel = existing.into();
        active_model.last_sync_at = Set(Some(Utc::now()));
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Delete a system integration
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = SystemIntegrations::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Get decrypted credentials for an integration
    pub async fn get_credentials(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<serde_json::Value>> {
        let integration = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Integration not found: {}", id))?;

        if let Some(encrypted) = integration.credentials {
            let encryption = CredentialEncryption::global()?;
            let decrypted: serde_json::Value = encryption.decrypt_json(&encrypted)?;
            Ok(Some(decrypted))
        } else {
            Ok(None)
        }
    }

    /// Check if an integration has credentials set
    pub fn has_credentials(integration: &system_integrations::Model) -> bool {
        integration.credentials.is_some()
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
    async fn test_create_integration_without_credentials() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let integration = SystemIntegrationsRepository::create(
            &db,
            "test_integration",
            "Test Integration",
            "metadata_provider",
            None,
            Some(serde_json::json!({"rate_limit": 60})),
            false,
            None,
        )
        .await
        .unwrap();

        assert_eq!(integration.name, "test_integration");
        assert_eq!(integration.display_name, "Test Integration");
        assert_eq!(integration.integration_type, "metadata_provider");
        assert!(!integration.enabled);
        assert_eq!(integration.health_status, "unknown");
        assert!(integration.credentials.is_none());
    }

    #[tokio::test]
    async fn test_create_integration_with_credentials() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let credentials = serde_json::json!({
            "api_key": "secret-key-123"
        });

        let integration = SystemIntegrationsRepository::create(
            &db,
            "test_integration",
            "Test Integration",
            "metadata_provider",
            Some(&credentials),
            None,
            true,
            None,
        )
        .await
        .unwrap();

        assert!(integration.credentials.is_some());
        assert!(integration.enabled);

        // Verify credentials can be decrypted
        let decrypted = SystemIntegrationsRepository::get_credentials(&db, integration.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(decrypted["api_key"], "secret-key-123");
    }

    #[tokio::test]
    async fn test_get_by_name() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        SystemIntegrationsRepository::create(
            &db,
            "my_integration",
            "My Integration",
            "notification",
            None,
            None,
            false,
            None,
        )
        .await
        .unwrap();

        let found = SystemIntegrationsRepository::get_by_name(&db, "my_integration")
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "my_integration");

        let not_found = SystemIntegrationsRepository::get_by_name(&db, "nonexistent")
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_get_by_type() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        SystemIntegrationsRepository::create(
            &db,
            "provider1",
            "Provider 1",
            "metadata_provider",
            None,
            None,
            true,
            None,
        )
        .await
        .unwrap();

        SystemIntegrationsRepository::create(
            &db,
            "notifier1",
            "Notifier 1",
            "notification",
            None,
            None,
            true,
            None,
        )
        .await
        .unwrap();

        let providers = SystemIntegrationsRepository::get_by_type(&db, "metadata_provider")
            .await
            .unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "provider1");
    }

    #[tokio::test]
    async fn test_enable_disable() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let integration = SystemIntegrationsRepository::create(
            &db, "test", "Test", "sync", None, None, false, None,
        )
        .await
        .unwrap();

        assert!(!integration.enabled);

        // Enable
        let enabled = SystemIntegrationsRepository::enable(&db, integration.id, None)
            .await
            .unwrap();
        assert!(enabled.enabled);
        assert_eq!(enabled.health_status, "unknown");

        // Disable
        let disabled = SystemIntegrationsRepository::disable(&db, integration.id, None)
            .await
            .unwrap();
        assert!(!disabled.enabled);
        assert_eq!(disabled.health_status, "disabled");
    }

    #[tokio::test]
    async fn test_update_credentials() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let integration = SystemIntegrationsRepository::create(
            &db,
            "test",
            "Test",
            "sync",
            Some(&serde_json::json!({"key": "original"})),
            None,
            false,
            None,
        )
        .await
        .unwrap();

        // Update credentials
        let new_creds = serde_json::json!({"key": "updated"});
        SystemIntegrationsRepository::update(
            &db,
            integration.id,
            None,
            Some(Some(&new_creds)),
            None,
            None,
        )
        .await
        .unwrap();

        let decrypted = SystemIntegrationsRepository::get_credentials(&db, integration.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(decrypted["key"], "updated");

        // Clear credentials
        SystemIntegrationsRepository::update(&db, integration.id, None, Some(None), None, None)
            .await
            .unwrap();

        let cleared = SystemIntegrationsRepository::get_credentials(&db, integration.id)
            .await
            .unwrap();
        assert!(cleared.is_none());
    }

    #[tokio::test]
    async fn test_update_health_status() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let integration = SystemIntegrationsRepository::create(
            &db, "test", "Test", "sync", None, None, true, None,
        )
        .await
        .unwrap();

        // Update to healthy
        let healthy = SystemIntegrationsRepository::update_health_status(
            &db,
            integration.id,
            "healthy",
            None,
        )
        .await
        .unwrap();
        assert_eq!(healthy.health_status, "healthy");
        assert!(healthy.last_health_check_at.is_some());
        assert!(healthy.error_message.is_none());

        // Update to unhealthy with error
        let unhealthy = SystemIntegrationsRepository::update_health_status(
            &db,
            integration.id,
            "unhealthy",
            Some("Connection failed".to_string()),
        )
        .await
        .unwrap();
        assert_eq!(unhealthy.health_status, "unhealthy");
        assert_eq!(
            unhealthy.error_message,
            Some("Connection failed".to_string())
        );
    }

    #[tokio::test]
    async fn test_delete() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        let integration = SystemIntegrationsRepository::create(
            &db, "test", "Test", "sync", None, None, false, None,
        )
        .await
        .unwrap();

        let deleted = SystemIntegrationsRepository::delete(&db, integration.id)
            .await
            .unwrap();
        assert!(deleted);

        let found = SystemIntegrationsRepository::get_by_id(&db, integration.id)
            .await
            .unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_get_enabled() {
        setup_test_encryption_key();
        let db = setup_test_db().await;

        SystemIntegrationsRepository::create(
            &db,
            "enabled1",
            "Enabled 1",
            "sync",
            None,
            None,
            true,
            None,
        )
        .await
        .unwrap();

        SystemIntegrationsRepository::create(
            &db,
            "disabled1",
            "Disabled 1",
            "sync",
            None,
            None,
            false,
            None,
        )
        .await
        .unwrap();

        let enabled = SystemIntegrationsRepository::get_enabled(&db)
            .await
            .unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "enabled1");
    }
}
