//! Repository for refresh-token persistence.
//!
//! All cryptography lives in [`crate::services::refresh_token`]. This module only
//! handles CRUD, atomic rotation, and family-wide revocation against the
//! `refresh_tokens` table.

#![allow(dead_code)]

use crate::entities::{refresh_tokens, refresh_tokens::Entity as RefreshToken};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::sea_query::Expr;
use sea_orm::*;
use uuid::Uuid;

/// Input for creating a new refresh-token row.
///
/// The caller is responsible for hashing the plain token (sha256 hex) and
/// supplying `family_id`. On the first rotation of a new login this is a fresh
/// UUID; on subsequent rotations it carries over from the predecessor row.
pub struct NewRefreshToken {
    pub user_id: Uuid,
    pub family_id: Uuid,
    pub token_hash: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}

pub struct RefreshTokenRepository;

impl RefreshTokenRepository {
    /// Insert a new refresh-token row. Returns the persisted model.
    pub async fn create(
        db: &impl ConnectionTrait,
        input: NewRefreshToken,
    ) -> Result<refresh_tokens::Model> {
        let active_model = refresh_tokens::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(input.user_id),
            family_id: Set(input.family_id),
            token_hash: Set(input.token_hash),
            issued_at: Set(input.issued_at),
            expires_at: Set(input.expires_at),
            revoked_at: Set(None),
            replaced_by: Set(None),
            user_agent: Set(input.user_agent),
            ip_address: Set(input.ip_address),
        };
        Ok(active_model.insert(db).await?)
    }

    /// Look up a refresh token by its sha256 hash.
    pub async fn get_by_hash(
        db: &impl ConnectionTrait,
        token_hash: &str,
    ) -> Result<Option<refresh_tokens::Model>> {
        let row = RefreshToken::find()
            .filter(refresh_tokens::Column::TokenHash.eq(token_hash))
            .one(db)
            .await?;
        Ok(row)
    }

    /// Look up a refresh token by id.
    pub async fn get_by_id(
        db: &impl ConnectionTrait,
        id: Uuid,
    ) -> Result<Option<refresh_tokens::Model>> {
        Ok(RefreshToken::find_by_id(id).one(db).await?)
    }

    /// Mark a single refresh-token row revoked. No-op if it is already revoked.
    ///
    /// Returns the number of rows affected so callers can detect a stale rotation
    /// (a parallel refresh got there first).
    pub async fn revoke(db: &impl ConnectionTrait, id: Uuid) -> Result<u64> {
        let res = RefreshToken::update_many()
            .col_expr(
                refresh_tokens::Column::RevokedAt,
                Expr::value(Some(Utc::now())),
            )
            .filter(refresh_tokens::Column::Id.eq(id))
            .filter(refresh_tokens::Column::RevokedAt.is_null())
            .exec(db)
            .await?;
        Ok(res.rows_affected)
    }

    /// Revoke every refresh token in the supplied family. Used on theft
    /// detection (reuse of an already-rotated token).
    ///
    /// Idempotent: rows already revoked are skipped.
    pub async fn revoke_family(db: &impl ConnectionTrait, family_id: Uuid) -> Result<u64> {
        let res = RefreshToken::update_many()
            .col_expr(
                refresh_tokens::Column::RevokedAt,
                Expr::value(Some(Utc::now())),
            )
            .filter(refresh_tokens::Column::FamilyId.eq(family_id))
            .filter(refresh_tokens::Column::RevokedAt.is_null())
            .exec(db)
            .await?;
        Ok(res.rows_affected)
    }

    /// Atomically revoke `old_id` and insert a new row carrying the predecessor's
    /// `family_id`. If `old_id` is already revoked, the whole operation aborts
    /// with no new row inserted (the calling refresh attempt should be rejected
    /// and theft detection should run).
    ///
    /// Returns the newly-inserted model on success.
    pub async fn rotate(
        db: &DatabaseConnection,
        old_id: Uuid,
        new_token: NewRefreshToken,
    ) -> Result<refresh_tokens::Model> {
        let txn = db.begin().await?;

        let revoke_res = RefreshToken::update_many()
            .col_expr(
                refresh_tokens::Column::RevokedAt,
                Expr::value(Some(Utc::now())),
            )
            .filter(refresh_tokens::Column::Id.eq(old_id))
            .filter(refresh_tokens::Column::RevokedAt.is_null())
            .exec(&txn)
            .await?;

        if revoke_res.rows_affected == 0 {
            txn.rollback().await?;
            anyhow::bail!("refresh token already revoked or missing");
        }

        let new_id = Uuid::new_v4();
        let active_model = refresh_tokens::ActiveModel {
            id: Set(new_id),
            user_id: Set(new_token.user_id),
            family_id: Set(new_token.family_id),
            token_hash: Set(new_token.token_hash),
            issued_at: Set(new_token.issued_at),
            expires_at: Set(new_token.expires_at),
            revoked_at: Set(None),
            replaced_by: Set(None),
            user_agent: Set(new_token.user_agent),
            ip_address: Set(new_token.ip_address),
        };
        let inserted = active_model.insert(&txn).await?;

        RefreshToken::update_many()
            .col_expr(refresh_tokens::Column::ReplacedBy, Expr::value(new_id))
            .filter(refresh_tokens::Column::Id.eq(old_id))
            .exec(&txn)
            .await?;

        txn.commit().await?;
        Ok(inserted)
    }

    /// Delete expired tokens and tokens that have been revoked for longer than
    /// `revoked_grace_days`. Returns the number of rows removed.
    pub async fn cleanup_expired(
        db: &impl ConnectionTrait,
        revoked_grace_days: i64,
    ) -> Result<u64> {
        let now = Utc::now();
        let grace_cutoff = now - chrono::Duration::days(revoked_grace_days);

        let res = RefreshToken::delete_many()
            .filter(
                Condition::any()
                    .add(refresh_tokens::Column::ExpiresAt.lt(now))
                    .add(
                        Condition::all()
                            .add(refresh_tokens::Column::RevokedAt.is_not_null())
                            .add(refresh_tokens::Column::RevokedAt.lt(grace_cutoff)),
                    ),
            )
            .exec(db)
            .await?;

        Ok(res.rows_affected)
    }

    /// Delete every refresh token belonging to the user. Used on hard "sign out
    /// everywhere" requests or account deletion. Cascade handles the latter via
    /// the FK, but having an explicit hook is convenient.
    pub async fn delete_by_user_id(db: &impl ConnectionTrait, user_id: Uuid) -> Result<u64> {
        let res = RefreshToken::delete_many()
            .filter(refresh_tokens::Column::UserId.eq(user_id))
            .exec(db)
            .await?;
        Ok(res.rows_affected)
    }
}
