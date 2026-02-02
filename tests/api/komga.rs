//! Integration tests for Komga-compatible API endpoints
//!
//! Tests the Komga API library, series, and book endpoints for compatibility with apps like Komic.

// Allow unused temp_dir - needed to keep TempDir alive but not always referenced
#![allow(unused_variables)]

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::komga::dto::book::KomgaBookDto;
use codex::api::routes::komga::dto::library::KomgaLibraryDto;
use codex::api::routes::komga::dto::pagination::KomgaPage;
use codex::api::routes::komga::dto::series::KomgaSeriesDto;
use codex::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, UserRepository,
};
use codex::db::ScanningStrategy;
use codex::utils::password;
use common::*;
use hyper::StatusCode;

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

// Helper to create an admin user for Basic Auth testing
async fn create_admin_user(db: &sea_orm::DatabaseConnection) {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    UserRepository::create(db, &user).await.unwrap();
}

// ============================================================================
// List Libraries Tests
// ============================================================================

#[tokio::test]
async fn test_komga_list_libraries_with_bearer_auth() {
    let (db, temp_dir) = setup_test_db().await;

    // Create some test libraries
    LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let request = get_request_with_auth("/komga/api/v1/libraries", &token);
    let (status, response): (StatusCode, Option<Vec<KomgaLibraryDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let libraries = response.unwrap();
    assert_eq!(libraries.len(), 2);

    // Verify Komga-specific fields
    assert_eq!(libraries[0].name, "Comics");
    assert_eq!(libraries[0].root, "/comics");
    assert!(!libraries[0].unavailable);

    // Check camelCase field names in serialization (implicit in deserialize)
    assert!(libraries[0].scan_cbx);
    assert!(libraries[0].scan_epub);
    assert!(libraries[0].scan_pdf);
}

#[tokio::test]
async fn test_komga_list_libraries_with_basic_auth() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a library and user
    LibraryRepository::create(&db, "My Library", "/path", ScanningStrategy::Default)
        .await
        .unwrap();
    create_admin_user(&db).await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router_with_komga(state);

    // Use Basic Auth (what Komic uses)
    let request = get_request_with_basic_auth("/komga/api/v1/libraries", "admin", "admin123");
    let (status, response): (StatusCode, Option<Vec<KomgaLibraryDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let libraries = response.unwrap();
    assert_eq!(libraries.len(), 1);
    assert_eq!(libraries[0].name, "My Library");
}

#[tokio::test]
async fn test_komga_list_libraries_without_auth() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router_with_komga(state);

    let request = get_request("/komga/api/v1/libraries");
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Get Library by ID Tests
// ============================================================================

#[tokio::test]
async fn test_komga_get_library_by_id() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a library
    let library =
        LibraryRepository::create(&db, "Test Library", "/test/path", ScanningStrategy::Default)
            .await
            .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let uri = format!("/komga/api/v1/libraries/{}", library.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, response): (StatusCode, Option<KomgaLibraryDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let dto = response.unwrap();
    assert_eq!(dto.id, library.id.to_string());
    assert_eq!(dto.name, "Test Library");
    assert_eq!(dto.root, "/test/path");
}

#[tokio::test]
async fn test_komga_get_library_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Use a random UUID that doesn't exist
    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/komga/api/v1/libraries/{}", fake_id);
    let request = get_request_with_auth(&uri, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Library Thumbnail Tests
// ============================================================================

#[tokio::test]
async fn test_komga_library_thumbnail_no_series() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a library with no series
    let library =
        LibraryRepository::create(&db, "Empty Library", "/empty", ScanningStrategy::Default)
            .await
            .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let uri = format!("/komga/api/v1/libraries/{}/thumbnail", library.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, _) = make_raw_request(app, request).await;

    // Should return 404 since there are no series in the library
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Feature Flag Tests
// ============================================================================

#[tokio::test]
async fn test_komga_api_disabled_returns_404() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library
    LibraryRepository::create(&db, "Library", "/path", ScanningStrategy::Default)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Use router WITHOUT Komga API enabled (default router)
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth("/komga/api/v1/libraries", &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    // Should return 404 since Komga API is not enabled
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Response Format Tests
// ============================================================================

#[tokio::test]
async fn test_komga_library_dto_format() {
    let (db, temp_dir) = setup_test_db().await;

    LibraryRepository::create(&db, "Formatted", "/format/path", ScanningStrategy::Default)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let request = get_request_with_auth("/komga/api/v1/libraries", &token);
    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Parse as raw JSON to verify field names are camelCase
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let library = &json[0];

    // Verify camelCase fields exist
    assert!(library.get("id").is_some());
    assert!(library.get("name").is_some());
    assert!(library.get("root").is_some());
    assert!(library.get("scanCbx").is_some());
    assert!(library.get("scanEpub").is_some());
    assert!(library.get("scanPdf").is_some());
    assert!(library.get("hashFiles").is_some());
    assert!(library.get("scanInterval").is_some());
    assert!(library.get("seriesCover").is_some());
    assert!(library.get("unavailable").is_some());

    // Verify NOT snake_case
    assert!(library.get("scan_cbx").is_none());
    assert!(library.get("scan_epub").is_none());
}

// ============================================================================
// Series Tests
// ============================================================================

#[tokio::test]
async fn test_komga_list_series() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a library and series
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Superman", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let request = get_request_with_auth("/komga/api/v1/series", &token);
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 2);
    assert_eq!(page.content.len(), 2);
    assert!(!page.empty);
}

#[tokio::test]
async fn test_komga_list_series_pagination() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a library with 5 series
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    for i in 1..=5 {
        SeriesRepository::create(&db, library.id, &format!("Series {}", i), None)
            .await
            .unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Request page 0 with size 2
    let request = get_request_with_auth("/komga/api/v1/series?page=0&size=2", &token);
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 5);
    assert_eq!(page.content.len(), 2);
    assert_eq!(page.number, 0);
    assert!(page.first);
    assert!(!page.last);
    assert_eq!(page.total_pages, 3); // ceil(5/2) = 3
}

