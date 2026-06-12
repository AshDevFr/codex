//! IdP bearer token API integration tests.
//!
//! End-to-end coverage of the resource-server path: provider-issued
//! RS256 tokens on `Authorization: Bearer`, resolved to local users via
//! `oidc_connections`. The IdP is a local axum stub serving a discovery
//! document and a JWKS built from the static RSA test keys shared with
//! the codex-services suite.

#[path = "../common/mod.rs"]
mod common;

use axum::routing::get;
use axum::{Json, Router};
use codex::api::extractors::{AppState, IdpBearerAuth};
use codex::config::{OidcConfig, OidcDefaultRole, OidcProviderConfig};
use codex::db::entities::{oidc_connections, users};
use codex::db::repositories::{OidcConnectionRepository, UserRepository};
use common::*;
use hyper::StatusCode;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::Arc;

fn fixture_path(name: &str) -> String {
    format!(
        "{}/crates/codex-services/tests/fixtures/idp_bearer/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// Start a local IdP stub serving discovery + a static JWKS; returns its URL.
async fn start_idp_stub() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let url = format!("http://{}", listener.local_addr().unwrap());

    let issuer = url.clone();
    let jwks: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixture_path("jwks_key_a.json")).expect("jwks fixture"),
    )
    .expect("jwks fixture parses");

    let app = Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(move || {
                let issuer = issuer.clone();
                async move {
                    Json(serde_json::json!({
                        "issuer": issuer,
                        "jwks_uri": format!("{issuer}/jwks"),
                    }))
                }
            }),
        )
        .route("/jwks", get(move || async move { Json(jwks.clone()) }));

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("stub IdP serves");
    });

    url
}

/// Sign an RS256 token with one of the fixture keys.
fn sign_idp_token(kid: &str, key: &str, claims: &serde_json::Value) -> String {
    let pem = std::fs::read(fixture_path(&format!("{key}.pem"))).expect("test key fixture");
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_owned());
    encode(
        &header,
        claims,
        &EncodingKey::from_rsa_pem(&pem).expect("test key parses"),
    )
    .expect("token signs")
}

fn idp_claims(iss: &str, sub: &str, exp_offset_secs: i64) -> serde_json::Value {
    serde_json::json!({
        "iss": iss,
        "aud": "codex-client",
        "exp": chrono::Utc::now().timestamp() + exp_offset_secs,
        "sub": sub,
    })
}

fn oidc_config_for(idp_url: &str) -> OidcConfig {
    OidcConfig {
        enabled: true,
        auto_create_users: false,
        default_role: OidcDefaultRole::Reader,
        redirect_uri_base: None,
        providers: HashMap::from([(
            "authentik".to_string(),
            OidcProviderConfig {
                display_name: "Authentik".to_string(),
                issuer_url: idp_url.to_string(),
                client_id: "codex-client".to_string(),
                client_secret: None,
                client_secret_env: None,
                scopes: vec![],
                role_mapping: HashMap::new(),
                groups_claim: "groups".to_string(),
                username_claim: "preferred_username".to_string(),
                email_claim: "email".to_string(),
                accepted_audiences: vec![],
            },
        )]),
    }
}

/// AppState with the IdP bearer path configured against the stub.
async fn create_state_with_idp_bearer(db: DatabaseConnection, idp_url: &str) -> Arc<AppState> {
    let base = create_test_app_state(db).await;
    Arc::new(AppState {
        idp_bearer: Some(Arc::new(IdpBearerAuth::new(&oidc_config_for(idp_url)))),
        ..(*base).clone()
    })
}

/// Seed a user and link it to the authentik identity `subject`.
async fn seed_linked_user(db: &DatabaseConnection, subject: &str, admin: bool) -> users::Model {
    let user = create_test_user(
        &format!("idp-{subject}"),
        &format!("{subject}@example.com"),
        "oidc:placeholder",
        admin,
    );
    UserRepository::create(db, &user).await.unwrap();

    let connection = oidc_connections::Model {
        id: uuid::Uuid::new_v4(),
        user_id: user.id,
        provider_name: "authentik".to_string(),
        subject: subject.to_string(),
        email: Some(user.email.clone()),
        display_name: None,
        groups: None,
        access_token_hash: None,
        refresh_token_encrypted: None,
        token_expires_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        last_used_at: None,
    };
    OidcConnectionRepository::create(db, &connection)
        .await
        .unwrap();

    user
}

