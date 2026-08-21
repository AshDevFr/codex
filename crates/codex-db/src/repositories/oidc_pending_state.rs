//! Repository for in-flight OIDC login state.
//!
//! The store exists so that the authorization request and the callback, which
//! are separate HTTP requests, do not have to be served by the same process.

use crate::entities::{oidc_pending_states, oidc_pending_states::Entity as OidcPendingState};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::*;
use sea_orm::{DbBackend, Statement};

/// A pending login to persist.
pub struct NewOidcPendingState {
    /// CSRF state token, and the primary key.
    pub state: String,
    pub provider_name: String,
    pub pkce_verifier: String,
    pub nonce: String,
    /// `None` means the built-in web completion page.
    pub redirect_uri: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub struct OidcPendingStateRepository;

impl OidcPendingStateRepository {
    /// Persist a pending login.
    pub async fn create(db: &impl ConnectionTrait, input: NewOidcPendingState) -> Result<()> {
        oidc_pending_states::ActiveModel {
            state: Set(input.state),
            provider_name: Set(input.provider_name),
            pkce_verifier: Set(input.pkce_verifier),
            nonce: Set(input.nonce),
            redirect_uri: Set(input.redirect_uri),
            created_at: Set(input.created_at),
            expires_at: Set(input.expires_at),
        }
        .insert(db)
        .await?;
        Ok(())
    }

    /// Take the pending login for `state`, removing it.
    ///
    /// One statement, so the database decides the winner. Two callbacks racing
    /// the same state token cannot both be handed the PKCE verifier, because
    /// only one `DELETE` can match the row and only that one gets a row back.
    /// A read followed by a separate delete would not do: both callers could
    /// complete their read before either deleted, and both would proceed. That
    /// is the CSRF single-use guarantee gone, so the atomicity has to come
    /// from the engine rather than from how the two requests happen to
    /// interleave. Mirrors `ScheduledFiringRepository::try_claim`, which
    /// likewise lets a single statement pick the winner.
    ///
    /// Expired rows are returned rather than filtered out. The caller checks
    /// the TTL, so an expired state is reported as expired instead of being
    /// indistinguishable from one that never existed, and it is consumed
    /// either way because the flow is over.
    pub async fn consume(
        db: &impl ConnectionTrait,
        state: &str,
    ) -> Result<Option<oidc_pending_states::Model>> {
        const COLUMNS: &str =
            "state, provider_name, pkce_verifier, nonce, redirect_uri, created_at, expires_at";

        // Placeholder syntax is the only thing that differs between backends.
        let backend = db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => {
                format!("DELETE FROM oidc_pending_states WHERE state = $1 RETURNING {COLUMNS}")
            }
            _ => format!("DELETE FROM oidc_pending_states WHERE state = ? RETURNING {COLUMNS}"),
        };

        Ok(OidcPendingState::find()
            .from_raw_sql(Statement::from_sql_and_values(backend, sql, [state.into()]))
            .one(db)
            .await?)
    }

    /// Delete every state that has passed its expiry. Returns the row count.
    pub async fn delete_expired(db: &impl ConnectionTrait) -> Result<u64> {
        let res = OidcPendingState::delete_many()
            .filter(oidc_pending_states::Column::ExpiresAt.lt(Utc::now()))
            .exec(db)
            .await?;
        Ok(res.rows_affected)
    }

