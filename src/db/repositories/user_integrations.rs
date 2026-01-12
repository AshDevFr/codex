//! User Integrations Repository
//!
//! Provides CRUD operations for per-user external service connections.
//! Credentials are encrypted at rest using AES-256-GCM.

use crate::db::entities::{user_integrations, user_integrations::Entity as UserIntegrations};
use crate::services::CredentialEncryption;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use sea_orm::*;
use uuid::Uuid;

pub struct UserIntegrationsRepository;

impl UserIntegrationsRepository {
    /// Get all integrations for a user
    pub async fn get_all_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<user_integrations::Model>> {
        let integrations = UserIntegrations::find()
            .filter(user_integrations::Column::UserId.eq(user_id))
            .order_by_asc(user_integrations::Column::IntegrationName)
            .all(db)
            .await?;
        Ok(integrations)
    }

    /// Get enabled integrations for a user
    pub async fn get_enabled_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<user_integrations::Model>> {
        let integrations = UserIntegrations::find()
            .filter(user_integrations::Column::UserId.eq(user_id))
            .filter(user_integrations::Column::Enabled.eq(true))
            .order_by_asc(user_integrations::Column::IntegrationName)
            .all(db)
            .await?;
        Ok(integrations)
    }

    /// Get a user integration by ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<user_integrations::Model>> {
        let integration = UserIntegrations::find_by_id(id).one(db).await?;
        Ok(integration)
    }

    /// Get a user integration by user ID and integration name
    pub async fn get_by_user_and_name(
        db: &DatabaseConnection,
        user_id: Uuid,
        integration_name: &str,
    ) -> Result<Option<user_integrations::Model>> {
        let integration = UserIntegrations::find()
            .filter(user_integrations::Column::UserId.eq(user_id))
            .filter(user_integrations::Column::IntegrationName.eq(integration_name))
            .one(db)
            .await?;
        Ok(integration)
    }

    /// Get all users with a specific integration enabled
    pub async fn get_users_with_integration(
        db: &DatabaseConnection,
        integration_name: &str,
    ) -> Result<Vec<user_integrations::Model>> {
        let integrations = UserIntegrations::find()
            .filter(user_integrations::Column::IntegrationName.eq(integration_name))
            .filter(user_integrations::Column::Enabled.eq(true))
            .all(db)
            .await?;
        Ok(integrations)
    }

    /// Get integrations that need token refresh (token expires soon)
    pub async fn get_needing_refresh(
        db: &DatabaseConnection,
        expires_before: DateTime<Utc>,
    ) -> Result<Vec<user_integrations::Model>> {
        let integrations = UserIntegrations::find()
            .filter(user_integrations::Column::Enabled.eq(true))
            .filter(user_integrations::Column::TokenExpiresAt.is_not_null())
            .filter(user_integrations::Column::TokenExpiresAt.lt(expires_before))
            .all(db)
            .await?;
        Ok(integrations)
    }

    /// Create a new user integration
    pub async fn create(
        db: &DatabaseConnection,
        user_id: Uuid,
        integration_name: &str,
        display_name: Option<String>,
        credentials: &serde_json::Value,
        settings: Option<serde_json::Value>,
        external_user_id: Option<String>,
        external_username: Option<String>,
        token_expires_at: Option<DateTime<Utc>>,
    ) -> Result<user_integrations::Model> {
        let now = Utc::now();

        // Encrypt credentials
        let encryption = CredentialEncryption::global()?;
        let encrypted_credentials = encryption.encrypt_json(credentials)?;

        let integration = user_integrations::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            integration_name: Set(integration_name.to_string()),
            display_name: Set(display_name),
            credentials: Set(encrypted_credentials),
            settings: Set(settings.unwrap_or(serde_json::json!({}))),
            enabled: Set(true),
            last_sync_at: Set(None),
            last_error: Set(None),
            sync_status: Set("idle".to_string()),
            external_user_id: Set(external_user_id),
            external_username: Set(external_username),
            token_expires_at: Set(token_expires_at),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let result = integration.insert(db).await?;
        Ok(result)
    }

    /// Update credentials for an integration (e.g., after token refresh)
    pub async fn update_credentials(
        db: &DatabaseConnection,
        id: Uuid,
        credentials: &serde_json::Value,
        token_expires_at: Option<DateTime<Utc>>,
    ) -> Result<user_integrations::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Integration not found: {}", id))?;

        let encryption = CredentialEncryption::global()?;
        let encrypted_credentials = encryption.encrypt_json(credentials)?;

        let mut active_model: user_integrations::ActiveModel = existing.into();
        active_model.credentials = Set(encrypted_credentials);
        active_model.token_expires_at = Set(token_expires_at);
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Update settings for an integration
    pub async fn update_settings(
        db: &DatabaseConnection,
        id: Uuid,
        settings: serde_json::Value,
    ) -> Result<user_integrations::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Integration not found: {}", id))?;

        let mut active_model: user_integrations::ActiveModel = existing.into();
        active_model.settings = Set(settings);
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Update display name for an integration
    pub async fn update_display_name(
        db: &DatabaseConnection,
        id: Uuid,
        display_name: Option<String>,
    ) -> Result<user_integrations::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Integration not found: {}", id))?;

        let mut active_model: user_integrations::ActiveModel = existing.into();
        active_model.display_name = Set(display_name);
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Enable an integration
    pub async fn enable(db: &DatabaseConnection, id: Uuid) -> Result<user_integrations::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Integration not found: {}", id))?;

        let mut active_model: user_integrations::ActiveModel = existing.into();
        active_model.enabled = Set(true);
        active_model.sync_status = Set("idle".to_string());
        active_model.last_error = Set(None);
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Disable an integration
    pub async fn disable(db: &DatabaseConnection, id: Uuid) -> Result<user_integrations::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Integration not found: {}", id))?;

        let mut active_model: user_integrations::ActiveModel = existing.into();
        active_model.enabled = Set(false);
        active_model.sync_status = Set("idle".to_string());
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Update sync status (e.g., syncing, error, rate_limited)
    pub async fn update_sync_status(
        db: &DatabaseConnection,
        id: Uuid,
        status: &str,
        error_message: Option<String>,
    ) -> Result<user_integrations::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Integration not found: {}", id))?;

        let mut active_model: user_integrations::ActiveModel = existing.into();
        active_model.sync_status = Set(status.to_string());
        active_model.last_error = Set(error_message);
        active_model.updated_at = Set(Utc::now());

        // Update last_sync_at if status is idle (sync completed)
        if status == "idle" {
            active_model.last_sync_at = Set(Some(Utc::now()));
        }

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Update external user info after OAuth
    pub async fn update_external_user(
        db: &DatabaseConnection,
        id: Uuid,
        external_user_id: Option<String>,
        external_username: Option<String>,
    ) -> Result<user_integrations::Model> {
        let existing = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Integration not found: {}", id))?;

        let mut active_model: user_integrations::ActiveModel = existing.into();
        active_model.external_user_id = Set(external_user_id);
        active_model.external_username = Set(external_username);
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Delete a user integration (disconnect)
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = UserIntegrations::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete by user ID and integration name
    pub async fn delete_by_user_and_name(
        db: &DatabaseConnection,
        user_id: Uuid,
        integration_name: &str,
    ) -> Result<bool> {
        let result = UserIntegrations::delete_many()
            .filter(user_integrations::Column::UserId.eq(user_id))
            .filter(user_integrations::Column::IntegrationName.eq(integration_name))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Get decrypted credentials for an integration
    pub async fn get_credentials(db: &DatabaseConnection, id: Uuid) -> Result<serde_json::Value> {
        let integration = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow!("Integration not found: {}", id))?;

        let encryption = CredentialEncryption::global()?;
        let decrypted: serde_json::Value = encryption.decrypt_json(&integration.credentials)?;
        Ok(decrypted)
    }

    /// Check if user has a specific integration connected
    pub async fn is_connected(
        db: &DatabaseConnection,
        user_id: Uuid,
        integration_name: &str,
    ) -> Result<bool> {
        let count = UserIntegrations::find()
            .filter(user_integrations::Column::UserId.eq(user_id))
            .filter(user_integrations::Column::IntegrationName.eq(integration_name))
            .count(db)
            .await?;
        Ok(count > 0)
    }

    /// Count total integrations for a user
    pub async fn count_for_user(db: &DatabaseConnection, user_id: Uuid) -> Result<u64> {
        let count = UserIntegrations::find()
            .filter(user_integrations::Column::UserId.eq(user_id))
            .count(db)
            .await?;
        Ok(count)
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

    async fn create_test_user(db: &DatabaseConnection) -> Uuid {
        use crate::db::entities::users;

        let user = users::ActiveModel {
            id: Set(Uuid::new_v4()),
            username: Set("testuser".to_string()),
            email: Set("test@example.com".to_string()),
            password_hash: Set("hash".to_string()),
            is_admin: Set(false),
            is_active: Set(true),
            email_verified: Set(true),
            permissions: Set(serde_json::json!([])),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            last_login_at: Set(None),
        };

        let result = user.insert(db).await.unwrap();
        result.id
    }

    #[tokio::test]
    async fn test_create_integration() {
        setup_test_encryption_key();
        let db = setup_test_db().await;
        let user_id = create_test_user(&db).await;

        let credentials = serde_json::json!({
            "access_token": "test-token",
            "refresh_token": "refresh-token"
        });

        let integration = UserIntegrationsRepository::create(
            &db,
            user_id,
            "anilist",
            Some("My AniList".to_string()),
            &credentials,
            Some(serde_json::json!({"sync_progress": true})),
            Some("12345".to_string()),
            Some("testuser".to_string()),
            None,
        )
        .await
        .unwrap();

        assert_eq!(integration.user_id, user_id);
        assert_eq!(integration.integration_name, "anilist");
        assert_eq!(integration.display_name, Some("My AniList".to_string()));
        assert!(integration.enabled);
        assert_eq!(integration.sync_status, "idle");
        assert_eq!(integration.external_user_id, Some("12345".to_string()));
    }

    #[tokio::test]
    async fn test_get_by_user_and_name() {
        setup_test_encryption_key();
        let db = setup_test_db().await;
        let user_id = create_test_user(&db).await;

        let credentials = serde_json::json!({"access_token": "test"});
        UserIntegrationsRepository::create(
            &db,
            user_id,
            "anilist",
            None,
            &credentials,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let found = UserIntegrationsRepository::get_by_user_and_name(&db, user_id, "anilist")
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().integration_name, "anilist");

        let not_found =
            UserIntegrationsRepository::get_by_user_and_name(&db, user_id, "myanimelist")
                .await
                .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_get_credentials() {
        setup_test_encryption_key();
        let db = setup_test_db().await;
        let user_id = create_test_user(&db).await;

        let credentials = serde_json::json!({
            "access_token": "secret-token-123",
            "refresh_token": "refresh-456"
        });

        let integration = UserIntegrationsRepository::create(
            &db,
            user_id,
            "anilist",
            None,
            &credentials,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let decrypted = UserIntegrationsRepository::get_credentials(&db, integration.id)
            .await
            .unwrap();

        assert_eq!(decrypted["access_token"], "secret-token-123");
        assert_eq!(decrypted["refresh_token"], "refresh-456");
    }

    #[tokio::test]
    async fn test_update_credentials() {
        setup_test_encryption_key();
        let db = setup_test_db().await;
        let user_id = create_test_user(&db).await;

        let credentials = serde_json::json!({"access_token": "old-token"});
        let integration = UserIntegrationsRepository::create(
            &db,
            user_id,
            "anilist",
            None,
            &credentials,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let new_credentials = serde_json::json!({
            "access_token": "new-token",
            "refresh_token": "new-refresh"
        });
        let expires = Utc::now() + chrono::Duration::hours(1);

        UserIntegrationsRepository::update_credentials(
            &db,
            integration.id,
            &new_credentials,
            Some(expires),
        )
        .await
        .unwrap();

        let decrypted = UserIntegrationsRepository::get_credentials(&db, integration.id)
            .await
            .unwrap();

        assert_eq!(decrypted["access_token"], "new-token");
    }

    #[tokio::test]
    async fn test_enable_disable() {
        setup_test_encryption_key();
        let db = setup_test_db().await;
        let user_id = create_test_user(&db).await;

        let credentials = serde_json::json!({"access_token": "test"});
        let integration = UserIntegrationsRepository::create(
            &db,
            user_id,
            "anilist",
            None,
            &credentials,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        assert!(integration.enabled);

        // Disable
        let disabled = UserIntegrationsRepository::disable(&db, integration.id)
            .await
            .unwrap();
        assert!(!disabled.enabled);

        // Enable
        let enabled = UserIntegrationsRepository::enable(&db, integration.id)
            .await
            .unwrap();
        assert!(enabled.enabled);
    }

    #[tokio::test]
    async fn test_update_sync_status() {
        setup_test_encryption_key();
        let db = setup_test_db().await;
        let user_id = create_test_user(&db).await;

        let credentials = serde_json::json!({"access_token": "test"});
        let integration = UserIntegrationsRepository::create(
            &db,
            user_id,
            "anilist",
            None,
            &credentials,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        // Start syncing
        let syncing =
            UserIntegrationsRepository::update_sync_status(&db, integration.id, "syncing", None)
                .await
                .unwrap();
        assert_eq!(syncing.sync_status, "syncing");
        assert!(syncing.last_sync_at.is_none());

        // Complete sync
        let completed =
            UserIntegrationsRepository::update_sync_status(&db, integration.id, "idle", None)
                .await
                .unwrap();
        assert_eq!(completed.sync_status, "idle");
        assert!(completed.last_sync_at.is_some());

        // Error
        let error = UserIntegrationsRepository::update_sync_status(
            &db,
            integration.id,
            "error",
            Some("Connection failed".to_string()),
        )
        .await
        .unwrap();
        assert_eq!(error.sync_status, "error");
        assert_eq!(error.last_error, Some("Connection failed".to_string()));
    }

    #[tokio::test]
    async fn test_delete() {
        setup_test_encryption_key();
        let db = setup_test_db().await;
        let user_id = create_test_user(&db).await;

        let credentials = serde_json::json!({"access_token": "test"});
        let integration = UserIntegrationsRepository::create(
            &db,
            user_id,
            "anilist",
            None,
            &credentials,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let deleted = UserIntegrationsRepository::delete(&db, integration.id)
            .await
            .unwrap();
        assert!(deleted);

        let found = UserIntegrationsRepository::get_by_id(&db, integration.id)
            .await
            .unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_is_connected() {
        setup_test_encryption_key();
        let db = setup_test_db().await;
        let user_id = create_test_user(&db).await;

        assert!(
            !UserIntegrationsRepository::is_connected(&db, user_id, "anilist")
                .await
                .unwrap()
        );

        let credentials = serde_json::json!({"access_token": "test"});
        UserIntegrationsRepository::create(
            &db,
            user_id,
            "anilist",
            None,
            &credentials,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        assert!(
            UserIntegrationsRepository::is_connected(&db, user_id, "anilist")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_get_all_for_user() {
        setup_test_encryption_key();
        let db = setup_test_db().await;
        let user_id = create_test_user(&db).await;

        let credentials = serde_json::json!({"access_token": "test"});

        UserIntegrationsRepository::create(
            &db,
            user_id,
            "anilist",
            None,
            &credentials,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        UserIntegrationsRepository::create(
            &db,
            user_id,
            "myanimelist",
            None,
            &credentials,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let all = UserIntegrationsRepository::get_all_for_user(&db, user_id)
            .await
            .unwrap();

        assert_eq!(all.len(), 2);
    }
}
