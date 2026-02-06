//! Repository for OIDC connection operations
//!
//! This repository handles CRUD operations and lookups for OIDC connections,
//! which link Codex users to their external identity provider accounts.

use crate::db::entities::oidc_connections::{self, Entity as OidcConnection};
use anyhow::Result;
use chrono::Utc;
use sea_orm::*;
use uuid::Uuid;

pub struct OidcConnectionRepository;

impl OidcConnectionRepository {
    /// Create a new OIDC connection
    pub async fn create(
        db: &DatabaseConnection,
        model: &oidc_connections::Model,
    ) -> Result<oidc_connections::Model> {
        let active_model = oidc_connections::ActiveModel {
            id: Set(model.id),
            user_id: Set(model.user_id),
            provider_name: Set(model.provider_name.clone()),
            subject: Set(model.subject.clone()),
            email: Set(model.email.clone()),
            display_name: Set(model.display_name.clone()),
            groups: Set(model.groups.clone()),
            access_token_hash: Set(model.access_token_hash.clone()),
            refresh_token_encrypted: Set(model.refresh_token_encrypted.clone()),
            token_expires_at: Set(model.token_expires_at),
            created_at: Set(model.created_at),
            updated_at: Set(model.updated_at),
            last_used_at: Set(model.last_used_at),
        };

        let result = active_model.insert(db).await?;
        Ok(result)
    }

