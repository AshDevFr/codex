//! Refresh-token service: hash, validate, rotate, detect theft.
//!
//! The plain token never lives at rest. We persist `sha256(token)` only, so a
//! database compromise yields nothing usable. Each rotation issues a new plain
//! token and revokes the previous row in the same transaction; reuse of an
//! already-revoked token is treated as theft and revokes every refresh token
//! sharing the same `family_id`.

#![allow(dead_code)]

use anyhow::Result;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use rand::Rng;
use sea_orm::DatabaseConnection;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::db::entities::refresh_tokens;
use crate::db::repositories::{NewRefreshToken, RefreshTokenRepository};

/// 32 random bytes -> 43-character URL-safe base64 (no padding).
const TOKEN_BYTES: usize = 32;

#[derive(Debug, Error)]
pub enum RefreshTokenError {
    /// Token does not match any row.
    #[error("refresh token not recognized")]
    Unknown,
    /// Token row exists but is past `expires_at`.
    #[error("refresh token expired")]
    Expired,
    /// Token row exists but has already been revoked. This is the theft-detection
    /// path: the caller has already triggered family-wide revocation.
    #[error("refresh token revoked")]
    Revoked,
    /// Atomic rotation could not insert/revoke (e.g. lost a race with a parallel
    /// refresh, or an unexpected DB error).
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Result of a successful refresh: the new plain token (return to the caller
/// once, never again) plus the persisted row.
#[derive(Debug)]
pub struct IssuedRefreshToken {
    pub plain_token: String,
    pub model: refresh_tokens::Model,
}

/// Service for issuing and rotating refresh tokens.
#[derive(Clone)]
pub struct RefreshTokenService {
    db: DatabaseConnection,
    ttl: Duration,
}

impl RefreshTokenService {
    /// Build a service. `expiry_days` should mirror
    /// `auth.refresh_token_expiry_days` from configuration.
    pub fn new(db: DatabaseConnection, expiry_days: u32) -> Self {
        Self {
            db,
            ttl: Duration::days(expiry_days as i64),
        }
    }

    /// Generate a fresh, cryptographically random refresh token (43 chars,
    /// URL-safe base64 over 32 random bytes).
    pub fn generate_token() -> String {
        let mut bytes = [0u8; TOKEN_BYTES];
        rand::rng().fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Stable, deterministic sha256 hex of the plain token. Lookup key in the DB.
    pub fn hash_token(plain: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(plain.as_bytes());
        hex_encode(&hasher.finalize())
    }

    /// Issue a brand-new refresh token for a fresh login. The returned plain
    /// token is shown to the client exactly once. The persisted row uses a new
    /// `family_id` so subsequent rotations from this login can be grouped.
    pub async fn issue(
        &self,
        user_id: Uuid,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<IssuedRefreshToken> {
        let plain = Self::generate_token();
        let hash = Self::hash_token(&plain);
        let now = Utc::now();

        let model = RefreshTokenRepository::create(
            &self.db,
            NewRefreshToken {
                user_id,
                family_id: Uuid::new_v4(),
                token_hash: hash,
                issued_at: now,
                expires_at: now + self.ttl,
                user_agent,
                ip_address,
            },
        )
        .await?;

        Ok(IssuedRefreshToken {
            plain_token: plain,
            model,
        })
    }

    /// Validate the supplied plain token and atomically rotate it for a fresh
    /// one. The new token shares the predecessor's `family_id`.
    ///
    /// Error semantics:
    /// - `Unknown`: no row matches the hash.
    /// - `Expired`: matching row is past its `expires_at`.
    /// - `Revoked`: matching row is already revoked. This is the theft-detection
    ///   case; the entire family is revoked as a side effect before the error
    ///   returns, so the legitimate client (which holds a sibling token in the
    ///   same family) will also fail on its next refresh and be forced to log
    ///   in again. That is the documented OAuth 2.0 BCP recommendation.
    pub async fn validate_and_rotate(
        &self,
        plain_token: &str,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<IssuedRefreshToken, RefreshTokenError> {
        let hash = Self::hash_token(plain_token);
        let existing = RefreshTokenRepository::get_by_hash(&self.db, &hash)
            .await
            .map_err(RefreshTokenError::Other)?
            .ok_or(RefreshTokenError::Unknown)?;

        let now = Utc::now();

        if existing.revoked_at.is_some() {
            // Reuse of a rotated/revoked token. Revoke the whole family and
            // refuse the refresh. Failures from the revoke are surfaced rather
            // than swallowed: better to error than to silently leave a known
            // theft chain alive.
            RefreshTokenRepository::revoke_family(&self.db, existing.family_id)
                .await
                .map_err(RefreshTokenError::Other)?;
            return Err(RefreshTokenError::Revoked);
        }

        if existing.expires_at <= now {
            return Err(RefreshTokenError::Expired);
        }

        let new_plain = Self::generate_token();
        let new_hash = Self::hash_token(&new_plain);

        let new_model = RefreshTokenRepository::rotate(
            &self.db,
            existing.id,
            NewRefreshToken {
                user_id: existing.user_id,
                family_id: existing.family_id,
                token_hash: new_hash,
                issued_at: now,
                expires_at: now + self.ttl,
                user_agent,
                ip_address,
            },
        )
        .await
        .map_err(RefreshTokenError::Other)?;

        Ok(IssuedRefreshToken {
            plain_token: new_plain,
            model: new_model,
        })
    }

    /// Revoke a single refresh token. Used by the logout endpoint.
    ///
    /// Idempotent: an unknown or already-revoked token still returns `Ok(())`.
    /// We intentionally do not surface "not found" because logout must not leak
    /// whether the supplied token was ever valid.
    pub async fn revoke(&self, plain_token: &str) -> Result<()> {
        let hash = Self::hash_token(plain_token);
        if let Some(existing) = RefreshTokenRepository::get_by_hash(&self.db, &hash).await? {
            RefreshTokenRepository::revoke(&self.db, existing.id).await?;
        }
        Ok(())
    }

    /// Revoke every refresh token in `family_id`. Surface for the "sign out of
    /// all devices" feature and for tests.
    pub async fn revoke_family(&self, family_id: Uuid) -> Result<u64> {
        RefreshTokenRepository::revoke_family(&self.db, family_id).await
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        write!(&mut s, "{:02x}", b).expect("writing to String never fails");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::db::entities::users;
    use crate::db::repositories::UserRepository;
    use codex_config::{DatabaseConfig, DatabaseType, SQLiteConfig};
    use std::collections::HashMap;
    use tempfile::TempDir;

    async fn setup() -> (DatabaseConnection, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let mut pragmas = HashMap::new();
        pragmas.insert("foreign_keys".to_string(), "ON".to_string());

        let config = DatabaseConfig {
            db_type: DatabaseType::SQLite,
            postgres: None,
            sqlite: Some(SQLiteConfig {
                path: db_path.to_str().unwrap().to_string(),
                pragmas: Some(pragmas),
                ..SQLiteConfig::default()
            }),
        };

        let db = Database::new(&config).await.unwrap();
        db.run_migrations().await.unwrap();
        (db.sea_orm_connection().clone(), temp_dir)
    }

    async fn create_user(db: &DatabaseConnection) -> users::Model {
        let now = Utc::now();
        let model = users::Model {
            id: Uuid::new_v4(),
            username: format!("user-{}", Uuid::new_v4()),
            email: format!("{}@example.com", Uuid::new_v4()),
            password_hash: "hash".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: true,
            permissions: serde_json::json!([]),
            created_at: now,
            updated_at: now,
            last_login_at: None,
        };
        UserRepository::create(db, &model).await.unwrap()
    }

    #[test]
    fn generate_token_is_url_safe_and_unique() {
        let a = RefreshTokenService::generate_token();
        let b = RefreshTokenService::generate_token();
        // 32 bytes -> 43 chars URL-safe base64 no-pad.
        assert_eq!(a.len(), 43);
        assert_eq!(b.len(), 43);
        assert_ne!(a, b);
        assert!(
            a.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
    }

    #[test]
    fn hash_token_is_stable_and_hex() {
        let t = "the-token";
        let a = RefreshTokenService::hash_token(t);
        let b = RefreshTokenService::hash_token(t);
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn issue_persists_and_returns_plain() {
        let (db, _tmp) = setup().await;
        let user = create_user(&db).await;
        let svc = RefreshTokenService::new(db.clone(), 30);

        let issued = svc.issue(user.id, None, None).await.unwrap();

        assert_eq!(issued.model.user_id, user.id);
        assert_eq!(issued.model.token_hash.len(), 64);
        assert!(issued.model.revoked_at.is_none());
        assert!(issued.model.expires_at > Utc::now());

        let lookup = RefreshTokenRepository::get_by_hash(
            &db,
            &RefreshTokenService::hash_token(&issued.plain_token),
        )
        .await
        .unwrap();
        assert!(lookup.is_some());
    }

    #[tokio::test]
    async fn validate_and_rotate_happy_path() {
        let (db, _tmp) = setup().await;
        let user = create_user(&db).await;
        let svc = RefreshTokenService::new(db.clone(), 30);

        let first = svc.issue(user.id, None, None).await.unwrap();
        let second = svc
            .validate_and_rotate(&first.plain_token, None, None)
            .await
            .unwrap();

        assert_ne!(first.plain_token, second.plain_token);
        assert_eq!(second.model.family_id, first.model.family_id);
        assert_eq!(second.model.user_id, user.id);
        assert!(second.model.revoked_at.is_none());

        let old = RefreshTokenRepository::get_by_id(&db, first.model.id)
            .await
            .unwrap()
            .unwrap();
        assert!(old.revoked_at.is_some(), "old token must be revoked");
        assert_eq!(
            old.replaced_by,
            Some(second.model.id),
            "old token must link to its successor"
        );
    }

    #[tokio::test]
    async fn unknown_token_returns_unknown() {
        let (db, _tmp) = setup().await;
        let svc = RefreshTokenService::new(db.clone(), 30);
        let bogus = RefreshTokenService::generate_token();

        let err = svc
            .validate_and_rotate(&bogus, None, None)
            .await
            .unwrap_err();
        assert!(matches!(err, RefreshTokenError::Unknown));
    }

    #[tokio::test]
    async fn expired_token_returns_expired() {
        let (db, _tmp) = setup().await;
        let user = create_user(&db).await;
        let svc = RefreshTokenService::new(db.clone(), 30);

        let plain = RefreshTokenService::generate_token();
        let hash = RefreshTokenService::hash_token(&plain);
        let now = Utc::now();
        RefreshTokenRepository::create(
            &db,
            NewRefreshToken {
                user_id: user.id,
                family_id: Uuid::new_v4(),
                token_hash: hash,
                issued_at: now - Duration::days(2),
                expires_at: now - Duration::days(1),
                user_agent: None,
                ip_address: None,
            },
        )
        .await
        .unwrap();

        let err = svc
            .validate_and_rotate(&plain, None, None)
            .await
            .unwrap_err();
        assert!(matches!(err, RefreshTokenError::Expired));
    }

    #[tokio::test]
    async fn reusing_rotated_token_revokes_family() {
        let (db, _tmp) = setup().await;
        let user = create_user(&db).await;
        let svc = RefreshTokenService::new(db.clone(), 30);

        let first = svc.issue(user.id, None, None).await.unwrap();
        let second = svc
            .validate_and_rotate(&first.plain_token, None, None)
            .await
            .unwrap();

        // Reuse of the (now-revoked) original token -> theft detection path.
        let err = svc
            .validate_and_rotate(&first.plain_token, None, None)
            .await
            .unwrap_err();
        assert!(matches!(err, RefreshTokenError::Revoked));

        // The legitimate sibling token is now also revoked.
        let sibling = RefreshTokenRepository::get_by_id(&db, second.model.id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            sibling.revoked_at.is_some(),
            "family-wide revocation must catch the live sibling"
        );

        // And it can no longer be used.
        let err2 = svc
            .validate_and_rotate(&second.plain_token, None, None)
            .await
            .unwrap_err();
        assert!(matches!(err2, RefreshTokenError::Revoked));
    }

    #[tokio::test]
    async fn revoke_marks_single_token_only() {
        let (db, _tmp) = setup().await;
        let user = create_user(&db).await;
        let svc = RefreshTokenService::new(db.clone(), 30);

        let issued_a = svc.issue(user.id, None, None).await.unwrap();
        let issued_b = svc.issue(user.id, None, None).await.unwrap();

        svc.revoke(&issued_a.plain_token).await.unwrap();

        let a = RefreshTokenRepository::get_by_id(&db, issued_a.model.id)
            .await
            .unwrap()
            .unwrap();
        let b = RefreshTokenRepository::get_by_id(&db, issued_b.model.id)
            .await
            .unwrap()
            .unwrap();
        assert!(a.revoked_at.is_some());
        assert!(
            b.revoked_at.is_none(),
            "revoke must touch only the supplied token, not the family"
        );
    }

    #[tokio::test]
    async fn revoke_unknown_token_is_noop() {
        let (db, _tmp) = setup().await;
        let svc = RefreshTokenService::new(db.clone(), 30);

        svc.revoke("not-a-real-token").await.unwrap();
    }
}
