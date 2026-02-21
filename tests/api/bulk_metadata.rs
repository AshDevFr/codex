//! Integration tests for bulk metadata editing endpoints
//!
//! Tests bulk metadata PATCH, bulk tag/genre add/remove, bulk lock toggling,
//! and JSON merge patch for custom_metadata.

#[path = "../common/mod.rs"]
mod common;

use codex::api::routes::v1::dto::bulk_metadata::BulkMetadataUpdateResponse;
use codex::api::routes::v1::dto::series::SeriesMetadataResponse;
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    BookMetadataRepository, BookRepository, GenreRepository, LibraryRepository, SeriesRepository,
    TagRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use serde_json::json;

// Helper to create admin and token
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

// Helper to create a test book model
fn create_test_book_model(
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
    path: &str,
    name: &str,
) -> codex::db::entities::books::Model {
    use chrono::Utc;
    codex::db::entities::books::Model {
        id: uuid::Uuid::new_v4(),
        series_id,
        library_id,
        file_path: path.to_string(),
        file_name: name.to_string(),
        file_size: 1024,
        file_hash: format!("hash_{}", uuid::Uuid::new_v4()),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 20,
        deleted: false,
        analyzed: false,
        analysis_error: None,
        analysis_errors: None,
        modified_at: Utc::now(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        thumbnail_path: None,
        thumbnail_generated_at: None,
    }
}

// ============================================================================
// Bulk Patch Series Metadata Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_patch_series_metadata() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // Create library and two series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series1 = SeriesRepository::create(&db, library.id, "Series One", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Series Two", None)
        .await
        .unwrap();

    // Bulk patch publisher and language
    let body = json!({
        "seriesIds": [series1.id, series2.id],
        "publisher": "DC Comics",
        "language": "en"
    });
    let request =
        patch_request_with_auth_json("/api/v1/series/bulk/metadata", &token, &body.to_string());
    let (status, response): (StatusCode, Option<BulkMetadataUpdateResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let resp = response.unwrap();
    assert_eq!(resp.updated_count, 2);

    // Verify series1 metadata
    let meta1 =
        codex::db::repositories::SeriesMetadataRepository::get_by_series_id(&db, series1.id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(meta1.publisher.as_deref(), Some("DC Comics"));
    assert_eq!(meta1.language.as_deref(), Some("en"));

    // Verify series2 metadata
    let meta2 =
        codex::db::repositories::SeriesMetadataRepository::get_by_series_id(&db, series2.id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(meta2.publisher.as_deref(), Some("DC Comics"));
    assert_eq!(meta2.language.as_deref(), Some("en"));
}

#[tokio::test]
async fn test_bulk_patch_series_metadata_empty_ids() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let body = json!({
        "seriesIds": [],
        "publisher": "DC Comics"
    });
    let request =
        patch_request_with_auth_json("/api/v1/series/bulk/metadata", &token, &body.to_string());
    let (status, response): (StatusCode, Option<BulkMetadataUpdateResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.unwrap().updated_count, 0);
}

#[tokio::test]
async fn test_bulk_patch_series_metadata_nonexistent_ids() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let body = json!({
        "seriesIds": [uuid::Uuid::new_v4()],
        "publisher": "DC Comics"
    });
    let request =
        patch_request_with_auth_json("/api/v1/series/bulk/metadata", &token, &body.to_string());
    let (status, response): (StatusCode, Option<BulkMetadataUpdateResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.unwrap().updated_count, 0);
}

// ============================================================================
// Bulk Modify Series Tags Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_modify_series_tags_add_and_remove() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series1 = SeriesRepository::create(&db, library.id, "Series One", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Series Two", None)
        .await
        .unwrap();

    // Add an existing tag to series1 that we'll remove
    TagRepository::add_tag_to_series(&db, series1.id, "OldTag")
        .await
        .unwrap();

    // Bulk add "NewTag" and remove "OldTag"
    let body = json!({
        "seriesIds": [series1.id, series2.id],
        "add": ["NewTag", "AnotherTag"],
        "remove": ["OldTag"]
    });
    let request =
        post_request_with_auth_json("/api/v1/series/bulk/tags", &token, &body.to_string());
    let (status, response): (StatusCode, Option<BulkMetadataUpdateResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let resp = response.unwrap();
    assert_eq!(resp.updated_count, 2);

    // Verify series1 has NewTag, AnotherTag and does NOT have OldTag
    let tags1 = TagRepository::get_tags_for_series(&db, series1.id)
        .await
        .unwrap();
    let tag_names1: Vec<&str> = tags1.iter().map(|t| t.name.as_str()).collect();
    assert!(tag_names1.contains(&"NewTag"));
    assert!(tag_names1.contains(&"AnotherTag"));
    assert!(!tag_names1.contains(&"OldTag"));

    // Verify series2 has NewTag, AnotherTag
    let tags2 = TagRepository::get_tags_for_series(&db, series2.id)
        .await
        .unwrap();
    let tag_names2: Vec<&str> = tags2.iter().map(|t| t.name.as_str()).collect();
    assert!(tag_names2.contains(&"NewTag"));
    assert!(tag_names2.contains(&"AnotherTag"));
}

// ============================================================================
// Bulk Modify Series Genres Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_modify_series_genres_add() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series1 = SeriesRepository::create(&db, library.id, "Series One", None)
        .await
        .unwrap();

    let body = json!({
        "seriesIds": [series1.id],
        "add": ["Action", "Comedy"],
        "remove": []
    });
    let request =
        post_request_with_auth_json("/api/v1/series/bulk/genres", &token, &body.to_string());
    let (status, response): (StatusCode, Option<BulkMetadataUpdateResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.unwrap().updated_count, 1);

    let genres = GenreRepository::get_genres_for_series(&db, series1.id)
        .await
        .unwrap();
    let genre_names: Vec<&str> = genres.iter().map(|g| g.name.as_str()).collect();
    assert!(genre_names.contains(&"Action"));
    assert!(genre_names.contains(&"Comedy"));
}

