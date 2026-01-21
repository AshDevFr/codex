use crate::db::entities::{sharing_tags, user_sharing_tags, users, users::Entity as User};
use anyhow::Result;
use chrono::Utc;
use sea_orm::*;
use uuid::Uuid;

/// Parameters for filtering and paginating user list
#[derive(Debug, Clone, Default)]
pub struct UserListFilter {
    /// Filter by role
    pub role: Option<String>,
    /// Filter by sharing tag name (users who have a grant for this tag)
    pub sharing_tag: Option<String>,
    /// Filter by sharing tag access mode (allow/deny) - only used with sharing_tag
    pub sharing_tag_mode: Option<String>,
}

/// Paginated result for user listing
#[derive(Debug)]
pub struct UserListResult {
    pub users: Vec<users::Model>,
    pub total: u64,
}

pub struct UserRepository;

impl UserRepository {
    /// Create a new user
    pub async fn create(db: &DatabaseConnection, model: &users::Model) -> Result<users::Model> {
        let active_model = users::ActiveModel {
            id: Set(model.id),
            username: Set(model.username.clone()),
            email: Set(model.email.clone()),
            password_hash: Set(model.password_hash.clone()),
            role: Set(model.role.clone()),
            is_active: Set(model.is_active),
            email_verified: Set(model.email_verified),
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

    /// List users with filtering and pagination
    pub async fn list_paginated(
        db: &DatabaseConnection,
        filter: &UserListFilter,
        offset: u64,
        limit: u64,
    ) -> Result<UserListResult> {
        // Build base query with optional sharing tag join
        let user_ids = if filter.sharing_tag.is_some() {
            // When filtering by sharing tag, we need to find users with grants for that tag
            let tag_name = filter.sharing_tag.as_ref().unwrap();

            let mut query = user_sharing_tags::Entity::find()
                .inner_join(sharing_tags::Entity)
                .filter(sharing_tags::Column::Name.eq(tag_name));

            // Optionally filter by access mode
            if let Some(mode) = &filter.sharing_tag_mode {
                query = query.filter(user_sharing_tags::Column::AccessMode.eq(mode));
            }

            // Get user IDs with this sharing tag grant
            let grants: Vec<user_sharing_tags::Model> = query.all(db).await?;
            let ids: Vec<Uuid> = grants.into_iter().map(|g| g.user_id).collect();
            Some(ids)
        } else {
            None
        };

        // Build the user query
        let mut query = User::find();

        // Filter by role if specified
        if let Some(role) = &filter.role {
            query = query.filter(users::Column::Role.eq(role));
        }

        // Filter by user IDs if we have a sharing tag filter
        if let Some(ids) = &user_ids {
            if ids.is_empty() {
                // No users match the sharing tag filter
                return Ok(UserListResult {
                    users: vec![],
                    total: 0,
                });
            }
            query = query.filter(users::Column::Id.is_in(ids.iter().cloned()));
        }

        // Count total matching users
        let total = query.clone().count(db).await?;

        // Apply pagination and fetch results
        let users = query
            .order_by_asc(users::Column::Username)
            .offset(offset)
            .limit(limit)
            .all(db)
            .await?;

        Ok(UserListResult { users, total })
    }

    /// Check if any users exist in the database
    pub async fn has_any_users(db: &DatabaseConnection) -> Result<bool> {
        let count = User::find().count(db).await?;
        Ok(count > 0)
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
            role: "reader".to_string(),
            is_active: true,
            email_verified: false,
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
            role: "reader".to_string(),
            is_active: true,
            email_verified: false,
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
            role: "reader".to_string(),
            is_active: true,
            email_verified: false,
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

    #[tokio::test]
    async fn test_has_any_users_empty_db() {
        let db = setup_test_db().await;

        // Fresh database should have no users
        let has_users = UserRepository::has_any_users(&db).await.unwrap();
        assert!(!has_users, "Fresh database should have no users");
    }

    #[tokio::test]
    async fn test_has_any_users_with_users() {
        let db = setup_test_db().await;

        // Create a user
        let user = users::Model {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash: "hash123".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: false,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };

        UserRepository::create(&db, &user).await.unwrap();

        // Now database should have users
        let has_users = UserRepository::has_any_users(&db).await.unwrap();
        assert!(has_users, "Database should have users after creating one");
    }
}
