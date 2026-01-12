#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::library::{CreateLibraryRequest, LibraryDto, UpdateLibraryRequest};
use codex::api::error::ErrorResponse;
use codex::api::permissions::{serialize_permissions, Permission};
use codex::db::repositories::{
    ApiKeyRepository, BookRepository, LibraryRepository, SeriesRepository, UserRepository,
};
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

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

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
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

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

    let permissions_json = serialize_permissions(&permissions);
    let api_key = create_test_api_key(
        created_user.id,
        "Test Key",
        &key_hash,
        "codex_test123",
        serde_json::from_str(&permissions_json).unwrap(),
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

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

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

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

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
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

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
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Create a temp directory for the library path
    let lib_path = temp_dir.path().join("new_library");
    std::fs::create_dir(&lib_path).unwrap();

    let create_request = CreateLibraryRequest {
        name: "New Library".to_string(),
        path: lib_path.to_str().unwrap().to_string(),
        description: None,
        series_strategy: None,
        series_config: None,
        book_strategy: None,
        book_config: None,
        scanning_config: None,
        scan_immediately: false,
        allowed_formats: None,
        excluded_patterns: None,
        default_reading_direction: None,
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
    let state = create_test_auth_state(db.clone()).await;
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let create_request = CreateLibraryRequest {
        name: "New Library".to_string(),
        path: "/new/path".to_string(),
        description: None,
        series_strategy: None,
        series_config: None,
        book_strategy: None,
        book_config: None,
        scan_immediately: false,
        scanning_config: None,
        allowed_formats: None,
        excluded_patterns: None,
        default_reading_direction: None,
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

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let update_request = UpdateLibraryRequest {
        name: Some("Updated Name".to_string()),
        path: Some(updated_path.to_str().unwrap().to_string()),
        description: None,
        is_active: None,
        scanning_config: None,
        allowed_formats: None,
        excluded_patterns: None,
        default_reading_direction: None,
    };

    let request = patch_json_request_with_auth(
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

    let state = create_test_auth_state(db.clone()).await;
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let update_request = UpdateLibraryRequest {
        name: Some("Updated".to_string()),
        path: None,
        description: None,
        is_active: None,
        scanning_config: None,
        allowed_formats: None,
        excluded_patterns: None,
        default_reading_direction: None,
    };

    let request = patch_json_request_with_auth(
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

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

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

    let state = create_test_auth_state(db.clone()).await;
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router(state).await;

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
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = delete_request_with_auth(&format!("/api/v1/libraries/{}", fake_id), &token);
    let (status, _) = make_request(app, request).await;

    // Note: Delete currently returns 200 OK even if resource doesn't exist
    // This is a design choice - idempotent deletes
    assert_eq!(status, StatusCode::OK);
}

// ============================================================================
// Library Counts Tests (Book/Series Statistics)
// ============================================================================

#[tokio::test]
async fn test_library_includes_book_and_series_counts() {
    use codex::scanner::ScanMode;

    let (db, temp_dir) = setup_test_db().await;

    // Create test files in the library
    create_test_cbz_files_in_dir(temp_dir.path());

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Scan the library to populate data
    let state = create_test_app_state(db.clone()).await;
    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Wait for scan to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    // Get library details
    let request = get_request_with_auth(&format!("/api/v1/libraries/{}", library.id), &token);
    let (status, response): (StatusCode, Option<LibraryDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let library_dto = response.unwrap();

    // Verify counts are present
    assert!(
        library_dto.book_count.is_some(),
        "Library should include book_count"
    );
    assert!(
        library_dto.series_count.is_some(),
        "Library should include series_count"
    );

    println!(
        "Library stats - Books: {}, Series: {}",
        library_dto.book_count.unwrap(),
        library_dto.series_count.unwrap()
    );
}

#[tokio::test]
async fn test_empty_library_has_zero_counts() {
    let (db, temp_dir) = setup_test_db().await;

    // Create empty library (no files)
    let library = LibraryRepository::create(
        &db,
        "Empty Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth(&format!("/api/v1/libraries/{}", library.id), &token);
    let (status, response): (StatusCode, Option<LibraryDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let library_dto = response.unwrap();

    // Empty library should have zero counts
    assert_eq!(library_dto.book_count, Some(0));
    assert_eq!(library_dto.series_count, Some(0));
}

#[tokio::test]
async fn test_list_libraries_includes_counts() {
    use codex::scanner::ScanMode;

    let (db, temp_dir) = setup_test_db().await;

    // Create test files
    create_test_cbz_files_in_dir(temp_dir.path());

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Scan the library
    let state = create_test_app_state(db.clone()).await;
    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Wait for scan to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    // List libraries
    let request = get_request_with_auth("/api/v1/libraries", &token);
    let (status, response): (StatusCode, Option<Vec<LibraryDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let libraries = response.unwrap();

    assert!(!libraries.is_empty());

    // Find our library and verify counts
    let library_dto = libraries
        .iter()
        .find(|lib| lib.id == library.id)
        .expect("Library should be in list");

    assert!(
        library_dto.book_count.is_some(),
        "Listed library should include book_count"
    );
    assert!(
        library_dto.series_count.is_some(),
        "Listed library should include series_count"
    );
}

#[tokio::test]
async fn test_book_count_excludes_deleted_books() {
    use codex::scanner::ScanMode;

    let (db, temp_dir) = setup_test_db().await;

    // Create test files
    create_test_cbz_files_in_dir(temp_dir.path());

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Scan the library
    let state = create_test_app_state(db.clone()).await;
    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Wait for scan to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Get initial counts
    let initial_book_count = BookRepository::count_by_library(&db, library.id)
        .await
        .unwrap();

    if initial_book_count > 0 {
        // Soft delete one book
        let series_list = SeriesRepository::list_by_library(&db, library.id)
            .await
            .unwrap();
        if !series_list.is_empty() {
            let books = BookRepository::list_by_series(&db, series_list[0].id, false)
                .await
                .unwrap();
            if !books.is_empty() {
                BookRepository::mark_deleted(&db, books[0].id, true, None)
                    .await
                    .unwrap();

                // Get counts after soft delete
                let count_after_delete = BookRepository::count_by_library(&db, library.id)
                    .await
                    .unwrap();

                // Count should be reduced by 1
                assert_eq!(
                    count_after_delete,
                    initial_book_count - 1,
                    "Deleted books should not be counted"
                );
            }
        }
    }
}

#[tokio::test]
async fn test_series_count_accuracy() {
    use codex::db::repositories::SeriesRepository;
    use codex::scanner::ScanMode;

    let (db, temp_dir) = setup_test_db().await;

    // Create test files
    create_test_cbz_files_in_dir(temp_dir.path());

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Scan the library
    let state = create_test_app_state(db.clone()).await;
    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Wait for scan to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Get series count from repository
    let series_count = SeriesRepository::count_by_library(&db, library.id)
        .await
        .unwrap();

    // Get library via API
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth(&format!("/api/v1/libraries/{}", library.id), &token);
    let (status, response): (StatusCode, Option<LibraryDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let library_dto = response.unwrap();

    // Verify API count matches repository count
    assert_eq!(
        library_dto.series_count,
        Some(series_count),
        "API series count should match repository count"
    );
}

// ============================================================================
// Scheduler Reload Tests
// ============================================================================

#[tokio::test]
async fn test_create_library_with_scheduler_succeeds_without_scheduler() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // Create temporary directory for library
    let lib_path = temp_dir.path().join("test_library");
    std::fs::create_dir_all(&lib_path).unwrap();

    let request_body = CreateLibraryRequest {
        name: "Test Library".to_string(),
        path: lib_path.to_string_lossy().to_string(),
        description: None,
        series_strategy: None,
        series_config: None,
        book_strategy: None,
        book_config: None,
        allowed_formats: None,
        excluded_patterns: None,
        scanning_config: None,
        scan_immediately: false,
        default_reading_direction: None,
    };

    let request = post_json_request_with_auth("/api/v1/libraries", &request_body, &token);
    let (status, response): (StatusCode, Option<LibraryDto>) =
        make_json_request(app, request).await;

    // Should succeed even without scheduler (graceful degradation)
    assert_eq!(status, StatusCode::OK);
    assert!(response.is_some());
    let library = response.unwrap();
    assert_eq!(library.name, "Test Library");
}

#[tokio::test]
async fn test_update_library_with_scheduler_succeeds_without_scheduler() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // Create temporary directory for library
    let lib_path = temp_dir.path().join("test_library");
    std::fs::create_dir_all(&lib_path).unwrap();

    // Create a library
    let library = LibraryRepository::create(
        &db,
        "Original Name",
        lib_path.to_string_lossy().as_ref(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // Update the library
    let request_body = UpdateLibraryRequest {
        name: Some("Updated Name".to_string()),
        path: None,
        description: None,
        is_active: None,
        allowed_formats: None,
        excluded_patterns: None,
        scanning_config: None,
        default_reading_direction: None,
    };

    let request = patch_json_request_with_auth(
        &format!("/api/v1/libraries/{}", library.id),
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<LibraryDto>) =
        make_json_request(app, request).await;

    // Should succeed even without scheduler (graceful degradation)
    assert_eq!(status, StatusCode::OK);
    assert!(response.is_some());
    let updated_library = response.unwrap();
    assert_eq!(updated_library.name, "Updated Name");
}

#[tokio::test]
async fn test_delete_library_with_scheduler_succeeds_without_scheduler() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // Create a library
    let library = LibraryRepository::create(&db, "To Delete", "/path", ScanningStrategy::Default)
        .await
        .unwrap();

    // Delete the library
    let request = delete_request_with_auth(&format!("/api/v1/libraries/{}", library.id), &token);
    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;

    // Should succeed even without scheduler (graceful degradation)
    assert_eq!(status, StatusCode::OK);

    // Verify library is deleted
    let deleted = LibraryRepository::get_by_id(&db, library.id).await.unwrap();
    assert!(deleted.is_none());
}
