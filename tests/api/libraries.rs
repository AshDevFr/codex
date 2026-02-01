// Allow unused temp_dir - needed to keep TempDir alive but not always referenced
#![allow(unused_variables)]

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::permissions::{serialize_permissions, Permission};
use codex::api::routes::v1::dto::common::PaginatedResponse;
use codex::api::routes::v1::dto::library::{
    CreateLibraryRequest, LibraryDto, UpdateLibraryRequest,
};
use codex::api::routes::v1::dto::patch::PatchValue;
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
        .generate_token(created.id, created.username.clone(), created.get_role())
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
        .generate_token(created.id, created.username.clone(), created.get_role())
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
    let (status, response): (StatusCode, Option<PaginatedResponse<LibraryDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let paginated = response.unwrap();
    assert_eq!(paginated.data.len(), 2);
    assert_eq!(paginated.data[0].name, "Library 1");
    assert_eq!(paginated.data[1].name, "Library 2");
    assert_eq!(paginated.page, 1); // 1-indexed
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
    let (status, response): (StatusCode, Option<PaginatedResponse<LibraryDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let paginated = response.unwrap();
    assert_eq!(paginated.data.len(), 1);
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
        number_strategy: None,
        number_config: None,
        scanning_config: None,
        scan_immediately: false,
        allowed_formats: None,
        excluded_patterns: None,
        default_reading_direction: None,
        title_preprocessing_rules: None,
        auto_match_conditions: None,
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
        number_strategy: None,
        number_config: None,
        scan_immediately: false,
        scanning_config: None,
        allowed_formats: None,
        excluded_patterns: None,
        default_reading_direction: None,
        title_preprocessing_rules: None,
        auto_match_conditions: None,
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
        book_strategy: None,
        book_config: None,
        number_strategy: None,
        number_config: None,
        scanning_config: None,
        allowed_formats: None,
        excluded_patterns: None,
        default_reading_direction: None,
        title_preprocessing_rules: PatchValue::Absent,
        auto_match_conditions: PatchValue::Absent,
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
        book_strategy: None,
        book_config: None,
        number_strategy: None,
        number_config: None,
        scanning_config: None,
        allowed_formats: None,
        excluded_patterns: None,
        default_reading_direction: None,
        title_preprocessing_rules: PatchValue::Absent,
        auto_match_conditions: PatchValue::Absent,
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

/// Test that sending an empty array clears preprocessing rules
#[tokio::test]
async fn test_update_library_clears_preprocessing_rules() {
    let (db, temp_dir) = setup_test_db().await;

    // Create temp directory
    let library_path = temp_dir.path().join("library");
    std::fs::create_dir(&library_path).unwrap();

    // Create library with preprocessing rules
    let mut params = codex::db::repositories::CreateLibraryParams::new(
        "Test Library",
        library_path.to_str().unwrap(),
    );
    params.title_preprocessing_rules =
        Some(r#"[{"name":"Test Rule","pattern":"test","replacement":"replaced"}]"#.to_string());
    let library = LibraryRepository::create_with_params(&db, params)
        .await
        .unwrap();

    // Verify rules were set
    let initial_library = LibraryRepository::get_by_id(&db, library.id)
        .await
        .unwrap()
        .unwrap();
    assert!(initial_library.title_preprocessing_rules.is_some());

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Send empty array to clear rules (using PatchValue::Value with empty array)
    let update_request = UpdateLibraryRequest {
        name: None,
        path: None,
        description: None,
        is_active: None,
        book_strategy: None,
        book_config: None,
        number_strategy: None,
        number_config: None,
        scanning_config: None,
        allowed_formats: None,
        excluded_patterns: None,
        default_reading_direction: None,
        title_preprocessing_rules: PatchValue::Value(serde_json::json!([])), // Empty array
        auto_match_conditions: PatchValue::Absent,
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
    // Empty array or null should clear the rules
    assert!(
        updated.title_preprocessing_rules.is_none()
            || updated
                .title_preprocessing_rules
                .as_ref()
                .is_some_and(|v| v.as_array().is_some_and(|a| a.is_empty()))
    );

    // Verify in database
    let db_library = LibraryRepository::get_by_id(&db, library.id)
        .await
        .unwrap()
        .unwrap();
    assert!(db_library.title_preprocessing_rules.is_none());
}

/// Test that sending null clears auto-match conditions
#[tokio::test]
async fn test_update_library_clears_auto_match_conditions() {
    let (db, temp_dir) = setup_test_db().await;

    // Create temp directory
    let library_path = temp_dir.path().join("library");
    std::fs::create_dir(&library_path).unwrap();

    // Create library with auto-match conditions
    let mut params = codex::db::repositories::CreateLibraryParams::new(
        "Test Library",
        library_path.to_str().unwrap(),
    );
    params.auto_match_conditions = Some(
        r#"{"mode":"all","rules":[{"field":"book_count","operator":"gte","value":1}]}"#.to_string(),
    );
    let library = LibraryRepository::create_with_params(&db, params)
        .await
        .unwrap();

    // Verify conditions were set
    let initial_library = LibraryRepository::get_by_id(&db, library.id)
        .await
        .unwrap()
        .unwrap();
    assert!(initial_library.auto_match_conditions.is_some());

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Send null to clear conditions (using PatchValue::Null)
    let update_request = UpdateLibraryRequest {
        name: None,
        path: None,
        description: None,
        is_active: None,
        book_strategy: None,
        book_config: None,
        number_strategy: None,
        number_config: None,
        scanning_config: None,
        allowed_formats: None,
        excluded_patterns: None,
        default_reading_direction: None,
        title_preprocessing_rules: PatchValue::Absent,
        auto_match_conditions: PatchValue::Null, // Null to clear
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
    assert!(
        updated.auto_match_conditions.is_none(),
        "Expected auto_match_conditions to be None, got: {:?}",
        updated.auto_match_conditions
    );

    // Verify in database
    let db_library = LibraryRepository::get_by_id(&db, library.id)
        .await
        .unwrap()
        .unwrap();
    assert!(db_library.auto_match_conditions.is_none());
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
    let (status, response): (StatusCode, Option<PaginatedResponse<LibraryDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let paginated = response.unwrap();

    assert!(!paginated.data.is_empty());

    // Find our library and verify counts
    let library_dto = paginated
        .data
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
        number_strategy: None,
        number_config: None,
        allowed_formats: None,
        excluded_patterns: None,
        scanning_config: None,
        scan_immediately: false,
        default_reading_direction: None,
        title_preprocessing_rules: None,
        auto_match_conditions: None,
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
        book_strategy: None,
        book_config: None,
        number_strategy: None,
        number_config: None,
        allowed_formats: None,
        excluded_patterns: None,
        scanning_config: None,
        default_reading_direction: None,
        title_preprocessing_rules: PatchValue::Absent,
        auto_match_conditions: PatchValue::Absent,
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

// ============================================================================
// Reprocess Library Titles Tests
// ============================================================================

use codex::api::routes::v1::dto::series::{EnqueueReprocessTitleResponse, ReprocessTitleRequest};
use codex::db::repositories::{SeriesMetadataRepository, TaskRepository};
use codex::services::metadata::preprocessing::PreprocessingRule;
use codex::tasks::handlers::{ReprocessSeriesTitlesHandler, TaskHandler};

#[tokio::test]
async fn test_reprocess_library_titles_success() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library with preprocessing rules
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Add preprocessing rules to the library
    let rules = vec![PreprocessingRule::new(r"\s*\(Digital\)$", "")];
    let rules_json = serde_json::to_string(&rules).unwrap();

    use codex::db::entities::libraries;
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    let library_model = libraries::Entity::find_by_id(library.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut active: libraries::ActiveModel = library_model.into();
    active.title_preprocessing_rules = Set(Some(rules_json));
    active.update(&db).await.unwrap();

    // Create multiple series - some with "(Digital)" suffix, some without
    SeriesRepository::create(&db, library.id, "One Piece (Digital)", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Naruto (Digital)", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Death Note", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Reprocess all titles (enqueues a fan-out task)
    let request_body = ReprocessTitleRequest { dry_run: false };
    let request = post_json_request_with_auth(
        &format!("/api/v1/libraries/{}/series/titles/reprocess", library.id),
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<EnqueueReprocessTitleResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(result.success);
    assert_eq!(result.tasks_enqueued, 1);
    assert_eq!(result.task_ids.len(), 1);

    // Execute the fan-out task (which enqueues individual tasks)
    let task = TaskRepository::get_by_id(&db, result.task_ids[0])
        .await
        .unwrap()
        .unwrap();
    let handler = ReprocessSeriesTitlesHandler::new();
    let task_result = handler.handle(&task, &db, None).await.unwrap();

    // The fan-out task should have enqueued 3 individual tasks
    assert!(task_result.message.unwrap().contains("Enqueued 3"));
}

#[tokio::test]
async fn test_reprocess_library_titles_dry_run() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library with preprocessing rules
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let rules = vec![PreprocessingRule::new(r"\s*\(Digital\)$", "")];
    let rules_json = serde_json::to_string(&rules).unwrap();

    use codex::db::entities::libraries;
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    let library_model = libraries::Entity::find_by_id(library.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut active: libraries::ActiveModel = library_model.into();
    active.title_preprocessing_rules = Set(Some(rules_json));
    active.update(&db).await.unwrap();

    // Create series
    let series = SeriesRepository::create(&db, library.id, "Bleach (Digital)", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Dry run
    let request_body = ReprocessTitleRequest { dry_run: true };
    let request = post_json_request_with_auth(
        &format!("/api/v1/libraries/{}/series/titles/reprocess", library.id),
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<EnqueueReprocessTitleResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(result.success);
    assert_eq!(result.tasks_enqueued, 0); // No task enqueued for dry run
    assert!(result.message.contains("Dry run"));
    assert!(result.message.contains("1 would change")); // 1 series would change

    // Verify database was NOT updated
    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(metadata.title, "Bleach (Digital)"); // Should still have the original title
}

#[tokio::test]
async fn test_reprocess_library_titles_with_locked_series() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library with preprocessing rules
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let rules = vec![PreprocessingRule::new(r"\s*\(Digital\)$", "")];
    let rules_json = serde_json::to_string(&rules).unwrap();

    use codex::db::entities::libraries;
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    let library_model = libraries::Entity::find_by_id(library.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut active: libraries::ActiveModel = library_model.into();
    active.title_preprocessing_rules = Set(Some(rules_json));
    active.update(&db).await.unwrap();

    // Create series
    let _series1 = SeriesRepository::create(&db, library.id, "One Piece (Digital)", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Naruto (Digital)", None)
        .await
        .unwrap();

    // Lock series2's title
    use codex::db::entities::series_metadata;
    let metadata = series_metadata::Entity::find_by_id(series2.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut active_meta: series_metadata::ActiveModel = metadata.into();
    active_meta.title_lock = Set(true);
    active_meta.update(&db).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Reprocess (enqueues a fan-out task)
    let request_body = ReprocessTitleRequest { dry_run: false };
    let request = post_json_request_with_auth(
        &format!("/api/v1/libraries/{}/series/titles/reprocess", library.id),
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<EnqueueReprocessTitleResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(result.success);
    assert_eq!(result.tasks_enqueued, 1);

    // Execute the fan-out task (which enqueues individual tasks)
    let task = TaskRepository::get_by_id(&db, result.task_ids[0])
        .await
        .unwrap()
        .unwrap();
    let handler = ReprocessSeriesTitlesHandler::new();
    let task_result = handler.handle(&task, &db, None).await.unwrap();
    assert!(task_result.message.unwrap().contains("Enqueued 2")); // 2 individual tasks
}

#[tokio::test]
async fn test_reprocess_library_titles_empty_library() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library with no series
    let library =
        LibraryRepository::create(&db, "Empty Library", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request_body = ReprocessTitleRequest { dry_run: false };
    let request = post_json_request_with_auth(
        &format!("/api/v1/libraries/{}/series/titles/reprocess", library.id),
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<EnqueueReprocessTitleResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(result.success);
    assert_eq!(result.tasks_enqueued, 1); // Still enqueues a task (which will find 0 series)

    // Execute the fan-out task
    let task = TaskRepository::get_by_id(&db, result.task_ids[0])
        .await
        .unwrap()
        .unwrap();
    let handler = ReprocessSeriesTitlesHandler::new();
    let task_result = handler.handle(&task, &db, None).await.unwrap();
    assert!(task_result
        .message
        .unwrap()
        .contains("No series to process"));
}

#[tokio::test]
async fn test_reprocess_library_titles_no_rules() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library WITHOUT preprocessing rules
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with "(Digital)" suffix
    let _series = SeriesRepository::create(&db, library.id, "One Piece (Digital)", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Reprocess (enqueues a fan-out task)
    let request_body = ReprocessTitleRequest { dry_run: false };
    let request = post_json_request_with_auth(
        &format!("/api/v1/libraries/{}/series/titles/reprocess", library.id),
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<EnqueueReprocessTitleResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(result.success);
    assert_eq!(result.tasks_enqueued, 1);

    // Execute the fan-out task (which enqueues individual tasks)
    let task = TaskRepository::get_by_id(&db, result.task_ids[0])
        .await
        .unwrap()
        .unwrap();
    let handler = ReprocessSeriesTitlesHandler::new();
    let task_result = handler.handle(&task, &db, None).await.unwrap();
    // No rules, so title won't change - but task should still be enqueued
    assert!(task_result.message.unwrap().contains("Enqueued 1"));
}

#[tokio::test]
async fn test_reprocess_library_titles_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request_body = ReprocessTitleRequest { dry_run: false };
    let non_existent_id = uuid::Uuid::new_v4();
    let request = post_json_request_with_auth(
        &format!(
            "/api/v1/libraries/{}/series/titles/reprocess",
            non_existent_id
        ),
        &request_body,
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}
