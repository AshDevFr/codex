//! Integration tests for series export endpoints

#[path = "../common/mod.rs"]
mod common;

use codex::db::repositories::UserRepository;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use serde_json::Value;

async fn create_user_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
    username: &str,
) -> (uuid::Uuid, String) {
    let password_hash = password::hash_password("password123").unwrap();
    let user = create_test_user(
        username,
        &format!("{username}@example.com"),
        &password_hash,
        false,
    );
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

// ============================================================================
// Field catalog
// ============================================================================

#[tokio::test]
async fn test_get_field_catalog() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "user1").await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/user/exports/series/fields", &token);
    let (status, response): (StatusCode, Option<Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    let fields = body["fields"].as_array().unwrap();
    assert!(!fields.is_empty());

    // Check a known field
    let title_field = fields.iter().find(|f| f["key"] == "title").unwrap();
    assert_eq!(title_field["label"], "Title");
    assert_eq!(title_field["multiValue"], false);
    assert_eq!(title_field["userSpecific"], false);

    // Check a multi-value field
    let genres_field = fields.iter().find(|f| f["key"] == "genres").unwrap();
    assert_eq!(genres_field["multiValue"], true);

    // Check a user-specific field
    let rating_field = fields.iter().find(|f| f["key"] == "user_rating").unwrap();
    assert_eq!(rating_field["userSpecific"], true);
}

// ============================================================================
// Create export
// ============================================================================

#[tokio::test]
async fn test_create_export_success() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "user1").await;
    let app = create_test_router(state).await;

    let body = serde_json::json!({
        "format": "json",
        "libraryIds": ["550e8400-e29b-41d4-a716-446655440000"],
        "fields": ["title", "genres", "user_rating"]
    });

    let request =
        post_request_with_auth_json("/api/v1/user/exports/series", &token, &body.to_string());
    let (status, response): (StatusCode, Option<Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::ACCEPTED);
    let export = response.unwrap();
    assert_eq!(export["format"], "json");
    assert_eq!(export["status"], "pending");
    assert!(export["id"].is_string());
}

#[tokio::test]
async fn test_create_export_invalid_format() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "user1").await;
    let app = create_test_router(state).await;

    let body = serde_json::json!({
        "format": "xml",
        "libraryIds": ["550e8400-e29b-41d4-a716-446655440000"],
        "fields": ["title"]
    });

    let request =
        post_request_with_auth_json("/api/v1/user/exports/series", &token, &body.to_string());
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_export_invalid_field() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "user1").await;
    let app = create_test_router(state).await;

    let body = serde_json::json!({
        "format": "json",
        "libraryIds": ["550e8400-e29b-41d4-a716-446655440000"],
        "fields": ["title", "nonexistent_field"]
    });

    let request =
        post_request_with_auth_json("/api/v1/user/exports/series", &token, &body.to_string());
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_export_empty_libraries() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "user1").await;
    let app = create_test_router(state).await;

    let body = serde_json::json!({
        "format": "csv",
        "libraryIds": [],
        "fields": ["title"]
    });

    let request =
        post_request_with_auth_json("/api/v1/user/exports/series", &token, &body.to_string());
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ============================================================================
// List exports
// ============================================================================

#[tokio::test]
async fn test_list_exports_empty() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "user1").await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/user/exports/series", &token);
    let (status, response): (StatusCode, Option<Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body["exports"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_list_exports_shows_own_only() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token1) = create_user_and_token(&db, &state, "user1").await;
    let (_, token2) = create_user_and_token(&db, &state, "user2").await;

    // User 1 creates an export
    let body = serde_json::json!({
        "format": "json",
        "libraryIds": ["550e8400-e29b-41d4-a716-446655440000"],
        "fields": ["title"]
    });

    let app = create_test_router(state.clone()).await;
    let request =
        post_request_with_auth_json("/api/v1/user/exports/series", &token1, &body.to_string());
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::ACCEPTED);

    // User 2 should see empty list
    let app = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/user/exports/series", &token2);
    let (status, response): (StatusCode, Option<Value>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.unwrap()["exports"].as_array().unwrap().len(), 0);
}

// ============================================================================
// Get / Delete export
// ============================================================================

#[tokio::test]
async fn test_get_export_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "user1").await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        "/api/v1/user/exports/series/550e8400-e29b-41d4-a716-446655440000",
        &token,
    );
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_export_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "user1").await;
    let app = create_test_router(state).await;

    let request = delete_request_with_auth(
        "/api/v1/user/exports/series/550e8400-e29b-41d4-a716-446655440000",
        &token,
    );
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_and_get_export() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "user1").await;

    // Create
    let body = serde_json::json!({
        "format": "csv",
        "libraryIds": ["550e8400-e29b-41d4-a716-446655440000"],
        "fields": ["title", "summary"]
    });

    let app = create_test_router(state.clone()).await;
    let request =
        post_request_with_auth_json("/api/v1/user/exports/series", &token, &body.to_string());
    let (status, response): (StatusCode, Option<Value>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let export_id = response.unwrap()["id"].as_str().unwrap().to_string();

    // Get
    let app = create_test_router(state.clone()).await;
    let request =
        get_request_with_auth(&format!("/api/v1/user/exports/series/{export_id}"), &token);
    let (status, response): (StatusCode, Option<Value>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let export = response.unwrap();
    assert_eq!(export["id"], export_id);
    assert_eq!(export["format"], "csv");
    assert_eq!(export["status"], "pending");

    // Delete
    let app = create_test_router(state).await;
    let request =
        delete_request_with_auth(&format!("/api/v1/user/exports/series/{export_id}"), &token);
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

// ============================================================================
// Auth required
// ============================================================================

#[tokio::test]
async fn test_exports_require_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let request = get_request("/api/v1/user/exports/series");
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Cross-user isolation
// ============================================================================

#[tokio::test]
async fn test_cannot_access_other_users_export() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token1) = create_user_and_token(&db, &state, "user1").await;
    let (_, token2) = create_user_and_token(&db, &state, "user2").await;

    // User 1 creates an export
    let body = serde_json::json!({
        "format": "json",
        "libraryIds": ["550e8400-e29b-41d4-a716-446655440000"],
        "fields": ["title"]
    });

    let app = create_test_router(state.clone()).await;
    let request =
        post_request_with_auth_json("/api/v1/user/exports/series", &token1, &body.to_string());
    let (_, response): (StatusCode, Option<Value>) = make_json_request(app, request).await;
    let export_id = response.unwrap()["id"].as_str().unwrap().to_string();

    // User 2 cannot access it
    let app = create_test_router(state).await;
    let request =
        get_request_with_auth(&format!("/api/v1/user/exports/series/{export_id}"), &token2);
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
