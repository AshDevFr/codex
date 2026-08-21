//! `oidc_pending_states` repository tests, run against both backends.
//!
//! The single-use guarantee in `consume` is what stops a CSRF state token from
//! being redeemed twice, and it is enforced by the database rather than by
//! Codex: two concurrent deletes for the same key are serialised by the
//! engine, and only one of them reports an affected row. That is a claim about
//! the engine, so asserting it on SQLite alone proves nothing about the
//! PostgreSQL deployments this table exists to fix.

#[path = "../common/mod.rs"]
mod common;

use chrono::{Duration, Utc};
use codex_db::repositories::{NewOidcPendingState, OidcPendingStateRepository};
use common::*;
use sea_orm::DatabaseConnection;

fn pending(state: &str, expires_in_secs: i64) -> NewOidcPendingState {
    let now = Utc::now();
    NewOidcPendingState {
        state: state.to_string(),
        provider_name: "authentik".to_string(),
        pkce_verifier: format!("verifier-for-{state}"),
        nonce: format!("nonce-for-{state}"),
        redirect_uri: Some("codexreader://auth".to_string()),
        created_at: now,
        expires_at: now + Duration::seconds(expires_in_secs),
    }
}

/// Concurrent callbacks for one state token must produce exactly one winner.
///
/// The value here is the PostgreSQL run rather than the concurrency: `consume`
/// issues raw `DELETE ... RETURNING` SQL whose placeholder syntax differs per
/// backend, so this is what proves the PostgreSQL variant is well-formed and
/// maps back onto the model. The concurrency itself is only a smoke test,
/// since the pool serialises these tasks rather than overlapping them.
async fn exercise_single_use_under_concurrency(db: &DatabaseConnection) {
    let state = format!("racy-{}", uuid::Uuid::new_v4());
    OidcPendingStateRepository::create(db, pending(&state, 300))
        .await
        .unwrap();

    let mut handles = Vec::new();
    for _ in 0..8 {
        let db = db.clone();
        let state = state.clone();
        handles.push(tokio::spawn(async move {
            OidcPendingStateRepository::consume(&db, &state)
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

async fn exercise_roundtrip_and_sweep(db: &DatabaseConnection) {
    let live = format!("live-{}", uuid::Uuid::new_v4());
    let stale = format!("stale-{}", uuid::Uuid::new_v4());

    OidcPendingStateRepository::create(db, pending(&live, 300))
        .await
        .unwrap();
    OidcPendingStateRepository::create(db, pending(&stale, -1))
        .await
        .unwrap();

    // Timestamps survive the round trip well enough for the sweep to tell the
    // two apart, which is the property that differs most between backends.
    assert!(
        OidcPendingStateRepository::delete_expired(db)
            .await
            .unwrap()
            >= 1
    );

    let got = OidcPendingStateRepository::consume(db, &live)
        .await
        .unwrap()
        .expect("an unexpired state must survive the sweep");
    assert_eq!(got.pkce_verifier, format!("verifier-for-{live}"));
    assert_eq!(got.redirect_uri.as_deref(), Some("codexreader://auth"));

    assert!(
        OidcPendingStateRepository::consume(db, &stale)
            .await
            .unwrap()
            .is_none(),
        "the sweep must have removed the expired state"
    );
}

#[tokio::test]
async fn test_oidc_pending_state_single_use_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_single_use_under_concurrency(&db).await;
}

#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn test_oidc_pending_state_single_use_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };
    exercise_single_use_under_concurrency(&db).await;
}

#[tokio::test]
async fn test_oidc_pending_state_roundtrip_and_sweep_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_roundtrip_and_sweep(&db).await;
}

#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn test_oidc_pending_state_roundtrip_and_sweep_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };
    exercise_roundtrip_and_sweep(&db).await;
}