#[tokio::test]
async fn test_komga_list_series_filter_by_library() {
    let (db, temp_dir) = setup_test_db().await;

    // Create two libraries with series
    let library1 = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let library2 = LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library1.id, "Batman", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library1.id, "Superman", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library2.id, "One Piece", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Filter by library1
    let uri = format!("/komga/api/v1/series?library_id={}", library1.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 2);

    // All series should be from library1
    for series in page.content {
        assert_eq!(series.library_id, library1.id.to_string());
    }
}

#[tokio::test]
async fn test_komga_get_series_by_id() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let uri = format!("/komga/api/v1/series/{}", series.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, response): (StatusCode, Option<KomgaSeriesDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let dto = response.unwrap();
    assert_eq!(dto.id, series.id.to_string());
    assert_eq!(dto.name, "Batman");
    assert_eq!(dto.library_id, library.id.to_string());
}

#[tokio::test]
async fn test_komga_get_series_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/komga/api/v1/series/{}", fake_id);
    let request = get_request_with_auth(&uri, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_komga_series_new() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Series A", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Series B", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let request = get_request_with_auth("/komga/api/v1/series/new", &token);
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 2);
    // Series should be ordered by created_at descending (newest first)
    // Series B was created after Series A, so it should be first
    assert_eq!(page.content[0].name, "Series B");
    assert_eq!(page.content[1].name, "Series A");
}

#[tokio::test]
async fn test_komga_series_updated() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Series A", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Series B", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let request = get_request_with_auth("/komga/api/v1/series/updated", &token);
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 2);
}

#[tokio::test]
async fn test_komga_series_thumbnail_no_books() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Empty Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let uri = format!("/komga/api/v1/series/{}/thumbnail", series.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, _) = make_raw_request(app, request).await;

    // Should return 404 since there are no books in the series
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_komga_series_dto_format() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let request = get_request_with_auth("/komga/api/v1/series", &token);
    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Parse as raw JSON to verify field names are camelCase
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify KomgaPage wrapper fields
    assert!(json.get("content").is_some());
    assert!(json.get("pageable").is_some());
    assert!(json.get("totalElements").is_some());
    assert!(json.get("totalPages").is_some());
    assert!(json.get("numberOfElements").is_some());

    // Verify series content camelCase fields
    let series = &json["content"][0];
    assert!(series.get("id").is_some());
    assert!(series.get("libraryId").is_some());
    assert!(series.get("name").is_some());
    assert!(series.get("booksCount").is_some());
    assert!(series.get("booksReadCount").is_some());
    assert!(series.get("booksUnreadCount").is_some());
    assert!(series.get("booksInProgressCount").is_some());
    assert!(series.get("lastModified").is_some());
    assert!(series.get("metadata").is_some());
    assert!(series.get("booksMetadata").is_some());

    // Verify metadata camelCase fields
    let metadata = &series["metadata"];
    assert!(metadata.get("title").is_some());
    assert!(metadata.get("titleSort").is_some());
    assert!(metadata.get("statusLock").is_some());

    // Verify NOT snake_case
    assert!(json.get("total_elements").is_none());
    assert!(series.get("library_id").is_none());
    assert!(series.get("books_count").is_none());
}