// ============================================================================
// Bulk Update Series Locks Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_update_series_locks() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series1 = SeriesRepository::create(&db, library.id, "Series One", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Series Two", None)
        .await
        .unwrap();

    // Bulk lock title and publisher
    let body = json!({
        "seriesIds": [series1.id, series2.id],
        "title": true,
        "publisher": true
    });
    let request = put_request_with_auth(
        "/api/v1/series/bulk/metadata/locks",
        &body.to_string(),
        &token,
    );
    let (status, response): (StatusCode, Option<BulkMetadataUpdateResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.unwrap().updated_count, 2);

    // Verify locks
    let meta1 =
        codex::db::repositories::SeriesMetadataRepository::get_by_series_id(&db, series1.id)
            .await
            .unwrap()
            .unwrap();
    assert!(meta1.title_lock);
    assert!(meta1.publisher_lock);
    assert!(!meta1.summary_lock); // Not changed

    let meta2 =
        codex::db::repositories::SeriesMetadataRepository::get_by_series_id(&db, series2.id)
            .await
            .unwrap()
            .unwrap();
    assert!(meta2.title_lock);
    assert!(meta2.publisher_lock);
}

// ============================================================================
// Bulk Book Tags Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_modify_book_tags() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book1_model = create_test_book_model(series.id, library.id, "/test/book1.cbz", "book1.cbz");
    let book2_model = create_test_book_model(series.id, library.id, "/test/book2.cbz", "book2.cbz");
    let book1 = BookRepository::create(&db, &book1_model, None)
        .await
        .unwrap();
    let book2 = BookRepository::create(&db, &book2_model, None)
        .await
        .unwrap();

    // Create metadata for books (needed for metadata operations)
    BookMetadataRepository::create_with_title_and_number(
        &db,
        book1.id,
        Some("Book 1".to_string()),
        None,
    )
    .await
    .unwrap();
    BookMetadataRepository::create_with_title_and_number(
        &db,
        book2.id,
        Some("Book 2".to_string()),
        None,
    )
    .await
    .unwrap();

    let body = json!({
        "bookIds": [book1.id, book2.id],
        "add": ["Favorite"],
        "remove": []
    });
    let request = post_request_with_auth_json("/api/v1/books/bulk/tags", &token, &body.to_string());
    let (status, response): (StatusCode, Option<BulkMetadataUpdateResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.unwrap().updated_count, 2);

    let tags1 = TagRepository::get_tags_for_book(&db, book1.id)
        .await
        .unwrap();
    assert!(tags1.iter().any(|t| t.name == "Favorite"));

    let tags2 = TagRepository::get_tags_for_book(&db, book2.id)
        .await
        .unwrap();
    assert!(tags2.iter().any(|t| t.name == "Favorite"));
}

// ============================================================================
// Bulk Book Metadata PATCH Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_patch_book_metadata() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book1_model = create_test_book_model(series.id, library.id, "/test/b1.cbz", "b1.cbz");
    let book2_model = create_test_book_model(series.id, library.id, "/test/b2.cbz", "b2.cbz");
    let book1 = BookRepository::create(&db, &book1_model, None)
        .await
        .unwrap();
    let book2 = BookRepository::create(&db, &book2_model, None)
        .await
        .unwrap();

    BookMetadataRepository::create_with_title_and_number(
        &db,
        book1.id,
        Some("Book 1".to_string()),
        None,
    )
    .await
    .unwrap();
    BookMetadataRepository::create_with_title_and_number(
        &db,
        book2.id,
        Some("Book 2".to_string()),
        None,
    )
    .await
    .unwrap();

    // Bulk update publisher and language
    let body = json!({
        "bookIds": [book1.id, book2.id],
        "publisher": "Marvel",
        "languageIso": "en"
    });
    let request =
        patch_request_with_auth_json("/api/v1/books/bulk/metadata", &token, &body.to_string());
    let (status, response): (StatusCode, Option<BulkMetadataUpdateResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.unwrap().updated_count, 2);

    // Verify book1 metadata
    let meta1 = BookMetadataRepository::get_by_book_id(&db, book1.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(meta1.publisher.as_deref(), Some("Marvel"));
    assert_eq!(meta1.language_iso.as_deref(), Some("en"));
    assert!(meta1.publisher_lock); // Auto-locked
    assert!(meta1.language_iso_lock);
}

