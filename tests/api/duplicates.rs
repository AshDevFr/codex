#[path = "../common/mod.rs"]
mod common;

use chrono::Utc;
use codex::api::dto::duplicates::{
    DuplicateGroup, ListDuplicatesResponse, TriggerDuplicateScanResponse,
};
use codex::api::error::ErrorResponse;
use codex::db::entities::{books, libraries, series};
use codex::db::repositories::{
    BookDuplicatesRepository, BookRepository, LibraryRepository, UserRepository,
};
use codex::db::ScanningStrategy;
use codex::utils::password;
use common::*;
use hyper::{Request, StatusCode};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use tower::ServiceExt;
use uuid::Uuid;

// Helper to create an admin user and get a token
async fn create_admin_and_token(
    db: &DatabaseConnection,
    state: &codex::api::extractors::AppState,
) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();

    state
        .jwt_service
        .generate_token(created.id, created.username, created.is_admin)
        .unwrap()
}

// Helper to create duplicate books with the same file hash
async fn create_duplicate_books(
    db: &DatabaseConnection,
    series_id: Uuid,
    shared_hash: &str,
) -> (Uuid, Uuid) {
    let now = Utc::now();

    // Create first book
    let book_id1 = Uuid::new_v4();
    let book1 = books::ActiveModel {
        id: Set(book_id1),
        series_id: Set(series_id),
        file_path: Set(format!("/tmp/test-{}.cbz", book_id1)),
        file_name: Set(format!("test-{}.cbz", book_id1)),
        file_size: Set(1024),
        file_hash: Set(shared_hash.to_string()),
        partial_hash: Set(format!("partial-{}", book_id1)),
        format: Set("cbz".to_string()),
        page_count: Set(10),
        modified_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    book1.insert(db).await.expect("Failed to create book 1");

    // Create second book with same hash
    let book_id2 = Uuid::new_v4();
    let book2 = books::ActiveModel {
        id: Set(book_id2),
        series_id: Set(series_id),
        file_path: Set(format!("/tmp/test-{}.cbz", book_id2)),
        file_name: Set(format!("test-{}.cbz", book_id2)),
        file_size: Set(1024),
        file_hash: Set(shared_hash.to_string()),
        partial_hash: Set(format!("partial-{}", book_id2)),
        format: Set("cbz".to_string()),
        page_count: Set(10),
        modified_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    book2.insert(db).await.expect("Failed to create book 2");

    (book_id1, book_id2)
}

// ============================================================================
// List Duplicates Tests
// ============================================================================

#[tokio::test]
async fn test_list_duplicates_empty() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth("/api/v1/duplicates", &token);

    let (status, response): (StatusCode, Option<ListDuplicatesResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let duplicates = response.unwrap();
    assert_eq!(duplicates.duplicates.len(), 0);
    assert_eq!(duplicates.total_groups, 0);
    assert_eq!(duplicates.total_duplicate_books, 0);
}

#[tokio::test]
async fn test_list_duplicates_with_data() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library and series
    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series_id = Uuid::new_v4();
    let now = Utc::now();
    let test_series = series::ActiveModel {
        id: Set(series_id),
        library_id: Set(library.id),
        name: Set("Test Series".to_string()),
        normalized_name: Set("test series".to_string()),
        book_count: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    test_series.insert(&db).await.unwrap();

    // Create duplicate books
    let shared_hash = "duplicate-hash-123";
    let (_book1, _book2) = create_duplicate_books(&db, series_id, shared_hash).await;

    // Rebuild duplicates
    BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth("/api/v1/duplicates", &token);

    let (status, response): (StatusCode, Option<ListDuplicatesResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let duplicates = response.unwrap();
    assert_eq!(duplicates.duplicates.len(), 1);
    assert_eq!(duplicates.total_groups, 1);
    assert_eq!(duplicates.total_duplicate_books, 2);

    let group = &duplicates.duplicates[0];
    assert_eq!(group.file_hash, shared_hash);
    assert_eq!(group.duplicate_count, 2);
    assert_eq!(group.book_ids.len(), 2);
}

// ============================================================================
// Trigger Duplicate Scan Tests
// ============================================================================

#[tokio::test]
async fn test_trigger_duplicate_scan() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = post_request_with_auth("/api/v1/duplicates/scan", &token);

    let (status, response): (StatusCode, Option<TriggerDuplicateScanResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let scan_response = response.unwrap();
    assert_eq!(
        scan_response.message,
        "Duplicate detection scan has been queued"
    );
    // Verify task_id is a valid UUID
    assert!(scan_response.task_id.to_string().len() > 0);
}

#[tokio::test]
async fn test_trigger_duplicate_scan_unauthorized() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state);

    // POST request without auth token
    let request = hyper::Request::builder()
        .method(hyper::Method::POST)
        .uri("/api/v1/duplicates/scan")
        .body(String::new())
        .unwrap();

    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Delete Duplicate Group Tests
// ============================================================================

#[tokio::test]
async fn test_delete_duplicate_group() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library and series
    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series_id = Uuid::new_v4();
    let now = Utc::now();
    let test_series = series::ActiveModel {
        id: Set(series_id),
        library_id: Set(library.id),
        name: Set("Test Series".to_string()),
        normalized_name: Set("test series".to_string()),
        book_count: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    test_series.insert(&db).await.unwrap();

    // Create duplicate books
    let shared_hash = "duplicate-hash-456";
    create_duplicate_books(&db, series_id, shared_hash).await;

    // Rebuild duplicates
    BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();

    // Get the duplicate group ID
    let duplicates = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates.len(), 1);
    let group_id = duplicates[0].id;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/duplicates/{}", group_id);
    let request = delete_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify the group was deleted
    let duplicates_after = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates_after.len(), 0);
}