// ============================================================================
// Book Tests
// ============================================================================

#[tokio::test]
async fn test_komga_get_book_by_id() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "abc123",
        "cbz",
        20,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let uri = format!("/komga/api/v1/books/{}", created_book.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, response): (StatusCode, Option<KomgaBookDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let dto = response.unwrap();
    assert_eq!(dto.id, created_book.id.to_string());
    assert_eq!(dto.series_id, series.id.to_string());
    assert_eq!(dto.library_id, library.id.to_string());
    assert_eq!(dto.name, "issue1.cbz");
    assert_eq!(dto.media.pages_count, 20);
}

#[tokio::test]
async fn test_komga_get_book_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/komga/api/v1/books/{}", fake_id);
    let request = get_request_with_auth(&uri, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_komga_get_book_thumbnail_no_pages() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and book with 0 pages
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/empty.cbz",
        "empty.cbz",
        "empty123",
        "cbz",
        0, // No pages
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let uri = format!("/komga/api/v1/books/{}/thumbnail", created_book.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, _) = make_raw_request(app, request).await;

    // Should return 404 since book has no pages
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_komga_books_ondeck_empty() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let request = get_request_with_auth("/komga/api/v1/books/ondeck", &token);
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 0);
    assert!(page.content.is_empty());
    assert!(page.empty);
}

#[tokio::test]
async fn test_komga_search_books_empty() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // POST /books/list with empty body
    let request = post_request_with_auth_json("/komga/api/v1/books/list", &token, "{}");
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 0);
    assert!(page.empty);
}

#[tokio::test]
async fn test_komga_search_books_with_filter() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and books
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book1 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        20,
    );
    BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue2.cbz",
        "issue2.cbz",
        "hash2",
        "cbz",
        25,
    );
    BookRepository::create(&db, &book2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // POST /books/list with series_id filter
    let body = format!(r#"{{"seriesId": ["{}"]}}"#, series.id);
    let request = post_request_with_auth_json("/komga/api/v1/books/list", &token, &body);
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 2);
    assert_eq!(page.content.len(), 2);
}

#[tokio::test]
async fn test_komga_get_next_book() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and books
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book1 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        20,
    );
    let created_book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue2.cbz",
        "issue2.cbz",
        "hash2",
        "cbz",
        25,
    );
    let created_book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Get next book after book1
    let uri = format!("/komga/api/v1/books/{}/next", created_book1.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, response): (StatusCode, Option<KomgaBookDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let next_book = response.unwrap();
    assert_eq!(next_book.id, created_book2.id.to_string());
}

#[tokio::test]
async fn test_komga_get_next_book_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and only one book (no next book)
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        20,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Get next book - should be 404 since this is the only book
    let uri = format!("/komga/api/v1/books/{}/next", created_book.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_komga_get_previous_book() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and books
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book1 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        20,
    );
    let created_book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue2.cbz",
        "issue2.cbz",
        "hash2",
        "cbz",
        25,
    );
    let created_book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Get previous book before book2
    let uri = format!("/komga/api/v1/books/{}/previous", created_book2.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, response): (StatusCode, Option<KomgaBookDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let prev_book = response.unwrap();
    assert_eq!(prev_book.id, created_book1.id.to_string());
}

