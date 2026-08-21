//! `user_plugin_oauth_states` repository tests, run against both backends.
//!
//! `consume` issues raw `DELETE ... RETURNING` SQL whose placeholder syntax
//! differs between SQLite and PostgreSQL, so exercising only the default
//! backend would leave the PostgreSQL statement, the one production actually
//! runs, unverified.

#[path = "../common/mod.rs"]
mod common;

use chrono::{Duration, Utc};
use codex::db::repositories::{PluginsRepository, UserRepository};
use codex_db::repositories::{NewUserPluginOAuthState, UserPluginOAuthStateRepository};
use common::*;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

/// Seed the rows the table's foreign keys require, returning `(plugin, user)`.
async fn seed(db: &DatabaseConnection) -> (Uuid, Uuid) {
    let plugin = PluginsRepository::create(
        db,
        &format!("oauth_state_plugin_{}", Uuid::new_v4()),
        "OAuth State Test Plugin",
        Some("A test plugin"),
        "user",
        "node",
        vec!["index.js".to_string()],
        vec![],
        None,
        vec![],
        vec![],
        vec![],
        None,
        "env",
        None,
        true,
        None,
        None,
        None,
        None, // log_level
    )
    .await
    .unwrap();

    let user = create_test_user(
        &format!("u-{}", Uuid::new_v4()),
        &format!("{}@example.com", Uuid::new_v4()),
        "hash",
        false,
    );
    let user = UserRepository::create(db, &user).await.unwrap();

    (plugin.id, user.id)
}

fn flow(
    state: &str,
    plugin_id: Uuid,
    user_id: Uuid,
    expires_in_secs: i64,
) -> NewUserPluginOAuthState {
    let now = Utc::now();
    NewUserPluginOAuthState {
        state: state.to_string(),
        plugin_id,
        user_id,
        pkce_verifier: Some(format!("verifier-for-{state}")),
        pkce_challenge: Some(format!("challenge-for-{state}")),
        redirect_uri: "https://codex.local/callback".to_string(),
        created_at: now,
        expires_at: now + Duration::seconds(expires_in_secs),
    }
}

async fn exercise_roundtrip_and_single_use(db: &DatabaseConnection) {
    let (plugin_id, user_id) = seed(db).await;
    let state = format!("s-{}", Uuid::new_v4());

    UserPluginOAuthStateRepository::create(db, flow(&state, plugin_id, user_id, 300))
        .await
        .unwrap();

    let got = UserPluginOAuthStateRepository::consume(db, &state)
        .await
        .unwrap()
        .expect("a live flow is consumable");
    assert_eq!(got.plugin_id, plugin_id);
    assert_eq!(got.user_id, user_id);
    assert_eq!(
        got.pkce_verifier.as_deref(),
        Some(&*format!("verifier-for-{state}"))
    );
    assert_eq!(got.redirect_uri, "https://codex.local/callback");

    assert!(
        UserPluginOAuthStateRepository::consume(db, &state)
            .await
            .unwrap()
            .is_none(),
        "a state token must not be redeemable twice"
    );
}

/// The per-user limit must count only live flows, or an abandoned flow locks a
/// user out of connecting until the next sweep runs.
async fn exercise_live_count_for_user(db: &DatabaseConnection) {
    let (plugin_id, user_id) = seed(db).await;
    let (_, other_user) = seed(db).await;

    UserPluginOAuthStateRepository::create(
        db,
        flow(&format!("live-{}", Uuid::new_v4()), plugin_id, user_id, 300),
    )
    .await
    .unwrap();
    UserPluginOAuthStateRepository::create(
        db,
        flow(&format!("stale-{}", Uuid::new_v4()), plugin_id, user_id, -1),
    )
    .await
    .unwrap();
    UserPluginOAuthStateRepository::create(
        db,
        flow(
            &format!("other-{}", Uuid::new_v4()),
            plugin_id,
            other_user,
            300,
        ),
    )
    .await
    .unwrap();

    assert_eq!(
        UserPluginOAuthStateRepository::count_live_for_user(db, user_id)
            .await
            .unwrap(),
        1,
        "the expired flow must not count against the user"
    );

    let deleted = UserPluginOAuthStateRepository::delete_expired(db)
        .await
        .unwrap();
    assert_eq!(deleted, 1);
}

#[tokio::test]
async fn test_user_plugin_oauth_state_roundtrip_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_roundtrip_and_single_use(&db).await;
}

#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn test_user_plugin_oauth_state_roundtrip_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };
    exercise_roundtrip_and_single_use(&db).await;
}

#[tokio::test]
async fn test_user_plugin_oauth_state_live_count_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_live_count_for_user(&db).await;
}

#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn test_user_plugin_oauth_state_live_count_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };
    exercise_live_count_for_user(&db).await;
}