#[tokio::test]
async fn valid_idp_token_with_linked_connection_resolves_on_me() {
    let idp_url = start_idp_stub().await;
    let (db, _temp_dir) = setup_test_db().await;
    let user = seed_linked_user(&db, "sub-linked", false).await;

    let state = create_state_with_idp_bearer(db, &idp_url).await;
    let app = create_test_router_with_app_state(state);

    let token = sign_idp_token("key-a", "key_a", &idp_claims(&idp_url, "sub-linked", 3600));
    let request = get_request_with_auth("/api/v1/auth/me", &token);
    let (status, body): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    let body = body.unwrap_or_default();

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["username"], user.username.as_str());
    assert_eq!(body["id"], user.id.to_string().as_str());
}

#[tokio::test]
async fn valid_idp_token_works_on_authcontext_endpoints() {
    let idp_url = start_idp_stub().await;
    let (db, _temp_dir) = setup_test_db().await;
    // /api/v1/api-keys takes the strict `AuthContext` extractor
    // (versus `/me`'s FlexibleAuthContext), covering the second dispatch site.
    seed_linked_user(&db, "sub-admin", true).await;

    let state = create_state_with_idp_bearer(db, &idp_url).await;
    let app = create_test_router_with_app_state(state);

    let token = sign_idp_token("key-a", "key_a", &idp_claims(&idp_url, "sub-admin", 3600));
    let request = get_request_with_auth("/api/v1/api-keys", &token);
    let (status, body): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    let body = body.unwrap_or_default();

    assert_eq!(status, StatusCode::OK, "body: {body}");
}

#[tokio::test]
async fn unlinked_identity_gets_the_link_account_401() {
    let idp_url = start_idp_stub().await;
    let (db, _temp_dir) = setup_test_db().await;
    // No user, no connection: the token is valid but unknown to Codex.

    let state = create_state_with_idp_bearer(db, &idp_url).await;
    let app = create_test_router_with_app_state(state);

    let token = sign_idp_token(
        "key-a",
        "key_a",
        &idp_claims(&idp_url, "sub-stranger", 3600),
    );
    let request = get_request_with_auth("/api/v1/auth/me", &token);
    let (status, body): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    let body = body.unwrap_or_default();

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let message = body["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("sign in to Codex via SSO"),
        "the 401 must tell the user how to link, got: {message}"
    );
}

#[tokio::test]
async fn tampered_idp_tokens_get_a_uniform_401() {
    let idp_url = start_idp_stub().await;
    let (db, _temp_dir) = setup_test_db().await;
    seed_linked_user(&db, "sub-linked", false).await;

    let state = create_state_with_idp_bearer(db, &idp_url).await;
    let app = create_test_router_with_app_state(state);

    let mut wrong_aud = idp_claims(&idp_url, "sub-linked", 3600);
    wrong_aud["aud"] = serde_json::json!("not-codex");

    let cases = [
        (
            "wrong issuer",
            sign_idp_token(
                "key-a",
                "key_a",
                &idp_claims("https://evil.example.com", "sub-linked", 3600),
            ),
        ),
        (
            "wrong audience",
            sign_idp_token("key-a", "key_a", &wrong_aud),
        ),
        (
            "expired",
            sign_idp_token("key-a", "key_a", &idp_claims(&idp_url, "sub-linked", -3600)),
        ),
        (
            // Signed by key B but claiming key A's kid.
            "bad signature",
            sign_idp_token("key-a", "key_b", &idp_claims(&idp_url, "sub-linked", 3600)),
        ),
    ];

    for (case, token) in cases {
        let request = get_request_with_auth("/api/v1/auth/me", &token);
        let (status, body): (StatusCode, Option<serde_json::Value>) =
            make_json_request(app.clone(), request).await;
        let body = body.unwrap_or_default();

        assert_eq!(status, StatusCode::UNAUTHORIZED, "case {case}: {body}");
        let message = body["message"].as_str().unwrap_or_default();
        assert_eq!(
            message, "Invalid bearer token",
            "case {case}: the response must not leak the validation internals"
        );
    }
}

