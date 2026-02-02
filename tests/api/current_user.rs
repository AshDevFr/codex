//! Integration tests for current user endpoint (GET /api/v1/user)

#[path = "../common/mod.rs"]
mod common;

use codex::api::routes::v1::dto::UserDetailDto;
use codex::db::repositories::UserRepository;
use codex::utils::password;
use common::*;
use hyper::StatusCode;

// Helper to create user and token
async fn create_user_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
    username: &str,
    is_admin: bool,
) -> (uuid::Uuid, String) {
    let password_hash = password::hash_password("password123").unwrap();
    let user = create_test_user(
        username,
        &format!("{}@example.com", username),
        &password_hash,
        is_admin,
    );
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

// ============================================================================
// Get Current User Tests
// ============================================================================

#[tokio::test]
async fn test_get_current_user_success() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "testuser", false).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/user", &token);
    let (status, response): (StatusCode, Option<UserDetailDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let user = response.unwrap();
    assert_eq!(user.id, user_id);
    assert_eq!(user.username, "testuser");
    assert_eq!(user.email, "testuser@example.com");
    assert!(user.is_active);
    assert!(user.sharing_tags.is_empty());
}

#[tokio::test]
async fn test_get_current_user_admin() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "adminuser", true).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/user", &token);
    let (status, response): (StatusCode, Option<UserDetailDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let user = response.unwrap();
    assert_eq!(user.id, user_id);
    assert_eq!(user.username, "adminuser");
    assert_eq!(user.role.to_string(), "admin");
}

#[tokio::test]
async fn test_get_current_user_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // Request without auth token
    let request = get_request("/api/v1/user");
    let (status, _): (StatusCode, Option<UserDetailDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_current_user_invalid_token() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // Request with invalid token
    let request = get_request_with_auth("/api/v1/user", "invalid_token");
    let (status, _): (StatusCode, Option<UserDetailDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