#[tokio::test]
async fn test_delete_duplicate_group_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = Uuid::new_v4();
    let uri = format!("/api/v1/duplicates/{}", fake_id);
    let request = delete_request_with_auth(&uri, &token);

    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Integration Tests
// ============================================================================

#[tokio::test]
async fn test_duplicates_after_book_deletion() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library and series
    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series_id = Uuid::new_v4();
    let now = Utc::now();
    let test_series = series::ActiveModel {
        id: Set(series_id),
        library_id: Set(library.id),
        name: Set("Test Series".to_string()),
        normalized_name: Set("test series".to_string()),
        book_count: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    test_series.insert(&db).await.unwrap();

    // Create duplicate books
    let shared_hash = "duplicate-hash-789";
    let (book1, _book2) = create_duplicate_books(&db, series_id, shared_hash).await;

    // Rebuild duplicates
    BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();

    // Verify duplicate group exists
    let duplicates = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates.len(), 1);
    assert_eq!(duplicates[0].duplicate_count, 2);

    // Delete one book (hard delete)
    BookRepository::delete(&db, book1).await.unwrap();

    // Verify duplicate group was removed (only 1 book left, not a duplicate)
    let duplicates_after = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates_after.len(), 0);
}

#[tokio::test]
async fn test_duplicates_exclude_soft_deleted_books() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library and series
    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series_id = Uuid::new_v4();
    let now = Utc::now();
    let test_series = series::ActiveModel {
        id: Set(series_id),
        library_id: Set(library.id),
        name: Set("Test Series".to_string()),
        normalized_name: Set("test series".to_string()),
        book_count: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    test_series.insert(&db).await.unwrap();

    // Create duplicate books
    let shared_hash = "duplicate-hash-soft-delete";
    let (book1, _book2) = create_duplicate_books(&db, series_id, shared_hash).await;

    // Rebuild duplicates
    BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();

    // Verify duplicate group exists
    let duplicates = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates.len(), 1);
    assert_eq!(duplicates[0].duplicate_count, 2);

    // Soft delete one book
    BookRepository::mark_deleted(&db, book1, true, None)
        .await
        .unwrap();

    // Verify duplicate group was removed (soft deleted book shouldn't count)
    let duplicates_after = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates_after.len(), 0);
}
