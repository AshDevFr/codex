//! Wire-format tests for permission strings on the write paths.
//!
//! Permissions travel in three formats: the canonical serde kebab-case name
//! (`"read-lists-read"`, `"api-keys-read"`) served and stored by the API,
//! the colon display format (`"readlists:read"`), and the legacy dash form
//! older frontends sent (`"readlists-read"`). The endpoints that accept
//! permission strings must take all three, and must store the canonical
//! form so the auth extractor can always parse what it loads.

#[path = "../common/mod.rs"]
mod common;

use codex::api::routes::v1::dto::{CreateApiKeyRequest, CreateApiKeyResponse};
use codex::db::repositories::UserRepository;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use serde_json::json;

async fn admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> (uuid::Uuid, String) {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

#[tokio::test]
async fn api_key_creation_accepts_every_wire_format() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_admin_id, token) = admin_and_token(&db, &state).await;

    for (label, permission) in [
        ("canonical kebab", "api-keys-read"),
        ("canonical multi-word", "read-lists-read"),
        ("colon", "readlists:read"),
        ("legacy dash", "readlists-read"),
        ("plugins manage", "plugins-manage"),
    ] {
        let app = create_test_router(state.clone()).await;
        let create_request = CreateApiKeyRequest {
            name: format!("key {label}"),
            permissions: Some(vec![permission.to_string()]),
            expires_at: None,
        };
        let request = post_json_request_with_auth("/api/v1/api-keys", &create_request, &token);
        let (status, response): (StatusCode, Option<CreateApiKeyResponse>) =
            make_json_request(app, request).await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "creating a key with {label} permission {permission} must succeed"
        );

        // Whatever came in, the stored form is the canonical serde name,
        // which is the only form the auth extractor can load back.
        let stored: Vec<String> =
            serde_json::from_value(response.unwrap().api_key.permissions.clone()).unwrap();
        for perm in &stored {
            assert!(
                !perm.contains(':'),
                "{label}: stored permission {perm} must be the canonical serde name"
            );
        }
    }
}

#[tokio::test]
async fn user_update_accepts_every_wire_format_and_stores_canonically() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_admin_id, token) = admin_and_token(&db, &state).await;

    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user("formatuser", "format@example.com", &password_hash, false);
    let user = UserRepository::create(&db, &user).await.unwrap();

    for (label, permission) in [
        ("canonical multi-word", "read-lists-read"),
        ("colon", "api-keys:read"),
        ("legacy dash", "readlists-read"),
    ] {
        let app = create_test_router(state.clone()).await;
        let body = json!({"permissions": [permission]});
        let request =
            patch_json_request_with_auth(&format!("/api/v1/users/{}", user.id), &body, &token);
        let (status, _): (StatusCode, Option<serde_json::Value>) =
            make_json_request(app, request).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "updating a user with {label} permission {permission} must succeed"
        );

        // The stored value must be loadable by the auth extractor, which
        // deserializes it as Vec<Permission> (canonical names only). A
        // request as that user proves it end to end.
        let user_token = state
            .jwt_service
            .generate_token(user.id, user.username.clone(), user.get_role())
            .unwrap();
        let app = create_test_router(state.clone()).await;
        let request = get_request_with_auth("/api/v1/user", &user_token);
        let (status, _): (StatusCode, Option<serde_json::Value>) =
            make_json_request(app, request).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{label}: the stored permissions must not break authentication"
        );
    }
}