    /// Get OIDC connection by ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<oidc_connections::Model>> {
        let connection = OidcConnection::find_by_id(id).one(db).await?;
        Ok(connection)
    }

    /// Find OIDC connection by provider name and subject
    ///
    /// This is the primary lookup method for OIDC authentication.
    /// The (provider_name, subject) pair uniquely identifies a user at an IdP.
    pub async fn find_by_provider_subject(
        db: &DatabaseConnection,
        provider_name: &str,
        subject: &str,
    ) -> Result<Option<oidc_connections::Model>> {
        let connection = OidcConnection::find()
            .filter(oidc_connections::Column::ProviderName.eq(provider_name))
            .filter(oidc_connections::Column::Subject.eq(subject))
            .one(db)
            .await?;
        Ok(connection)
    }

    /// Find all OIDC connections for a user
    pub async fn find_by_user_id(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<oidc_connections::Model>> {
        let connections = OidcConnection::find()
            .filter(oidc_connections::Column::UserId.eq(user_id))
            .order_by_desc(oidc_connections::Column::LastUsedAt)
            .all(db)
            .await?;
        Ok(connections)
    }

    /// Find all OIDC connections for a specific provider
    pub async fn find_by_provider(
        db: &DatabaseConnection,
        provider_name: &str,
    ) -> Result<Vec<oidc_connections::Model>> {
        let connections = OidcConnection::find()
            .filter(oidc_connections::Column::ProviderName.eq(provider_name))
            .all(db)
            .await?;
        Ok(connections)
    }

    /// Update an OIDC connection
    pub async fn update(
        db: &DatabaseConnection,
        model: &oidc_connections::Model,
    ) -> Result<oidc_connections::Model> {
        let mut active_model: oidc_connections::ActiveModel = model.clone().into();
        active_model.updated_at = Set(Utc::now());
        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Update the last_used_at timestamp for an OIDC connection
    pub async fn update_last_used(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        let connection = OidcConnection::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("OIDC connection not found"))?;

        let mut active_model: oidc_connections::ActiveModel = connection.into();
        active_model.last_used_at = Set(Some(Utc::now()));
        active_model.updated_at = Set(Utc::now());
        active_model.update(db).await?;

        Ok(())
    }

    /// Update groups and last_used_at for an OIDC connection
    ///
    /// This is typically called after successful authentication to update
    /// the user's groups from the IdP.
    pub async fn update_groups_and_last_used(
        db: &DatabaseConnection,
        id: Uuid,
        groups: Option<serde_json::Value>,
        email: Option<String>,
        display_name: Option<String>,
    ) -> Result<oidc_connections::Model> {
        let connection = OidcConnection::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("OIDC connection not found"))?;

        let mut active_model: oidc_connections::ActiveModel = connection.into();
        active_model.groups = Set(groups);
        active_model.email = Set(email);
        active_model.display_name = Set(display_name);
        active_model.last_used_at = Set(Some(Utc::now()));
        active_model.updated_at = Set(Utc::now());
        let result = active_model.update(db).await?;

        Ok(result)
    }

    /// Delete an OIDC connection by ID
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        OidcConnection::delete_by_id(id).exec(db).await?;
        Ok(())
    }

    /// Delete all OIDC connections for a user
    ///
    /// This is typically called when a user is deleted.
    pub async fn delete_by_user_id(db: &DatabaseConnection, user_id: Uuid) -> Result<u64> {
        let result = OidcConnection::delete_many()
            .filter(oidc_connections::Column::UserId.eq(user_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Delete OIDC connection by provider and subject
    ///
    /// This can be used to unlink a specific IdP account.
    pub async fn delete_by_provider_subject(
        db: &DatabaseConnection,
        provider_name: &str,
        subject: &str,
    ) -> Result<u64> {
        let result = OidcConnection::delete_many()
            .filter(oidc_connections::Column::ProviderName.eq(provider_name))
            .filter(oidc_connections::Column::Subject.eq(subject))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Check if a user has any OIDC connections
    pub async fn user_has_connections(db: &DatabaseConnection, user_id: Uuid) -> Result<bool> {
        let count = OidcConnection::find()
            .filter(oidc_connections::Column::UserId.eq(user_id))
            .count(db)
            .await?;
        Ok(count > 0)
    }

    /// Count OIDC connections for a provider
    pub async fn count_by_provider(db: &DatabaseConnection, provider_name: &str) -> Result<u64> {
        let count = OidcConnection::find()
            .filter(oidc_connections::Column::ProviderName.eq(provider_name))
            .count(db)
            .await?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::users;
    use crate::db::repositories::UserRepository;
    use crate::db::test_helpers::setup_test_db;

    async fn create_test_user(db: &DatabaseConnection) -> users::Model {
        let user = users::Model {
            id: Uuid::new_v4(),
            username: format!("oidcuser_{}", Uuid::new_v4()),
            email: format!("oidc_{}@example.com", Uuid::new_v4()),
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

    fn create_test_connection(
        user_id: Uuid,
        provider: &str,
        subject: &str,
    ) -> oidc_connections::Model {
        oidc_connections::Model {
            id: Uuid::new_v4(),
            user_id,
            provider_name: provider.to_string(),
            subject: subject.to_string(),
            email: Some("test@idp.example.com".to_string()),
            display_name: Some("Test User".to_string()),
            groups: Some(serde_json::json!(["users", "readers"])),
            access_token_hash: None,
            refresh_token_encrypted: None,
            token_expires_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_used_at: None,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_oidc_connection() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let connection = create_test_connection(user.id, "authentik", "sub_123456");

        let created = OidcConnectionRepository::create(&db, &connection)
            .await
            .unwrap();
        assert_eq!(created.provider_name, "authentik");
        assert_eq!(created.subject, "sub_123456");
        assert_eq!(created.user_id, user.id);

        let found = OidcConnectionRepository::get_by_id(&db, created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.provider_name, "authentik");
        assert_eq!(found.subject, "sub_123456");
    }

    #[tokio::test]
    async fn test_find_by_provider_subject() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let connection = create_test_connection(user.id, "keycloak", "kc_user_789");
        OidcConnectionRepository::create(&db, &connection)
            .await
            .unwrap();

        let found =
            OidcConnectionRepository::find_by_provider_subject(&db, "keycloak", "kc_user_789")
                .await
                .unwrap()
                .unwrap();
        assert_eq!(found.user_id, user.id);

        // Non-existent should return None
        let not_found =
            OidcConnectionRepository::find_by_provider_subject(&db, "keycloak", "nonexistent")
                .await
                .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_find_by_user_id() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Create multiple connections for the same user
        let conn1 = create_test_connection(user.id, "authentik", "auth_1");
        let conn2 = create_test_connection(user.id, "keycloak", "kc_1");
        OidcConnectionRepository::create(&db, &conn1).await.unwrap();
        OidcConnectionRepository::create(&db, &conn2).await.unwrap();

        let connections = OidcConnectionRepository::find_by_user_id(&db, user.id)
            .await
            .unwrap();
        assert_eq!(connections.len(), 2);
    }

    #[tokio::test]
    async fn test_update_last_used() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let connection = create_test_connection(user.id, "authentik", "sub_update_test");
        let created = OidcConnectionRepository::create(&db, &connection)
            .await
            .unwrap();
        assert!(created.last_used_at.is_none());

        OidcConnectionRepository::update_last_used(&db, created.id)
            .await
            .unwrap();

        let updated = OidcConnectionRepository::get_by_id(&db, created.id)
            .await
            .unwrap()
            .unwrap();
        assert!(updated.last_used_at.is_some());
    }

    #[tokio::test]
    async fn test_update_groups_and_last_used() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let connection = create_test_connection(user.id, "authentik", "sub_groups_test");
        let created = OidcConnectionRepository::create(&db, &connection)
            .await
            .unwrap();

        let new_groups = serde_json::json!(["admins", "developers"]);
        let updated = OidcConnectionRepository::update_groups_and_last_used(
            &db,
            created.id,
            Some(new_groups.clone()),
            Some("new@email.com".to_string()),
            Some("New Display Name".to_string()),
        )
        .await
        .unwrap();

        assert_eq!(updated.groups, Some(new_groups));
        assert_eq!(updated.email, Some("new@email.com".to_string()));
        assert_eq!(updated.display_name, Some("New Display Name".to_string()));
        assert!(updated.last_used_at.is_some());
    }

    #[tokio::test]
    async fn test_delete_connection() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let connection = create_test_connection(user.id, "authentik", "sub_delete_test");
        let created = OidcConnectionRepository::create(&db, &connection)
            .await
            .unwrap();

        OidcConnectionRepository::delete(&db, created.id)
            .await
            .unwrap();

        let not_found = OidcConnectionRepository::get_by_id(&db, created.id)
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_delete_by_user_id() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Create multiple connections
        let conn1 = create_test_connection(user.id, "authentik", "auth_del_1");
        let conn2 = create_test_connection(user.id, "keycloak", "kc_del_1");
        OidcConnectionRepository::create(&db, &conn1).await.unwrap();
        OidcConnectionRepository::create(&db, &conn2).await.unwrap();

        let deleted_count = OidcConnectionRepository::delete_by_user_id(&db, user.id)
            .await
            .unwrap();
        assert_eq!(deleted_count, 2);

        let remaining = OidcConnectionRepository::find_by_user_id(&db, user.id)
            .await
            .unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_delete_by_provider_subject() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let connection = create_test_connection(user.id, "authentik", "sub_provider_del");
        OidcConnectionRepository::create(&db, &connection)
            .await
            .unwrap();

        let deleted_count = OidcConnectionRepository::delete_by_provider_subject(
            &db,
            "authentik",
            "sub_provider_del",
        )
        .await
        .unwrap();
        assert_eq!(deleted_count, 1);

        let not_found = OidcConnectionRepository::find_by_provider_subject(
            &db,
            "authentik",
            "sub_provider_del",
        )
        .await
        .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_user_has_connections() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Initially no connections
        let has_connections = OidcConnectionRepository::user_has_connections(&db, user.id)
            .await
            .unwrap();
        assert!(!has_connections);

        // Add a connection
        let connection = create_test_connection(user.id, "authentik", "sub_has_test");
        OidcConnectionRepository::create(&db, &connection)
            .await
            .unwrap();

        let has_connections = OidcConnectionRepository::user_has_connections(&db, user.id)
            .await
            .unwrap();
        assert!(has_connections);
    }

    #[tokio::test]
    async fn test_count_by_provider() {
        let db = setup_test_db().await;
        let user1 = create_test_user(&db).await;
        let user2 = create_test_user(&db).await;

        // Create connections for different providers
        let conn1 = create_test_connection(user1.id, "authentik", "auth_count_1");
        let conn2 = create_test_connection(user2.id, "authentik", "auth_count_2");
        let conn3 = create_test_connection(user1.id, "keycloak", "kc_count_1");
        OidcConnectionRepository::create(&db, &conn1).await.unwrap();
        OidcConnectionRepository::create(&db, &conn2).await.unwrap();
        OidcConnectionRepository::create(&db, &conn3).await.unwrap();

        let authentik_count = OidcConnectionRepository::count_by_provider(&db, "authentik")
            .await
            .unwrap();
        assert_eq!(authentik_count, 2);

        let keycloak_count = OidcConnectionRepository::count_by_provider(&db, "keycloak")
            .await
            .unwrap();
        assert_eq!(keycloak_count, 1);
    }
}