#[tokio::test]
async fn test_komga_get_previous_book_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and only one book (no previous book)
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        20,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Get previous book - should be 404 since this is the only book
    let uri = format!("/komga/api/v1/books/{}/previous", created_book.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_komga_book_dto_format() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "abc123",
        "cbz",
        20,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let uri = format!("/komga/api/v1/books/{}", created_book.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Parse as raw JSON to verify field names are camelCase
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify camelCase fields exist
    assert!(json.get("id").is_some());
    assert!(json.get("seriesId").is_some());
    assert!(json.get("seriesTitle").is_some());
    assert!(json.get("libraryId").is_some());
    assert!(json.get("name").is_some());
    assert!(json.get("sizeBytes").is_some());
    assert!(json.get("fileLastModified").is_some());
    assert!(json.get("media").is_some());
    assert!(json.get("metadata").is_some());

    // Verify media fields
    let media = &json["media"];
    assert!(media.get("status").is_some());
    assert!(media.get("mediaType").is_some());
    assert!(media.get("mediaProfile").is_some());
    assert!(media.get("pagesCount").is_some());

    // Verify NOT snake_case
    assert!(json.get("series_id").is_none());
    assert!(json.get("library_id").is_none());
    assert!(json.get("size_bytes").is_none());
}

// ============================================================================
// Page Tests
// ============================================================================

#[tokio::test]
async fn test_komga_list_pages_for_book() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        10, // 10 pages
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let uri = format!("/komga/api/v1/books/{}/pages", created_book.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, response): (
        StatusCode,
        Option<Vec<codex::api::routes::komga::dto::page::KomgaPageDto>>,
    ) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let pages = response.unwrap();
    // Synthetic pages are generated from page_count since no pages in DB
    assert_eq!(pages.len(), 10);

    // Verify page numbers are 1-indexed
    assert_eq!(pages[0].number, 1);
    assert_eq!(pages[9].number, 10);

    // Verify synthetic filenames
    assert!(pages[0].file_name.contains("page"));
}