#[tokio::test]
async fn rs256_token_without_validator_keeps_todays_behavior() {
    let idp_url = start_idp_stub().await;
    let (db, _temp_dir) = setup_test_db().await;
    seed_linked_user(&db, "sub-linked", false).await;

    // OIDC disabled: AppState carries no validator, so the RS256 token
    // falls through to the local JWT path and fails exactly like today.
    let state = create_test_app_state(db).await;
    let app = create_test_router_with_app_state(state);

    let token = sign_idp_token("key-a", "key_a", &idp_claims(&idp_url, "sub-linked", 3600));
    let request = get_request_with_auth("/api/v1/auth/me", &token);
    let (status, body): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    let body = body.unwrap_or_default();

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let message = body["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("Invalid JWT token"),
        "expected the pre-existing local-JWT error, got: {message}"
    );
}

#[tokio::test]
async fn hs256_session_jwt_still_works_with_validator_configured() {
    let idp_url = start_idp_stub().await;
    let (db, _temp_dir) = setup_test_db().await;

    let user = create_test_user("local-user", "local@example.com", "hash", false);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_state_with_idp_bearer(db, &idp_url).await;
    let token = generate_test_token(&state, &user);
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth("/api/v1/auth/me", &token);
    let (status, body): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    let body = body.unwrap_or_default();

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["username"], "local-user");
}

#[tokio::test]
async fn basic_auth_still_works_with_validator_configured() {
    let idp_url = start_idp_stub().await;
    let (db, _temp_dir) = setup_test_db().await;

    let password_hash = codex::utils::password::hash_password("hunter2").unwrap();
    let user = create_test_user("basic-user", "basic@example.com", &password_hash, false);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_state_with_idp_bearer(db, &idp_url).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_basic_auth("/api/v1/auth/me", "basic-user", "hunter2");
    let (status, body): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    let body = body.unwrap_or_default();

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["username"], "basic-user");
}

#[tokio::test]
async fn inactive_linked_user_is_rejected_like_the_local_path() {
    let idp_url = start_idp_stub().await;
    let (db, _temp_dir) = setup_test_db().await;

    let mut user = create_test_user(
        "idp-inactive",
        "inactive@example.com",
        "oidc:placeholder",
        false,
    );
    user.is_active = false;
    UserRepository::create(&db, &user).await.unwrap();

    let connection = oidc_connections::Model {
        id: uuid::Uuid::new_v4(),
        user_id: user.id,
        provider_name: "authentik".to_string(),
        subject: "sub-inactive".to_string(),
        email: Some(user.email.clone()),
        display_name: None,
        groups: None,
        access_token_hash: None,
        refresh_token_encrypted: None,
        token_expires_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        last_used_at: None,
    };
    OidcConnectionRepository::create(&db, &connection)
        .await
        .unwrap();

    let state = create_state_with_idp_bearer(db, &idp_url).await;
    let app = create_test_router_with_app_state(state);

    let token = sign_idp_token(
        "key-a",
        "key_a",
        &idp_claims(&idp_url, "sub-inactive", 3600),
    );
    let request = get_request_with_auth("/api/v1/auth/me", &token);
    let (status, body): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    let body = body.unwrap_or_default();

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let message = body["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("inactive"),
        "inactive users must be rejected identically to the local path, got: {message}"
    );
}

#[tokio::test]
async fn unreachable_idp_yields_503_not_401() {
    let (db, _temp_dir) = setup_test_db().await;
    seed_linked_user(&db, "sub-linked", false).await;

    // Validator configured against a dead port: the token cannot be
    // checked, which is the IdP's failure, not the caller's.
    let state = create_state_with_idp_bearer(db, "http://127.0.0.1:1").await;
    let app = create_test_router_with_app_state(state);

    let token = sign_idp_token(
        "key-a",
        "key_a",
        &idp_claims("http://127.0.0.1:1", "sub-linked", 3600),
    );
    let request = get_request_with_auth("/api/v1/auth/me", &token);
    let (status, _body): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}
