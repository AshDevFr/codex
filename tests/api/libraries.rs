#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::common::PaginatedResponse;
use codex::api::dto::library::{CreateLibraryRequest, LibraryDto, UpdateLibraryRequest};
use codex::api::error::ErrorResponse;
use codex::api::permissions::{serialize_permissions, Permission, READONLY_PERMISSIONS};
use codex::db::repositories::{ApiKeyRepository, LibraryRepository, UserRepository};
use codex::db::ScanningStrategy;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use std::collections::HashSet;

// Helper to create an admin user and get a token
async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();

    state
        .jwt_service
        .generate_token(created.id, created.username, created.is_admin)
        .unwrap()
}

// Helper to create a readonly user and get a token
async fn create_readonly_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> String {
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user("readonly", "readonly@example.com", &password_hash, false);
    let created = UserRepository::create(db, &user).await.unwrap();

    state
        .jwt_service
        .generate_token(created.id, created.username, created.is_admin)
        .unwrap()
}

// ============================================================================
// List Libraries Tests
// ============================================================================

#[tokio::test]
async fn test_list_libraries_with_auth() {
    let (db, temp_dir) = setup_test_db().await;

    // Create some test libraries
    LibraryRepository::create(&db, "Library 1", "/path1", ScanningStrategy::Default)
        .await
        .unwrap();
    LibraryRepository::create(&db, "Library 2", "/path2", ScanningStrategy::Default)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state);

    let request = get_request_with_auth("/api/v1/libraries", &token);
    let (status, response): (StatusCode, Option<Vec<LibraryDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let libraries = response.unwrap();
    assert_eq!(libraries.len(), 2);
    assert_eq!(libraries[0].name, "Library 1");
    assert_eq!(libraries[1].name, "Library 2");
}

#[tokio::test]
async fn test_list_libraries_without_auth() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db);
    let app = create_test_router(state);

    let request = get_request("/api/v1/libraries");
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_list_libraries_with_api_key() {
    let (db, temp_dir) = setup_test_db().await;

    // Create user and API key with readonly permissions
    let user = create_test_user("apiuser", "api@example.com", "hash", false);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let plain_key = "codex_test123_key456";
    let key_hash = password::hash_password(plain_key).unwrap();

    let mut permissions = HashSet::new();
    permissions.insert(Permission::LibrariesRead);

    let api_key = create_test_api_key(
        created_user.id,
        "Test Key",
        &key_hash,
        "codex_test123",
        serialize_permissions(&permissions),
    );
    ApiKeyRepository::create(&db, &api_key).await.unwrap();

    // Create a library
    let lib_path = temp_dir.path().join("api_test_lib");
    std::fs::create_dir(&lib_path).unwrap();
    LibraryRepository::create(
        &db,
        "API Test Library",
        lib_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db);
    let app = create_test_router(state);

    let request = get_request_with_api_key("/api/v1/libraries", plain_key);
    let (status, response): (StatusCode, Option<Vec<LibraryDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let libraries = response.unwrap();
    assert_eq!(libraries.len(), 1);
}

// ============================================================================
// Get Library by ID Tests
// ============================================================================

#[tokio::test]
async fn test_get_library_by_id() {
    let (db, temp_dir) = setup_test_db().await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();

    let state = create_test_auth_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state);

    let request = get_request_with_auth(&format!("/api/v1/libraries/{}", library.id), &token);
    let (status, response): (StatusCode, Option<LibraryDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let lib = response.unwrap();
    assert_eq!(lib.id, library.id);
    assert_eq!(lib.name, "Test Library");
    assert_eq!(lib.path, "/test/path");
}

#[tokio::test]
async fn test_get_library_not_found() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state);

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/libraries/{}", fake_id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert_eq!(error.error, "NotFound");
}

// ============================================================================
// Create Library Tests
// ============================================================================