#[tokio::test]
async fn test_komga_list_pages_book_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/komga/api/v1/books/{}/pages", fake_id);
    let request = get_request_with_auth(&uri, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_komga_list_pages_without_auth() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db).await;
    let app = create_test_router_with_komga(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/komga/api/v1/books/{}/pages", fake_id);
    let request = get_request(&uri);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_komga_get_page_invalid_number() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        10,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Page 0 should be bad request
    let uri = format!("/komga/api/v1/books/{}/pages/0", created_book.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, _) = make_raw_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_komga_get_page_out_of_bounds() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        10, // 10 pages
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Page 100 should be not found (book only has 10 pages)
    let uri = format!("/komga/api/v1/books/{}/pages/100", created_book.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, _) = make_raw_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_komga_get_page_thumbnail_invalid_number() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        10,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Page -5 cannot be parsed by axum Path extractor, but page 0 should fail
    let uri = format!("/komga/api/v1/books/{}/pages/0/thumbnail", created_book.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, _) = make_raw_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_komga_page_dto_format() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        5,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let uri = format!("/komga/api/v1/books/{}/pages", created_book.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Parse as raw JSON to verify field names are camelCase
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let page = &json[0];

    // Verify camelCase fields exist
    assert!(page.get("fileName").is_some());
    assert!(page.get("mediaType").is_some());
    assert!(page.get("number").is_some());
    assert!(page.get("width").is_some());
    assert!(page.get("height").is_some());
    assert!(page.get("sizeBytes").is_some());
    assert!(page.get("size").is_some());

    // Verify NOT snake_case
    assert!(page.get("file_name").is_none());
    assert!(page.get("media_type").is_none());
    assert!(page.get("size_bytes").is_none());
}

// ============================================================================
// Read Progress Tests
// ============================================================================

#[tokio::test]
async fn test_komga_update_read_progress() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        100, // 100 pages
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Update progress - typical Komic format
    let uri = format!("/komga/api/v1/books/{}/read-progress", created_book.id);
    let body = r#"{"completed":false,"page":42}"#;
    let request = patch_request_with_auth_json(&uri, &token, body);
    let (status, _) = make_raw_request(app, request).await;

    // Komga returns 204 No Content on success
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_komga_update_read_progress_marks_completed() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        50, // 50 pages
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Update progress - mark as completed
    let uri = format!("/komga/api/v1/books/{}/read-progress", created_book.id);
    let body = r#"{"completed":true,"page":50}"#;
    let request = patch_request_with_auth_json(&uri, &token, body);
    let (status, _) = make_raw_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_komga_update_read_progress_without_auth() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        100,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router_with_komga(state);

    // Try to update progress without auth
    let uri = format!("/komga/api/v1/books/{}/read-progress", created_book.id);
    let request = patch_json_request(&uri, &serde_json::json!({"page": 42}));
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_komga_update_read_progress_book_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Try to update progress for non-existent book
    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/komga/api/v1/books/{}/read-progress", fake_id);
    let body = r#"{"completed":false,"page":42}"#;
    let request = patch_request_with_auth_json(&uri, &token, body);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_komga_delete_read_progress() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        100,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Delete progress
    let uri = format!("/komga/api/v1/books/{}/read-progress", created_book.id);
    let request = delete_request_with_auth(&uri, &token);
    let (status, _) = make_raw_request(app, request).await;

    // Komga returns 204 No Content on success
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_komga_delete_read_progress_without_auth() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        100,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router_with_komga(state);

    // Try to delete progress without auth
    let uri = format!("/komga/api/v1/books/{}/read-progress", created_book.id);
    let request = delete_request(&uri);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_komga_delete_read_progress_book_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Try to delete progress for non-existent book
    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/komga/api/v1/books/{}/read-progress", fake_id);
    let request = delete_request_with_auth(&uri, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_komga_read_progress_reflected_in_book_response() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash1",
        "cbz",
        100,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // First, update progress
    {
        let app = create_test_router_with_komga(state.clone());
        let uri = format!("/komga/api/v1/books/{}/read-progress", created_book.id);
        let body = r#"{"completed":false,"page":42}"#;
        let request = patch_request_with_auth_json(&uri, &token, body);
        let (status, _) = make_raw_request(app, request).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    // Then, verify progress is reflected in book response
    {
        let app = create_test_router_with_komga(state);
        let uri = format!("/komga/api/v1/books/{}", created_book.id);
        let request = get_request_with_auth(&uri, &token);
        let (status, response): (StatusCode, Option<KomgaBookDto>) =
            make_json_request(app, request).await;

        assert_eq!(status, StatusCode::OK);
        let book_dto = response.unwrap();

        // Verify progress is included
        assert!(book_dto.read_progress.is_some());
        let progress = book_dto.read_progress.unwrap();
        assert_eq!(progress.page, 42);
        assert!(!progress.completed);
    }
}

// ============================================================================
// User Tests
// ============================================================================

#[tokio::test]
async fn test_komga_get_current_user() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let request = get_request_with_auth("/komga/api/v1/users/me", &token);
    let (status, response): (
        StatusCode,
        Option<codex::api::routes::komga::dto::user::KomgaUserDto>,
    ) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let user = response.unwrap();
    assert_eq!(user.email, "admin@example.com");
    assert!(user.roles.contains(&"ADMIN".to_string()));
    assert!(user.shared_all_libraries);
}

#[tokio::test]
async fn test_komga_get_current_user_basic_auth() {
    let (db, temp_dir) = setup_test_db().await;

    create_admin_user(&db).await;

    let state = create_test_auth_state(db).await;
    let app = create_test_router_with_komga(state);

    // Use Basic Auth (what Komic uses)
    let request = get_request_with_basic_auth("/komga/api/v1/users/me", "admin", "admin123");
    let (status, response): (
        StatusCode,
        Option<codex::api::routes::komga::dto::user::KomgaUserDto>,
    ) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let user = response.unwrap();
    assert_eq!(user.email, "admin@example.com");
    assert!(user.roles.contains(&"ADMIN".to_string()));
}

#[tokio::test]
async fn test_komga_get_current_user_without_auth() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db).await;
    let app = create_test_router_with_komga(state);

    let request = get_request("/komga/api/v1/users/me");
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_komga_user_dto_format() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let request = get_request_with_auth("/komga/api/v1/users/me", &token);
    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Parse as raw JSON to verify field names are camelCase
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify camelCase fields exist
    assert!(json.get("id").is_some());
    assert!(json.get("email").is_some());
    assert!(json.get("roles").is_some());
    assert!(json.get("sharedLibrariesIds").is_some());
    assert!(json.get("sharedAllLibraries").is_some());
    assert!(json.get("labelsAllow").is_some());
    assert!(json.get("labelsExclude").is_some());
    assert!(json.get("contentRestrictions").is_some());

    // Verify NOT snake_case
    assert!(json.get("shared_libraries_ids").is_none());
    assert!(json.get("shared_all_libraries").is_none());
    assert!(json.get("labels_allow").is_none());
}

