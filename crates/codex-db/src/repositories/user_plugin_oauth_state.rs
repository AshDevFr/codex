//! Repository for in-flight plugin OAuth connect flows.
//!
//! Shares its shape with [`crate::repositories::oidc_pending_state`]: both hold
//! the server-side half of an authorization-code flow between the two HTTP
//! requests that make it up, and neither can live in process memory if more
//! than one `codex serve` is running.

use crate::entities::{
    user_plugin_oauth_states, user_plugin_oauth_states::Entity as UserPluginOauthState,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::*;
use sea_orm::{DbBackend, Statement};
use uuid::Uuid;

/// A pending OAuth connect flow to persist.
pub struct NewUserPluginOAuthState {
    /// CSRF state token, and the primary key.
    pub state: String,
    pub plugin_id: Uuid,
    pub user_id: Uuid,
    pub pkce_verifier: Option<String>,
    pub pkce_challenge: Option<String>,
    pub redirect_uri: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub struct UserPluginOAuthStateRepository;

impl UserPluginOAuthStateRepository {
    /// Persist a pending flow.
    pub async fn create(db: &impl ConnectionTrait, input: NewUserPluginOAuthState) -> Result<()> {
        user_plugin_oauth_states::ActiveModel {
            state: Set(input.state),
            plugin_id: Set(input.plugin_id),
            user_id: Set(input.user_id),
            pkce_verifier: Set(input.pkce_verifier),
            pkce_challenge: Set(input.pkce_challenge),
            redirect_uri: Set(input.redirect_uri),
            created_at: Set(input.created_at),
            expires_at: Set(input.expires_at),
        }
        .insert(db)
        .await?;
        Ok(())
    }

    /// Take the pending flow for `state`, removing it.
    ///
    /// One statement, so the database picks the winner when two callbacks race
    /// the same token. A read followed by a separate delete would let both
    /// callers read before either deleted, handing both a usable PKCE verifier
    /// and losing the single-use property the CSRF token depends on.
    ///
    /// Expired rows are returned rather than filtered out, so the caller can
    /// report an expired flow as expired rather than as unknown. They are
    /// consumed either way, because the flow is over.
    pub async fn consume(
        db: &impl ConnectionTrait,
        state: &str,
    ) -> Result<Option<user_plugin_oauth_states::Model>> {
        const COLUMNS: &str = "state, plugin_id, user_id, pkce_verifier, pkce_challenge, \
                               redirect_uri, created_at, expires_at";

        // Placeholder syntax is the only thing that differs between backends.
        let backend = db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => {
                format!("DELETE FROM user_plugin_oauth_states WHERE state = $1 RETURNING {COLUMNS}")
            }
            _ => {
                format!("DELETE FROM user_plugin_oauth_states WHERE state = ? RETURNING {COLUMNS}")
            }
        };

        Ok(UserPluginOauthState::find()
            .from_raw_sql(Statement::from_sql_and_values(backend, sql, [state.into()]))
            .one(db)
            .await?)
    }

    /// Delete every flow that has passed its expiry. Returns the row count.
    pub async fn delete_expired(db: &impl ConnectionTrait) -> Result<u64> {
        let res = UserPluginOauthState::delete_many()
            .filter(user_plugin_oauth_states::Column::ExpiresAt.lt(Utc::now()))
            .exec(db)
            .await?;
        Ok(res.rows_affected)
    }

    /// Number of pending flows currently stored.
    pub async fn count(db: &impl ConnectionTrait) -> Result<u64> {
        Ok(UserPluginOauthState::find().count(db).await?)
    }

    /// Number of a user's flows that have not yet expired.
    ///
    /// Backs the per-user concurrent-flow limit. Expired rows are excluded here
    /// rather than relying on the sweep having run, so an abandoned flow stops
    /// counting against the user the moment it expires instead of up to a sweep
    /// interval later.
    pub async fn count_live_for_user(db: &impl ConnectionTrait, user_id: Uuid) -> Result<u64> {
        Ok(UserPluginOauthState::find()
            .filter(user_plugin_oauth_states::Column::UserId.eq(user_id))
            .filter(user_plugin_oauth_states::Column::ExpiresAt.gt(Utc::now()))
            .count(db)
            .await?)
    }
}