#[tokio::test]
async fn test_create_library_success() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state);

    // Create a temp directory for the library path
    let lib_path = temp_dir.path().join("new_library");
    std::fs::create_dir(&lib_path).unwrap();

    let create_request = CreateLibraryRequest {
        name: "New Library".to_string(),
        path: lib_path.to_str().unwrap().to_string(),
        description: None,
    };

    let request = post_json_request_with_auth("/api/v1/libraries", &create_request, &token);
    let (status, response): (StatusCode, Option<LibraryDto>) =
        make_json_request(app, request).await;

    // Handler currently returns 200 OK instead of 201 CREATED
    assert_eq!(status, StatusCode::OK);
    let library = response.unwrap();
    assert_eq!(library.name, "New Library");
    assert_eq!(library.path, lib_path.to_str().unwrap());
}

#[tokio::test]
async fn test_create_library_without_permission() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone());
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router(state);

    let create_request = CreateLibraryRequest {
        name: "New Library".to_string(),
        path: "/new/path".to_string(),
        description: None,
    };

    let request = post_json_request_with_auth("/api/v1/libraries", &create_request, &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    let error = response.unwrap();
    assert_eq!(error.error, "Forbidden");
}

// ============================================================================
// Update Library Tests
// ============================================================================

#[tokio::test]
async fn test_update_library_success() {
    let (db, temp_dir) = setup_test_db().await;

    // Create temp directories
    let original_path = temp_dir.path().join("original");
    std::fs::create_dir(&original_path).unwrap();
    let updated_path = temp_dir.path().join("updated");
    std::fs::create_dir(&updated_path).unwrap();

    let library = LibraryRepository::create(
        &db,
        "Original Name",
        original_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state);

    let update_request = UpdateLibraryRequest {
        name: Some("Updated Name".to_string()),
        path: Some(updated_path.to_str().unwrap().to_string()),
        description: None,
        is_active: None,
    };

    let request = put_json_request_with_auth(
        &format!("/api/v1/libraries/{}", library.id),
        &update_request,
        &token,
    );
    let (status, response): (StatusCode, Option<LibraryDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let updated = response.unwrap();
    assert_eq!(updated.name, "Updated Name");
    assert_eq!(updated.path, updated_path.to_str().unwrap());
}

#[tokio::test]
async fn test_update_library_without_permission() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/path", ScanningStrategy::Default)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone());
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router(state);

    let update_request = UpdateLibraryRequest {
        name: Some("Updated".to_string()),
        path: None,
        description: None,
        is_active: None,
    };

    let request = put_json_request_with_auth(
        &format!("/api/v1/libraries/{}", library.id),
        &update_request,
        &token,
    );
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    let error = response.unwrap();
    assert_eq!(error.error, "Forbidden");
}

// ============================================================================
// Delete Library Tests
// ============================================================================

#[tokio::test]
async fn test_delete_library_success() {
    let (db, temp_dir) = setup_test_db().await;

    let library =
        LibraryRepository::create(&db, "To Delete", "/delete/path", ScanningStrategy::Default)
            .await
            .unwrap();

    let state = create_test_auth_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state);

    let request = delete_request_with_auth(&format!("/api/v1/libraries/{}", library.id), &token);
    let (status, _) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Verify deleted
    let result = LibraryRepository::get_by_id(&db, library.id).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_delete_library_without_permission() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/path", ScanningStrategy::Default)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone());
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router(state);

    let request = delete_request_with_auth(&format!("/api/v1/libraries/{}", library.id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    let error = response.unwrap();
    assert_eq!(error.error, "Forbidden");
}

#[tokio::test]
async fn test_delete_library_not_found() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state);

    let fake_id = uuid::Uuid::new_v4();
    let request = delete_request_with_auth(&format!("/api/v1/libraries/{}", fake_id), &token);
    let (status, _) = make_request(app, request).await;

    // Note: Delete currently returns 200 OK even if resource doesn't exist
    // This is a design choice - idempotent deletes
    assert_eq!(status, StatusCode::OK);
}
