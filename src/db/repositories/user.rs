use crate::db::entities::{users, users::Entity as User};
use anyhow::Result;
use chrono::Utc;
use sea_orm::*;
use uuid::Uuid;

pub struct UserRepository;

impl UserRepository {
    /// Create a new user
    pub async fn create(db: &DatabaseConnection, model: &users::Model) -> Result<users::Model> {
        let active_model = users::ActiveModel {
            id: Set(model.id),
            username: Set(model.username.clone()),
            email: Set(model.email.clone()),
            password_hash: Set(model.password_hash.clone()),
            is_admin: Set(model.is_admin),
            is_active: Set(model.is_active),
            permissions: Set(model.permissions.clone()),
            created_at: Set(model.created_at),
            updated_at: Set(model.updated_at),
            last_login_at: Set(model.last_login_at),
        };

        let result = active_model.insert(db).await?;
        Ok(result)
    }

    /// Get user by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<users::Model>> {
        let user = User::find_by_id(id).one(db).await?;
        Ok(user)
    }

    /// Get user by username
    pub async fn get_by_username(
        db: &DatabaseConnection,
        username: &str,
    ) -> Result<Option<users::Model>> {
        let user = User::find()
            .filter(users::Column::Username.eq(username))
            .one(db)
            .await?;
        Ok(user)
    }

    /// Get user by email
    pub async fn get_by_email(
        db: &DatabaseConnection,
        email: &str,
    ) -> Result<Option<users::Model>> {
        let user = User::find()
            .filter(users::Column::Email.eq(email))
            .one(db)
            .await?;
        Ok(user)
    }

    /// Update user's last login timestamp
    pub async fn update_last_login(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        let user = User::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let mut active_model: users::ActiveModel = user.into();
        active_model.last_login_at = Set(Some(Utc::now()));
        active_model.updated_at = Set(Utc::now());
        active_model.update(db).await?;

        Ok(())
    }

    /// Update user
    pub async fn update(db: &DatabaseConnection, model: &users::Model) -> Result<users::Model> {
        let mut active_model: users::ActiveModel = model.clone().into();
        active_model.updated_at = Set(Utc::now());
        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Delete user
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        User::delete_by_id(id).exec(db).await?;
        Ok(())
    }

    /// List all users
    pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<users::Model>> {
        let users = User::find().all(db).await?;
        Ok(users)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::setup_test_db;

    #[tokio::test]
    async fn test_create_and_get_user() {
        let db = setup_test_db().await;

        let user = users::Model {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash: "hash123".to_string(),
            is_admin: false,
            is_active: true,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };

        let created = UserRepository::create(&db, &user).await.unwrap();
        assert_eq!(created.username, "testuser");

        let found = UserRepository::get_by_id(&db, created.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.username, "testuser");
    }

    #[tokio::test]
    async fn test_get_by_username() {
        let db = setup_test_db().await;

        let user = users::Model {
            id: Uuid::new_v4(),
            username: "findme".to_string(),
            email: "findme@example.com".to_string(),
            password_hash: "hash123".to_string(),
            is_admin: false,
            is_active: true,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };

        UserRepository::create(&db, &user).await.unwrap();

        let found = UserRepository::get_by_username(&db, "findme")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.email, "findme@example.com");
    }

    #[tokio::test]
    async fn test_update_last_login() {
        let db = setup_test_db().await;

        let user = users::Model {
            id: Uuid::new_v4(),
            username: "logintest".to_string(),
            email: "login@example.com".to_string(),
            password_hash: "hash123".to_string(),
            is_admin: false,
            is_active: true,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };

        let created = UserRepository::create(&db, &user).await.unwrap();
        assert!(created.last_login_at.is_none());

        UserRepository::update_last_login(&db, created.id)
            .await
            .unwrap();

        let updated = UserRepository::get_by_id(&db, created.id)
            .await
            .unwrap()
            .unwrap();
        assert!(updated.last_login_at.is_some());
    }
}