    /// Number of pending states currently stored.
    pub async fn count(db: &impl ConnectionTrait) -> Result<u64> {
        Ok(OidcPendingState::find().count(db).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::setup_test_db;
    use chrono::Duration;

    fn pending(state: &str, expires_in_secs: i64) -> NewOidcPendingState {
        let now = Utc::now();
        NewOidcPendingState {
            state: state.to_string(),
            provider_name: "authentik".to_string(),
            pkce_verifier: "verifier-value".to_string(),
            nonce: "nonce-value".to_string(),
            redirect_uri: Some("codexreader://auth".to_string()),
            created_at: now,
            expires_at: now + Duration::seconds(expires_in_secs),
        }
    }

    #[tokio::test]
    async fn create_then_consume_returns_the_row() {
        let db = setup_test_db().await;
        OidcPendingStateRepository::create(&db, pending("s1", 300))
            .await
            .unwrap();

        let got = OidcPendingStateRepository::consume(&db, "s1")
            .await
            .unwrap()
            .expect("state is consumable");

        assert_eq!(got.provider_name, "authentik");
        assert_eq!(got.pkce_verifier, "verifier-value");
        assert_eq!(got.nonce, "nonce-value");
        assert_eq!(got.redirect_uri.as_deref(), Some("codexreader://auth"));
    }

    #[tokio::test]
    async fn consume_is_single_use() {
        let db = setup_test_db().await;
        OidcPendingStateRepository::create(&db, pending("s1", 300))
            .await
            .unwrap();

        assert!(
            OidcPendingStateRepository::consume(&db, "s1")
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            OidcPendingStateRepository::consume(&db, "s1")
                .await
                .unwrap()
                .is_none(),
            "a state token must not be redeemable twice"
        );
    }

    #[tokio::test]
    async fn consume_of_unknown_state_is_none() {
        let db = setup_test_db().await;
        assert!(
            OidcPendingStateRepository::consume(&db, "never-existed")
                .await
                .unwrap()
                .is_none()
        );
    }

    /// Concurrent callbacks for one state token must produce exactly one
    /// winner.
    ///
    /// A smoke test, deliberately labelled as one: the pool serialises these
    /// tasks in practice, so this does not reliably construct the interleaving
    /// it is named after, and it still passed against a read-then-delete
    /// implementation that had no atomicity at all. What actually guarantees
    /// single use is that `consume` is one `DELETE ... RETURNING` statement,
    /// which the engine adjudicates. This test only catches a regression that
    /// breaks consumption outright.
    #[tokio::test]
    async fn concurrent_consume_yields_exactly_one_winner() {
        let db = setup_test_db().await;
        OidcPendingStateRepository::create(&db, pending("racy", 300))
            .await
            .unwrap();

        let mut handles = Vec::new();
        for _ in 0..8 {
            let db = db.clone();
            handles.push(tokio::spawn(async move {
                OidcPendingStateRepository::consume(&db, "racy")
                    .await
                    .unwrap()
                    .is_some()
            }));
        }

        let mut winners = 0;
        for handle in handles {
            if handle.await.unwrap() {
                winners += 1;
            }
        }

        assert_eq!(winners, 1, "exactly one caller may consume a state token");
    }

    #[tokio::test]
    async fn delete_expired_removes_only_expired_rows() {
        let db = setup_test_db().await;
        OidcPendingStateRepository::create(&db, pending("live", 300))
            .await
            .unwrap();
        OidcPendingStateRepository::create(&db, pending("stale", -1))
            .await
            .unwrap();

        let deleted = OidcPendingStateRepository::delete_expired(&db)
            .await
            .unwrap();

        assert_eq!(deleted, 1);
        assert_eq!(OidcPendingStateRepository::count(&db).await.unwrap(), 1);
        assert!(
            OidcPendingStateRepository::consume(&db, "live")
                .await
                .unwrap()
                .is_some(),
            "an unexpired state must survive the sweep"
        );
    }

    /// An expired state is handed back so the caller can report it as expired,
    /// and is removed on the way past because the flow is over either way.
    #[tokio::test]
    async fn consume_returns_expired_rows_and_removes_them() {
        let db = setup_test_db().await;
        OidcPendingStateRepository::create(&db, pending("stale", -1))
            .await
            .unwrap();

        assert!(
            OidcPendingStateRepository::consume(&db, "stale")
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(OidcPendingStateRepository::count(&db).await.unwrap(), 0);
    }
}