// ============================================================================
// Bulk Book Locks Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_update_book_locks() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book1_model = create_test_book_model(series.id, library.id, "/test/b1.cbz", "b1.cbz");
    let book1 = BookRepository::create(&db, &book1_model, None)
        .await
        .unwrap();
    BookMetadataRepository::create_with_title_and_number(
        &db,
        book1.id,
        Some("Book 1".to_string()),
        None,
    )
    .await
    .unwrap();

    // Bulk lock title and summary
    let body = json!({
        "bookIds": [book1.id],
        "titleLock": true,
        "summaryLock": true
    });
    let request = put_request_with_auth(
        "/api/v1/books/bulk/metadata/locks",
        &body.to_string(),
        &token,
    );
    let (status, response): (StatusCode, Option<BulkMetadataUpdateResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.unwrap().updated_count, 1);

    let meta = BookMetadataRepository::get_by_book_id(&db, book1.id)
        .await
        .unwrap()
        .unwrap();
    assert!(meta.title_lock);
    assert!(meta.summary_lock);
    assert!(!meta.publisher_lock); // Not changed
}

// ============================================================================
// JSON Merge Patch for custom_metadata (via single-item PATCH)
// ============================================================================

#[tokio::test]
async fn test_single_series_patch_custom_metadata_merge() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Set initial custom_metadata
    let app = create_test_router(state.clone()).await;
    let body1 = json!({
        "customMetadata": {"rating": 5, "notes": "Great"}
    });
    let request1 = patch_request_with_auth_json(
        &format!("/api/v1/series/{}/metadata", series.id),
        &token,
        &body1.to_string(),
    );
    let (status1, _): (StatusCode, Option<SeriesMetadataResponse>) =
        make_json_request(app, request1).await;
    assert_eq!(status1, StatusCode::OK);

    // Merge patch: add "status", remove "notes", keep "rating"
    let app2 = create_test_router(state.clone()).await;
    let body2 = json!({
        "customMetadata": {"status": "completed", "notes": null}
    });
    let request2 = patch_request_with_auth_json(
        &format!("/api/v1/series/{}/metadata", series.id),
        &token,
        &body2.to_string(),
    );
    let (status2, response2): (StatusCode, Option<SeriesMetadataResponse>) =
        make_json_request(app2, request2).await;
    assert_eq!(status2, StatusCode::OK);

    let meta = response2.unwrap();
    let cm = meta.custom_metadata.unwrap();
    assert_eq!(cm["rating"], 5); // Preserved
    assert_eq!(cm["status"], "completed"); // Added
    assert!(cm.get("notes").is_none()); // Removed
}

// ============================================================================
// JSON Merge Patch for custom_metadata (via bulk PATCH)
// ============================================================================

#[tokio::test]
async fn test_bulk_series_patch_custom_metadata_merge() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Set initial custom_metadata via single-item patch
    let app = create_test_router(state.clone()).await;
    let body1 = json!({
        "customMetadata": {"rating": 5, "notes": "Great"}
    });
    let request1 = patch_request_with_auth_json(
        &format!("/api/v1/series/{}/metadata", series.id),
        &token,
        &body1.to_string(),
    );
    let (status1, _): (StatusCode, Option<SeriesMetadataResponse>) =
        make_json_request(app, request1).await;
    assert_eq!(status1, StatusCode::OK);

    // Bulk merge patch: add "tag" field
    let app2 = create_test_router(state.clone()).await;
    let body2 = json!({
        "seriesIds": [series.id],
        "customMetadata": {"tag": "favorite"}
    });
    let request2 =
        patch_request_with_auth_json("/api/v1/series/bulk/metadata", &token, &body2.to_string());
    let (status2, response2): (StatusCode, Option<BulkMetadataUpdateResponse>) =
        make_json_request(app2, request2).await;
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(response2.unwrap().updated_count, 1);

    // Verify merged state
    let meta = codex::db::repositories::SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    let cm: serde_json::Value =
        serde_json::from_str(meta.custom_metadata.as_deref().unwrap()).unwrap();
    assert_eq!(cm["rating"], 5); // Preserved
    assert_eq!(cm["notes"], "Great"); // Preserved
    assert_eq!(cm["tag"], "favorite"); // Added
}
