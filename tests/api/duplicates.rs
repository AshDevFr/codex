// Allow unused temp_dir - needed to keep TempDir alive but not always referenced
#![allow(unused_variables)]

#[path = "../common/mod.rs"]
mod common;

use chrono::Utc;
use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::duplicates::{
    ListDuplicatesResponse, ListSeriesDuplicatesResponse, TriggerDuplicateScanResponse,
};
use codex::db::ScanningStrategy;
use codex::db::entities::{books, series, series_metadata};
use codex::db::repositories::{
    BookDuplicatesRepository, BookRepository, LibraryRepository, SeriesDuplicatesRepository,
    SeriesExternalIdRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
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
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

// Helper to create duplicate books with the same file hash
async fn create_duplicate_books(
    db: &DatabaseConnection,
    series_id: Uuid,
    library_id: Uuid,
    shared_hash: &str,
) -> (Uuid, Uuid) {
    let now = Utc::now();

    // Create first book
    let book_id1 = Uuid::new_v4();
    let book1 = books::ActiveModel {
        id: Set(book_id1),
        series_id: Set(series_id),
        library_id: Set(library_id),
        path: Set(format!("/tmp/test-{}.cbz", book_id1)),
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
        library_id: Set(library_id),
        path: Set(format!("/tmp/test-{}.cbz", book_id2)),
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
        fingerprint: Set(Some(format!("test-series-{}", series_id))),
        path: Set("/test/series".to_string()),
        name: Set("Test Series".to_string()),
        normalized_name: Set("test series".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
    };
    test_series.insert(&db).await.unwrap();

    // Create series_metadata with the title
    let series_meta = series_metadata::ActiveModel {
        series_id: Set(series_id),
        title: Set("Test Series".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    series_meta.insert(&db).await.unwrap();

    // Create duplicate books
    let shared_hash = "duplicate-hash-123";
    let (_book1, _book2) = create_duplicate_books(&db, series_id, library.id, shared_hash).await;

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
    assert!(!scan_response.task_id.to_string().is_empty());
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
        fingerprint: Set(Some(format!("test-series-{}", series_id))),
        path: Set("/test/series".to_string()),
        name: Set("Test Series".to_string()),
        normalized_name: Set("test series".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
    };
    test_series.insert(&db).await.unwrap();

    // Create series_metadata with the title
    let series_meta = series_metadata::ActiveModel {
        series_id: Set(series_id),
        title: Set("Test Series".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    series_meta.insert(&db).await.unwrap();

    // Create duplicate books
    let shared_hash = "duplicate-hash-456";
    create_duplicate_books(&db, series_id, library.id, shared_hash).await;

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
        fingerprint: Set(Some(format!("test-series-{}", series_id))),
        path: Set("/test/series".to_string()),
        name: Set("Test Series".to_string()),
        normalized_name: Set("test series".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
    };
    test_series.insert(&db).await.unwrap();

    // Create series_metadata with the title
    let series_meta = series_metadata::ActiveModel {
        series_id: Set(series_id),
        title: Set("Test Series".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    series_meta.insert(&db).await.unwrap();

    // Create duplicate books
    let shared_hash = "duplicate-hash-789";
    let (book1, _book2) = create_duplicate_books(&db, series_id, library.id, shared_hash).await;

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
        fingerprint: Set(Some(format!("test-series-{}", series_id))),
        path: Set("/test/series".to_string()),
        name: Set("Test Series".to_string()),
        normalized_name: Set("test series".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
    };
    test_series.insert(&db).await.unwrap();

    // Create series_metadata with the title
    let series_meta = series_metadata::ActiveModel {
        series_id: Set(series_id),
        title: Set("Test Series".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    series_meta.insert(&db).await.unwrap();

    // Create duplicate books
    let shared_hash = "duplicate-hash-soft-delete";
    let (book1, _book2) = create_duplicate_books(&db, series_id, library.id, shared_hash).await;

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

// ============================================================================
// Series Duplicate Tests
// ============================================================================

async fn insert_series(db: &DatabaseConnection, library_id: Uuid, name: &str, title: &str) -> Uuid {
    let now = Utc::now();
    let id = Uuid::new_v4();
    let model = series::ActiveModel {
        id: Set(id),
        library_id: Set(library_id),
        fingerprint: Set(Some(format!("fp-{}", id))),
        path: Set(format!("/series/{}", id)),
        name: Set(name.to_string()),
        normalized_name: Set(name.to_lowercase()),
        created_at: Set(now),
        updated_at: Set(now),
    };
    model.insert(db).await.unwrap();

    let meta = series_metadata::ActiveModel {
        series_id: Set(id),
        title: Set(title.to_string()),
        search_title: Set(title.to_lowercase()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    meta.insert(db).await.unwrap();
    id
}

#[tokio::test]
async fn test_list_series_duplicates_empty() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth("/api/v1/duplicates/series", &token);
    let (status, response): (StatusCode, Option<ListSeriesDuplicatesResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body.total_groups, 0);
    assert_eq!(body.total_duplicate_series, 0);
    assert_eq!(body.external_id_groups, 0);
    assert_eq!(body.title_groups, 0);
}

#[tokio::test]
async fn test_list_series_duplicates_external_id_match() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Lib",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let s1 = insert_series(&db, library.id, "Naruto", "Naruto").await;
    let s2 = insert_series(&db, library.id, "Naruto-JP", "ナルト").await;

    SeriesExternalIdRepository::create_for_plugin(&db, s1, "mangabaka", "12345", None, None)
        .await
        .unwrap();
    SeriesExternalIdRepository::create_for_plugin(&db, s2, "mangabaka", "12345", None, None)
        .await
        .unwrap();

    SeriesDuplicatesRepository::rebuild_from_series(
        &db,
        &["plugin:mangabaka".to_string(), "plugin:anilist".to_string()],
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth("/api/v1/duplicates/series", &token);
    let (status, response): (StatusCode, Option<ListSeriesDuplicatesResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body.total_groups, 1);
    assert_eq!(body.external_id_groups, 1);
    assert_eq!(body.title_groups, 0);

    let group = &body.duplicates[0];
    assert_eq!(group.match_type, "external_id");
    assert_eq!(group.match_key, "plugin:mangabaka:12345");
    assert!(group.library_id.is_none());
    assert_eq!(group.duplicate_count, 2);

    // Members must be hydrated by the list endpoint so the UI never needs to
    // fetch /series/{id} per row.
    assert_eq!(group.members.len(), 2);
    let ids: Vec<Uuid> = group.members.iter().map(|m| m.id).collect();
    assert!(ids.contains(&s1));
    assert!(ids.contains(&s2));
    let titles: Vec<&str> = group.members.iter().map(|m| m.title.as_str()).collect();
    assert!(titles.contains(&"Naruto"));
    assert!(titles.contains(&"ナルト"));
    assert!(group.members.iter().all(|m| m.library_name == "Lib"));
    assert!(group.members.iter().all(|m| m.book_count == 0));
}

#[tokio::test]
async fn test_list_series_duplicates_members_include_book_count_and_library() {
    // Verifies the hydrated member shape: title falls back to series.name when
    // metadata is empty, book_count excludes soft-deleted books, and the
    // library_name comes from the joined libraries row.
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "My Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let s1 = insert_series(&db, library.id, "Fairy Tail", "Fairy Tail").await;
    let s2 = insert_series(&db, library.id, "Fairy Tail 2", "Fairy Tail").await;

    // Two non-deleted books on s1, plus one soft-deleted that must not count.
    let now = Utc::now();
    for i in 0..2 {
        let id = Uuid::new_v4();
        books::ActiveModel {
            id: Set(id),
            series_id: Set(s1),
            library_id: Set(library.id),
            path: Set(format!("/tmp/{}-{}.cbz", id, i)),
            file_name: Set(format!("{}-{}.cbz", id, i)),
            file_size: Set(1024),
            file_hash: Set(format!("hash-{}-{}", id, i)),
            partial_hash: Set(format!("partial-{}", id)),
            format: Set("cbz".to_string()),
            page_count: Set(10),
            modified_at: Set(now),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
    }
    let deleted_id = Uuid::new_v4();
    books::ActiveModel {
        id: Set(deleted_id),
        series_id: Set(s1),
        library_id: Set(library.id),
        path: Set(format!("/tmp/{}-deleted.cbz", deleted_id)),
        file_name: Set(format!("{}-deleted.cbz", deleted_id)),
        file_size: Set(1024),
        file_hash: Set(format!("hash-{}-deleted", deleted_id)),
        partial_hash: Set(format!("partial-{}-deleted", deleted_id)),
        format: Set("cbz".to_string()),
        page_count: Set(10),
        deleted: Set(true),
        modified_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();

    // Title pass groups both series under the same library — runs without any
    // trusted external sources.
    SeriesDuplicatesRepository::rebuild_from_series(&db, &[])
        .await
        .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth("/api/v1/duplicates/series", &token);
    let (status, response): (StatusCode, Option<ListSeriesDuplicatesResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body.total_groups, 1);
    let group = &body.duplicates[0];
    assert_eq!(group.members.len(), 2);

    let m1 = group.members.iter().find(|m| m.id == s1).unwrap();
    assert_eq!(m1.title, "Fairy Tail");
    assert_eq!(m1.library_name, "My Library");
    assert_eq!(m1.book_count, 2, "soft-deleted book must not count");
    let m2 = group.members.iter().find(|m| m.id == s2).unwrap();
    assert_eq!(m2.book_count, 0);
}

#[tokio::test]
async fn test_list_series_duplicates_filter_by_match_type() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Lib",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    // One external-id group.
    let a1 = insert_series(&db, library.id, "A", "A").await;
    let a2 = insert_series(&db, library.id, "A-jp", "ぁ").await;
    SeriesExternalIdRepository::create_for_plugin(&db, a1, "mangabaka", "1", None, None)
        .await
        .unwrap();
    SeriesExternalIdRepository::create_for_plugin(&db, a2, "mangabaka", "1", None, None)
        .await
        .unwrap();

    // One title group.
    let b1 = insert_series(&db, library.id, "B", "Bleach").await;
    let b2 = insert_series(&db, library.id, "B2", "Bleach").await;
    let _ = (b1, b2);

    SeriesDuplicatesRepository::rebuild_from_series(
        &db,
        &["plugin:mangabaka".to_string(), "plugin:anilist".to_string()],
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // External-id filter
    let app = create_test_router_with_app_state(state.clone());
    let request = get_request_with_auth("/api/v1/duplicates/series?matchType=external_id", &token);
    let (status, response): (StatusCode, Option<ListSeriesDuplicatesResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body.total_groups, 1);
    assert_eq!(body.external_id_groups, 1);
    assert_eq!(body.title_groups, 0);

    // Title filter
    let app = create_test_router_with_app_state(state.clone());
    let request = get_request_with_auth("/api/v1/duplicates/series?matchType=title", &token);
    let (status, response): (StatusCode, Option<ListSeriesDuplicatesResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let body = response.unwrap();
    assert_eq!(body.total_groups, 1);
    assert_eq!(body.title_groups, 1);
    assert_eq!(body.external_id_groups, 0);
}

#[tokio::test]
async fn test_list_series_duplicates_rejects_invalid_match_type() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth("/api/v1/duplicates/series?matchType=bogus", &token);
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_delete_series_duplicate_group() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Lib",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let s1 = insert_series(&db, library.id, "A", "Title").await;
    let s2 = insert_series(&db, library.id, "B", "Title").await;
    let _ = (s1, s2);

    SeriesDuplicatesRepository::rebuild_from_series(
        &db,
        &["plugin:mangabaka".to_string(), "plugin:anilist".to_string()],
    )
    .await
    .unwrap();

    let groups = SeriesDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(groups.len(), 1);
    let group_id = groups[0].id;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/duplicates/series/{}", group_id);
    let request = delete_request_with_auth(&uri, &token);
    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert!(
        SeriesDuplicatesRepository::find_all(&db)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn test_delete_series_duplicate_group_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = Uuid::new_v4();
    let uri = format!("/api/v1/duplicates/series/{}", fake_id);
    let request = delete_request_with_auth(&uri, &token);
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