/// Test that /api/v2/users/me works for Komic's connection test
///
/// Komic app uses /api/v2/users/me as its server connection test endpoint,
/// even though it uses /api/v1/* for all actual data requests.
#[tokio::test]
async fn test_komga_v2_users_me_for_komic_connection_test() {
    let (db, temp_dir) = setup_test_db().await;

    create_admin_user(&db).await;

    let state = create_test_auth_state(db).await;
    let app = create_test_router_with_komga(state);

    // Komic uses Basic Auth with /api/v2/users/me for connection testing
    let request = get_request_with_basic_auth("/komga/api/v2/users/me", "admin", "admin123");
    let (status, response): (
        StatusCode,
        Option<codex::api::routes::komga::dto::user::KomgaUserDto>,
    ) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let user = response.unwrap();
    assert_eq!(user.email, "admin@example.com");
    assert!(user.roles.contains(&"ADMIN".to_string()));
}

// ============================================================================
// POST /series/list Tests
// ============================================================================

/// Test that POST /series/list returns series (Komic uses this for filtering)
#[tokio::test]
async fn test_komga_search_series_post() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a library with a series
    let lib = LibraryRepository::create(&db, "Test", "/test", ScanningStrategy::Default)
        .await
        .unwrap();
    SeriesRepository::create(&db, lib.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // POST /series/list with empty condition (how Komic calls it)
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 1);
    assert_eq!(page.content[0].name, "Test Series");
}

// ============================================================================
// Stub Endpoint Tests (Collections, Read Lists, Genres, Tags, Authors)
// ============================================================================

/// Test that /collections returns empty result (stub)
#[tokio::test]
async fn test_komga_collections_empty() {
    let (db, temp_dir) = setup_test_db().await;
    create_admin_user(&db).await;

    let state = create_test_auth_state(db).await;
    let app = create_test_router_with_komga(state);

    let request =
        get_request_with_basic_auth("/komga/api/v1/collections?page=0", "admin", "admin123");
    let (status, response): (
        StatusCode,
        Option<KomgaPage<codex::api::routes::komga::dto::stubs::KomgaCollectionDto>>,
    ) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 0);
    assert_eq!(page.total_elements, 0);
}

/// Test that /readlists returns empty result (stub)
#[tokio::test]
async fn test_komga_readlists_empty() {
    let (db, temp_dir) = setup_test_db().await;
    create_admin_user(&db).await;

    let state = create_test_auth_state(db).await;
    let app = create_test_router_with_komga(state);

    let request =
        get_request_with_basic_auth("/komga/api/v1/readlists?page=0", "admin", "admin123");
    let (status, response): (
        StatusCode,
        Option<KomgaPage<codex::api::routes::komga::dto::stubs::KomgaReadListDto>>,
    ) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 0);
    assert_eq!(page.total_elements, 0);
}

