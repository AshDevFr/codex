//! JWT token generation and validation service
//!
//! TODO: Remove allow(dead_code) once all JWT features are fully integrated

#![allow(dead_code)]

use crate::api::permissions::UserRole;
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Username
    pub username: String,
    /// User role (reader, maintainer, admin)
    pub role: String,
    /// Expiration time (Unix timestamp)
    pub exp: usize,
    /// Issued at (Unix timestamp)
    pub iat: usize,
}

impl Claims {
    /// Get the user's role as a UserRole enum
    pub fn get_role(&self) -> UserRole {
        self.role.parse().unwrap_or_default()
    }
}

/// Service for generating and validating JWT tokens
pub struct JwtService {
    secret: String,
    expiry_hours: i64,
}

impl JwtService {
    /// Create a new JWT service
    ///
    /// # Arguments
    /// * `secret` - The secret key for signing tokens
    /// * `expiry_hours` - Token expiry time in hours
    pub fn new(secret: String, expiry_hours: u32) -> Self {
        Self {
            secret,
            expiry_hours: expiry_hours as i64,
        }
    }

    /// Generate a JWT token for a user
    ///
    /// # Arguments
    /// * `user_id` - The user's UUID
    /// * `username` - The user's username
    /// * `role` - The user's role
    ///
    /// # Returns
    /// The encoded JWT token string
    pub fn generate_token(
        &self,
        user_id: Uuid,
        username: String,
        role: UserRole,
    ) -> Result<String> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.expiry_hours);

        let claims = Claims {
            sub: user_id.to_string(),
            username,
            role: role.to_string(),
            exp: exp.timestamp() as usize,
            iat: now.timestamp() as usize,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
        .context("Failed to encode JWT token")
    }

    /// Verify and decode a JWT token
    ///
    /// # Arguments
    /// * `token` - The JWT token string to verify
    ///
    /// # Returns
    /// The decoded claims if the token is valid
    pub fn verify_token(&self, token: &str) -> Result<Claims> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )
        .context("Failed to decode JWT token")?;

        Ok(token_data.claims)
    }

    /// Decode a JWT token without verification (for debugging)
    ///
    /// # Arguments
    /// * `token` - The JWT token string to decode
    ///
    /// # Returns
    /// The decoded claims without verification
    ///
    /// # Warning
    /// This does not verify the token signature! Only use for debugging.
    pub fn decode_unverified(&self, token: &str) -> Result<Claims> {
        let mut validation = Validation::default();
        validation.insecure_disable_signature_validation();

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &validation,
        )
        .context("Failed to decode JWT token")?;

        Ok(token_data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> JwtService {
        JwtService::new("test_secret_key_for_jwt_testing_only".to_string(), 24)
    }

    #[test]
    fn test_generate_token() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();
        let username = "testuser".to_string();

        let token = service
            .generate_token(user_id, username.clone(), UserRole::Reader)
            .expect("Failed to generate token");

        // Token should not be empty
        assert!(!token.is_empty());

        // Token should have 3 parts separated by dots (JWT format)
        assert_eq!(token.split('.').count(), 3);
    }

    #[test]
    fn test_verify_token_valid_admin() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();
        let username = "testuser".to_string();

        let token = service
            .generate_token(user_id, username.clone(), UserRole::Admin)
            .expect("Failed to generate token");

        let claims = service
            .verify_token(&token)
            .expect("Failed to verify token");

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.username, username);
        assert_eq!(claims.role, "admin");
        assert_eq!(claims.get_role(), UserRole::Admin);
    }

    #[test]
    fn test_verify_token_valid_reader() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();
        let username = "testuser".to_string();

        let token = service
            .generate_token(user_id, username.clone(), UserRole::Reader)
            .expect("Failed to generate token");

        let claims = service
            .verify_token(&token)
            .expect("Failed to verify token");

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.username, username);
        assert_eq!(claims.role, "reader");
        assert_eq!(claims.get_role(), UserRole::Reader);
    }

    #[test]
    fn test_verify_token_valid_maintainer() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();
        let username = "testuser".to_string();

        let token = service
            .generate_token(user_id, username.clone(), UserRole::Maintainer)
            .expect("Failed to generate token");

        let claims = service
            .verify_token(&token)
            .expect("Failed to verify token");

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.username, username);
        assert_eq!(claims.role, "maintainer");
        assert_eq!(claims.get_role(), UserRole::Maintainer);
    }

    #[test]
    fn test_verify_token_invalid_secret() {
        let service1 = create_test_service();
        let service2 = JwtService::new("different_secret".to_string(), 24);

        let user_id = Uuid::new_v4();
        let token = service1
            .generate_token(user_id, "testuser".to_string(), UserRole::Reader)
            .expect("Failed to generate token");

        let result = service2.verify_token(&token);
        assert!(result.is_err(), "Should fail with different secret");
    }

    #[test]
    fn test_verify_token_tampered() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let token = service
            .generate_token(user_id, "testuser".to_string(), UserRole::Reader)
            .expect("Failed to generate token");

        // Tamper with the token by changing a character
        let mut tampered = token.clone();
        tampered.push('x');

        let result = service.verify_token(&tampered);
        assert!(result.is_err(), "Should fail with tampered token");
    }

    #[test]
    fn test_decode_unverified() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();
        let username = "testuser".to_string();

        let token = service
            .generate_token(user_id, username.clone(), UserRole::Reader)
            .expect("Failed to generate token");

        let claims = service
            .decode_unverified(&token)
            .expect("Failed to decode token");

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.username, username);
        assert_eq!(claims.role, "reader");
    }

    #[test]
    fn test_token_expiry() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let token = service
            .generate_token(user_id, "testuser".to_string(), UserRole::Reader)
            .expect("Failed to generate token");

        let claims = service
            .verify_token(&token)
            .expect("Failed to verify token");

        let now = Utc::now().timestamp() as usize;
        assert!(claims.exp > now, "Token should not be expired yet");
        assert!(claims.iat <= now, "Issued at should be in the past or now");
    }
}
