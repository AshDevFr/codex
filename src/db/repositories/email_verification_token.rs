//! Repository for email verification tokens
//!
//! TODO: Remove allow(dead_code) once email verification is fully integrated

#![allow(dead_code)]

use crate::db::entities::{
    email_verification_tokens, email_verification_tokens::Entity as EmailVerificationToken,
};
use anyhow::Result;
use chrono::{Duration, Utc};
use sea_orm::*;
use uuid::Uuid;

pub struct EmailVerificationTokenRepository;

impl EmailVerificationTokenRepository {
    /// Generate a random verification token
    pub fn generate_token() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        const TOKEN_LEN: usize = 64;
        let mut rng = rand::thread_rng();

        (0..TOKEN_LEN)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    /// Create a new verification token
    pub async fn create(
        db: &DatabaseConnection,
        user_id: Uuid,
        expiry_hours: i64,
    ) -> Result<email_verification_tokens::Model> {
        let token = Self::generate_token();
        let now = Utc::now();
        let expires_at = now + Duration::hours(expiry_hours);

        let active_model = email_verification_tokens::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            token: Set(token),
            expires_at: Set(expires_at),
            created_at: Set(now),
        };

        let result = active_model.insert(db).await?;
        Ok(result)
    }

    /// Get token by token string
    pub async fn get_by_token(
        db: &DatabaseConnection,
        token: &str,
    ) -> Result<Option<email_verification_tokens::Model>> {
        let token_model = EmailVerificationToken::find()
            .filter(email_verification_tokens::Column::Token.eq(token))
            .one(db)
            .await?;
        Ok(token_model)
    }

    /// Check if token is valid (exists and not expired)
    pub async fn is_valid(db: &DatabaseConnection, token: &str) -> Result<bool> {
        let token_model = Self::get_by_token(db, token).await?;

        match token_model {
            Some(model) => {
                let now = Utc::now();
                Ok(model.expires_at > now)
            }
            None => Ok(false),
        }
    }

    /// Delete token by token string
    pub async fn delete_by_token(db: &DatabaseConnection, token: &str) -> Result<()> {
        EmailVerificationToken::delete_many()
            .filter(email_verification_tokens::Column::Token.eq(token))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Delete all tokens for a user
    pub async fn delete_by_user_id(db: &DatabaseConnection, user_id: Uuid) -> Result<()> {
        EmailVerificationToken::delete_many()
            .filter(email_verification_tokens::Column::UserId.eq(user_id))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Get token by user ID (latest one)
    pub async fn get_by_user_id(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Option<email_verification_tokens::Model>> {
        let token = EmailVerificationToken::find()
            .filter(email_verification_tokens::Column::UserId.eq(user_id))
            .order_by_desc(email_verification_tokens::Column::CreatedAt)
            .one(db)
            .await?;
        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::users;
    use crate::db::repositories::user::UserRepository;
    use crate::db::test_helpers::setup_test_db;

    async fn create_test_user(db: &DatabaseConnection) -> users::Model {
        let user = users::Model {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash: "hash123".to_string(),
            is_admin: false,
            is_active: false,
            email_verified: false,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        UserRepository::create(db, &user).await.unwrap()
    }

    #[tokio::test]
    async fn test_generate_token() {
        let token1 = EmailVerificationTokenRepository::generate_token();
        let token2 = EmailVerificationTokenRepository::generate_token();

        assert_eq!(token1.len(), 64);
        assert_eq!(token2.len(), 64);
        assert_ne!(token1, token2); // Should be different
    }

    #[tokio::test]
    async fn test_create_and_get_token() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let token = EmailVerificationTokenRepository::create(&db, user.id, 24)
            .await
            .unwrap();

        assert_eq!(token.user_id, user.id);
        assert!(!token.token.is_empty());

        let found = EmailVerificationTokenRepository::get_by_token(&db, &token.token)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.user_id, user.id);
    }

    #[tokio::test]
    async fn test_is_valid() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let token = EmailVerificationTokenRepository::create(&db, user.id, 24)
            .await
            .unwrap();

        let is_valid = EmailVerificationTokenRepository::is_valid(&db, &token.token)
            .await
            .unwrap();
        assert!(is_valid);

        // Test invalid token
        let is_valid = EmailVerificationTokenRepository::is_valid(&db, "invalid_token")
            .await
            .unwrap();
        assert!(!is_valid);
    }

    #[tokio::test]
    async fn test_delete_by_token() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let token = EmailVerificationTokenRepository::create(&db, user.id, 24)
            .await
            .unwrap();

        EmailVerificationTokenRepository::delete_by_token(&db, &token.token)
            .await
            .unwrap();

        let found = EmailVerificationTokenRepository::get_by_token(&db, &token.token)
            .await
            .unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_delete_by_user_id() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        // Create multiple tokens
        EmailVerificationTokenRepository::create(&db, user.id, 24)
            .await
            .unwrap();
        EmailVerificationTokenRepository::create(&db, user.id, 24)
            .await
            .unwrap();

        EmailVerificationTokenRepository::delete_by_user_id(&db, user.id)
            .await
            .unwrap();

        let found = EmailVerificationTokenRepository::get_by_user_id(&db, user.id)
            .await
            .unwrap();
        assert!(found.is_none());
    }
}
