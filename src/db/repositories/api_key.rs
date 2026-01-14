//! Repository for API key operations
//!
//! TODO: Remove allow(dead_code) when API key management is fully integrated

#![allow(dead_code)]

use crate::db::entities::{api_keys, api_keys::Entity as ApiKey};
use anyhow::Result;
use chrono::Utc;
use sea_orm::*;
use uuid::Uuid;

pub struct ApiKeyRepository;

impl ApiKeyRepository {
    /// Create a new API key
    pub async fn create(
        db: &DatabaseConnection,
        model: &api_keys::Model,
    ) -> Result<api_keys::Model> {
        let active_model = api_keys::ActiveModel {
            id: Set(model.id),
            user_id: Set(model.user_id),
            name: Set(model.name.clone()),
            key_hash: Set(model.key_hash.clone()),
            key_prefix: Set(model.key_prefix.clone()),
            permissions: Set(model.permissions.clone()),
            is_active: Set(model.is_active),
            expires_at: Set(model.expires_at),
            last_used_at: Set(model.last_used_at),
            created_at: Set(model.created_at),
            updated_at: Set(model.updated_at),
        };

        let result = active_model.insert(db).await?;
        Ok(result)
    }

    /// Get API key by hash
    pub async fn get_by_hash(
        db: &DatabaseConnection,
        key_hash: &str,
    ) -> Result<Option<api_keys::Model>> {
        let key = ApiKey::find()
            .filter(api_keys::Column::KeyHash.eq(key_hash))
            .filter(api_keys::Column::IsActive.eq(true))
            .one(db)
            .await?;
        Ok(key)
    }

    /// Get API keys by prefix (for authentication lookup)
    pub async fn get_by_prefix(
        db: &DatabaseConnection,
        key_prefix: &str,
    ) -> Result<Vec<api_keys::Model>> {
        let keys = ApiKey::find()
            .filter(api_keys::Column::KeyPrefix.eq(key_prefix))
            .filter(api_keys::Column::IsActive.eq(true))
            .all(db)
            .await?;
        Ok(keys)
    }

    /// Get API key by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<api_keys::Model>> {
        let key = ApiKey::find_by_id(id).one(db).await?;
        Ok(key)
    }

    /// List all API keys for a user
    pub async fn list_by_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<api_keys::Model>> {
        let keys = ApiKey::find()
            .filter(api_keys::Column::UserId.eq(user_id))
            .all(db)
            .await?;
        Ok(keys)
    }

    /// Update last used timestamp
    pub async fn update_last_used(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        let key = ApiKey::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("API key not found"))?;

        let mut active_model: api_keys::ActiveModel = key.into();
        active_model.last_used_at = Set(Some(Utc::now()));
        active_model.updated_at = Set(Utc::now());
        active_model.update(db).await?;

        Ok(())
    }

    /// Revoke (deactivate) an API key
    pub async fn revoke(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        let key = ApiKey::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("API key not found"))?;

        let mut active_model: api_keys::ActiveModel = key.into();
        active_model.is_active = Set(false);
        active_model.updated_at = Set(Utc::now());
        active_model.update(db).await?;

        Ok(())
    }

    /// Delete an API key
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        ApiKey::delete_by_id(id).exec(db).await?;
        Ok(())
    }

    /// Update an API key
    pub async fn update(db: &DatabaseConnection, model: &api_keys::Model) -> Result<()> {
        let mut active_model: api_keys::ActiveModel = model.clone().into();
        active_model.updated_at = Set(Utc::now());
        active_model.update(db).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::user::UserRepository;
    use crate::db::{entities::users, test_helpers::setup_test_db};

    async fn create_test_user(db: &DatabaseConnection) -> users::Model {
        let user = users::Model {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash: "hash123".to_string(),
            is_admin: false,
            is_active: true,
            email_verified: false,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        UserRepository::create(db, &user).await.unwrap()
    }

    #[tokio::test]
    async fn test_create_and_get_api_key() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let api_key = api_keys::Model {
            id: Uuid::new_v4(),
            user_id: user.id,
            name: "Test Key".to_string(),
            key_hash: "hash_of_key".to_string(),
            key_prefix: "codex_abc".to_string(),
            permissions: serde_json::json!(["libraries-read"]),
            is_active: true,
            expires_at: None,
            last_used_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let created = ApiKeyRepository::create(&db, &api_key).await.unwrap();
        assert_eq!(created.name, "Test Key");

        let found = ApiKeyRepository::get_by_hash(&db, "hash_of_key")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.name, "Test Key");
    }

    #[tokio::test]
    async fn test_revoke_api_key() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let api_key = api_keys::Model {
            id: Uuid::new_v4(),
            user_id: user.id,
            name: "Revoke Test".to_string(),
            key_hash: "hash_revoke".to_string(),
            key_prefix: "codex_xyz".to_string(),
            permissions: serde_json::json!(["libraries-read"]),
            is_active: true,
            expires_at: None,
            last_used_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let created = ApiKeyRepository::create(&db, &api_key).await.unwrap();
        assert!(created.is_active);

        ApiKeyRepository::revoke(&db, created.id).await.unwrap();

        let revoked = ApiKeyRepository::get_by_id(&db, created.id)
            .await
            .unwrap()
            .unwrap();
        assert!(!revoked.is_active);

        // Should not find by hash when revoked
        let not_found = ApiKeyRepository::get_by_hash(&db, "hash_revoke")
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_list_by_user() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        for i in 0..3 {
            let api_key = api_keys::Model {
                id: Uuid::new_v4(),
                user_id: user.id,
                name: format!("Key {}", i),
                key_hash: format!("hash_{}", i),
                key_prefix: format!("codex_{}", i),
                permissions: serde_json::json!(["libraries-read"]),
                is_active: true,
                expires_at: None,
                last_used_at: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            ApiKeyRepository::create(&db, &api_key).await.unwrap();
        }

        let keys = ApiKeyRepository::list_by_user(&db, user.id).await.unwrap();
        assert_eq!(keys.len(), 3);
    }
}