/// Test that /genres returns empty array (stub)
#[tokio::test]
async fn test_komga_genres_empty() {
    let (db, temp_dir) = setup_test_db().await;
    create_admin_user(&db).await;

    let state = create_test_auth_state(db).await;
    let app = create_test_router_with_komga(state);

    let request = get_request_with_basic_auth("/komga/api/v1/genres", "admin", "admin123");
    let (status, response): (StatusCode, Option<Vec<String>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let genres = response.unwrap();
    assert_eq!(genres.len(), 0);
}

/// Test that /tags returns empty array (stub)
#[tokio::test]
async fn test_komga_tags_empty() {
    let (db, temp_dir) = setup_test_db().await;
    create_admin_user(&db).await;

    let state = create_test_auth_state(db).await;
    let app = create_test_router_with_komga(state);

    let request = get_request_with_basic_auth("/komga/api/v1/tags", "admin", "admin123");
    let (status, response): (StatusCode, Option<Vec<String>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let tags = response.unwrap();
    assert_eq!(tags.len(), 0);
}

/// Test that /api/v2/authors returns empty array (stub for Komic)
#[tokio::test]
async fn test_komga_authors_v2_empty() {
    let (db, temp_dir) = setup_test_db().await;
    create_admin_user(&db).await;

    let state = create_test_auth_state(db).await;
    let app = create_test_router_with_komga(state);

    let request =
        get_request_with_basic_auth("/komga/api/v2/authors?unpaged=true", "admin", "admin123");
    let (status, response): (
        StatusCode,
        Option<Vec<codex::api::routes::komga::dto::series::KomgaAuthorDto>>,
    ) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let authors = response.unwrap();
    assert_eq!(authors.len(), 0);
}

// ============================================================================
// Series Read Progress Tests
// ============================================================================

/// Test POST /series/{id}/read-progress marks all books in series as read
#[tokio::test]
async fn test_komga_mark_series_as_read() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and multiple books
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    // Create 3 books in the series
    for i in 1..=3 {
        let book = create_test_book(
            series.id,
            library.id,
            &format!("/comics/Batman/issue{}.cbz", i),
            &format!("issue{}.cbz", i),
            &format!("hash{}", i),
            "cbz",
            50, // 50 pages each
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Mark series as read
    let uri = format!("/komga/api/v1/series/{}/read-progress", series.id);
    let request = post_request_with_auth(&uri, &token);
    let (status, _) = make_raw_request(app, request).await;

    // Komga returns 204 No Content on success
    assert_eq!(status, StatusCode::NO_CONTENT);
}

/// Test POST /series/{id}/read-progress without auth returns 401
#[tokio::test]
async fn test_komga_mark_series_as_read_without_auth() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router_with_komga(state);

    // Try to mark series as read without auth
    let uri = format!("/komga/api/v1/series/{}/read-progress", series.id);
    let request = post_request(&uri);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// Test POST /series/{id}/read-progress with non-existent series returns 404
#[tokio::test]
async fn test_komga_mark_series_as_read_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Try to mark non-existent series as read
    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/komga/api/v1/series/{}/read-progress", fake_id);
    let request = post_request_with_auth(&uri, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Test DELETE /series/{id}/read-progress marks all books in series as unread
#[tokio::test]
async fn test_komga_mark_series_as_unread() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and multiple books
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    // Create 3 books in the series
    for i in 1..=3 {
        let book = create_test_book(
            series.id,
            library.id,
            &format!("/comics/Batman/issue{}.cbz", i),
            &format!("issue{}.cbz", i),
            &format!("hash{}", i),
            "cbz",
            50,
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Mark series as unread
    let uri = format!("/komga/api/v1/series/{}/read-progress", series.id);
    let request = delete_request_with_auth(&uri, &token);
    let (status, _) = make_raw_request(app, request).await;

    // Komga returns 204 No Content on success
    assert_eq!(status, StatusCode::NO_CONTENT);
}

/// Test DELETE /series/{id}/read-progress without auth returns 401
#[tokio::test]
async fn test_komga_mark_series_as_unread_without_auth() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router_with_komga(state);

    // Try to mark series as unread without auth
    let uri = format!("/komga/api/v1/series/{}/read-progress", series.id);
    let request = delete_request(&uri);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// Test DELETE /series/{id}/read-progress with non-existent series returns 404
#[tokio::test]
async fn test_komga_mark_series_as_unread_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Try to mark non-existent series as unread
    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/komga/api/v1/series/{}/read-progress", fake_id);
    let request = delete_request_with_auth(&uri, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Test that marking series as read is reflected in series response
#[tokio::test]
async fn test_komga_series_read_progress_reflected_in_response() {
    let (db, temp_dir) = setup_test_db().await;

    // Create library, series, and books
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    // Create 3 books
    for i in 1..=3 {
        let book = create_test_book(
            series.id,
            library.id,
            &format!("/comics/Batman/issue{}.cbz", i),
            &format!("issue{}.cbz", i),
            &format!("hash{}", i),
            "cbz",
            50,
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // First, mark series as read
    {
        let app = create_test_router_with_komga(state.clone());
        let uri = format!("/komga/api/v1/series/{}/read-progress", series.id);
        let request = post_request_with_auth(&uri, &token);
        let (status, _) = make_raw_request(app, request).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    // Then, verify the series shows all books as read
    {
        let app = create_test_router_with_komga(state);
        let uri = format!("/komga/api/v1/series/{}", series.id);
        let request = get_request_with_auth(&uri, &token);
        let (status, response): (StatusCode, Option<KomgaSeriesDto>) =
            make_json_request(app, request).await;

        assert_eq!(status, StatusCode::OK);
        let series_dto = response.unwrap();

        // All books should be marked as read
        assert_eq!(series_dto.books_count, 3);
        assert_eq!(series_dto.books_read_count, 3);
        assert_eq!(series_dto.books_unread_count, 0);
        assert_eq!(series_dto.books_in_progress_count, 0);
    }
}
