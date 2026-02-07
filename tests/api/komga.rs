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
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    AlternateTitleRepository, BookMetadataRepository, BookRepository, ExternalLinkRepository,
    GenreRepository, LibraryRepository, ReadProgressRepository, SeriesMetadataRepository,
    SeriesRepository, TagRepository, UserRepository,
};
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

/// Test that on-deck orders by most recently read series first.
/// Series B is read more recently, so its next unread book should appear first.
#[tokio::test]
async fn test_komga_books_ondeck_ordered_by_recency() {
    let (db, temp_dir) = setup_test_db().await;

    // Create two series, each with 2 books
    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series_a = SeriesRepository::create(&db, library.id, "Alpha Series", None)
        .await
        .unwrap();
    let series_b = SeriesRepository::create(&db, library.id, "Beta Series", None)
        .await
        .unwrap();

    // Alpha series: book 1 + book 2
    let a1 = create_test_book(
        series_a.id,
        library.id,
        "/comics/Alpha/issue1.cbz",
        "issue1.cbz",
        "hash_a1",
        "cbz",
        20,
    );
    let created_a1 = BookRepository::create(&db, &a1, None).await.unwrap();
    let a2 = create_test_book(
        series_a.id,
        library.id,
        "/comics/Alpha/issue2.cbz",
        "issue2.cbz",
        "hash_a2",
        "cbz",
        20,
    );
    let created_a2 = BookRepository::create(&db, &a2, None).await.unwrap();

    // Beta series: book 1 + book 2
    let b1 = create_test_book(
        series_b.id,
        library.id,
        "/comics/Beta/issue1.cbz",
        "issue1.cbz",
        "hash_b1",
        "cbz",
        20,
    );
    let created_b1 = BookRepository::create(&db, &b1, None).await.unwrap();
    let b2 = create_test_book(
        series_b.id,
        library.id,
        "/comics/Beta/issue2.cbz",
        "issue2.cbz",
        "hash_b2",
        "cbz",
        20,
    );
    let created_b2 = BookRepository::create(&db, &b2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Get user ID for direct progress updates
    let user = UserRepository::get_by_username(&db, "admin")
        .await
        .unwrap()
        .unwrap();

    // Mark Alpha book 1 as read FIRST (older timestamp)
    ReadProgressRepository::mark_as_read(&db, user.id, created_a1.id, 20)
        .await
        .unwrap();

    // Small delay to ensure different timestamps
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Mark Beta book 1 as read SECOND (newer timestamp)
    ReadProgressRepository::mark_as_read(&db, user.id, created_b1.id, 20)
        .await
        .unwrap();

    // Now on-deck should show Beta book 2 FIRST (most recently read series),
    // then Alpha book 2
    let app = create_test_router_with_komga(state);
    let request = get_request_with_auth("/komga/api/v1/books/ondeck", &token);
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 2);
    assert_eq!(
        page.content[0].id,
        created_b2.id.to_string(),
        "Beta series (read more recently) should appear first on-deck"
    );
    assert_eq!(
        page.content[1].id,
        created_a2.id.to_string(),
        "Alpha series (read earlier) should appear second on-deck"
    );
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

/// Test that marking a book as completed without a page field sets current_page to page_count.
/// This is the exact format Komic sends: `{ "completed": true }` with no page.
#[tokio::test]
async fn test_komga_mark_completed_without_page_sets_last_page() {
    let (db, temp_dir) = setup_test_db().await;

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
        178, // 178 pages
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Mark as completed without sending a page — the exact Komic "mark as read" payload
    {
        let app = create_test_router_with_komga(state.clone());
        let uri = format!("/komga/api/v1/books/{}/read-progress", created_book.id);
        let body = r#"{"completed":true}"#;
        let request = patch_request_with_auth_json(&uri, &token, body);
        let (status, _) = make_raw_request(app, request).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    // Verify via the book DTO endpoint that progress page = page_count
    {
        let app = create_test_router_with_komga(state);
        let uri = format!("/komga/api/v1/books/{}", created_book.id);
        let request = get_request_with_auth(&uri, &token);
        let (status, response): (StatusCode, Option<KomgaBookDto>) =
            make_json_request(app, request).await;

        assert_eq!(status, StatusCode::OK);
        let book_dto = response.unwrap();
        assert!(book_dto.read_progress.is_some());
        let progress = book_dto.read_progress.unwrap();
        assert!(progress.completed, "Book should be marked as completed");
        assert_eq!(
            progress.page, 178,
            "Completed book without explicit page should have current_page = page_count"
        );
    }
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

// ============================================================================
// Sort and Filter Tests (Phase 3 - Komga sort/filter fixes)
// ============================================================================

/// Helper to create a book_metadata record with release date
async fn create_book_metadata_with_date(
    db: &sea_orm::DatabaseConnection,
    book_id: uuid::Uuid,
    title: Option<&str>,
    year: Option<i32>,
    month: Option<i32>,
    day: Option<i32>,
) {
    use codex::db::entities::book_metadata;
    let metadata = book_metadata::Model {
        id: uuid::Uuid::new_v4(),
        book_id,
        title: title.map(|s| s.to_string()),
        title_sort: title.map(|s| s.to_string()),
        number: None,
        summary: None,
        writer: None,
        penciller: None,
        inker: None,
        colorist: None,
        letterer: None,
        cover_artist: None,
        editor: None,
        publisher: None,
        imprint: None,
        genre: None,
        language_iso: None,
        format_detail: None,
        black_and_white: None,
        manga: None,
        year,
        month,
        day,
        volume: None,
        count: None,
        isbns: None,
        title_lock: false,
        title_sort_lock: false,
        number_lock: false,
        summary_lock: false,
        writer_lock: false,
        penciller_lock: false,
        inker_lock: false,
        colorist_lock: false,
        letterer_lock: false,
        cover_artist_lock: false,
        editor_lock: false,
        publisher_lock: false,
        imprint_lock: false,
        genre_lock: false,
        language_iso_lock: false,
        format_detail_lock: false,
        black_and_white_lock: false,
        manga_lock: false,
        year_lock: false,
        month_lock: false,
        day_lock: false,
        volume_lock: false,
        count_lock: false,
        isbns_lock: false,
        book_type: None,
        subtitle: None,
        authors_json: None,
        translator: None,
        edition: None,
        original_title: None,
        original_year: None,
        series_position: None,
        series_total: None,
        subjects: None,
        awards_json: None,
        custom_metadata: None,
        book_type_lock: false,
        subtitle_lock: false,
        authors_json_lock: false,
        translator_lock: false,
        edition_lock: false,
        original_title_lock: false,
        original_year_lock: false,
        series_position_lock: false,
        series_total_lock: false,
        subjects_lock: false,
        awards_json_lock: false,
        custom_metadata_lock: false,
        cover_lock: false,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    BookMetadataRepository::upsert(db, &metadata).await.unwrap();
}

/// Test that POST /books/list supports sort by createdDate ascending
#[tokio::test]
async fn test_komga_search_books_sort_by_created_date_asc() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    // Create books with different file sizes to distinguish them
    let book1 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash_sort_1",
        "cbz",
        10,
    );
    let created_book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue2.cbz",
        "issue2.cbz",
        "hash_sort_2",
        "cbz",
        20,
    );
    let created_book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by createdDate ascending
    let request = post_request_with_auth_json(
        "/komga/api/v1/books/list?sort=createdDate,asc",
        &token,
        "{}",
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 2);
    assert_eq!(page.content.len(), 2);
    // First created should be first
    assert_eq!(page.content[0].id, created_book1.id.to_string());
    assert_eq!(page.content[1].id, created_book2.id.to_string());
}

/// Test that POST /books/list supports sort by createdDate descending
#[tokio::test]
async fn test_komga_search_books_sort_by_created_date_desc() {
    let (db, temp_dir) = setup_test_db().await;

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
        "hash_desc_1",
        "cbz",
        10,
    );
    let created_book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue2.cbz",
        "issue2.cbz",
        "hash_desc_2",
        "cbz",
        20,
    );
    let created_book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by createdDate descending
    let request = post_request_with_auth_json(
        "/komga/api/v1/books/list?sort=createdDate,desc",
        &token,
        "{}",
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 2);
    // Descending: most recently created first
    assert_eq!(page.content[0].id, created_book2.id.to_string());
    assert_eq!(page.content[1].id, created_book1.id.to_string());
}

/// Test that POST /books/list supports sort by page count
#[tokio::test]
async fn test_komga_search_books_sort_by_page_count() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    // Book with 50 pages
    let book1 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/long.cbz",
        "long.cbz",
        "hash_pc_1",
        "cbz",
        50,
    );
    let created_book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    // Book with 10 pages
    let book2 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/short.cbz",
        "short.cbz",
        "hash_pc_2",
        "cbz",
        10,
    );
    let created_book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by pagesCount ascending
    let request = post_request_with_auth_json(
        "/komga/api/v1/books/list?sort=media.pagesCount,asc",
        &token,
        "{}",
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 2);
    // 10 pages before 50 pages
    assert_eq!(page.content[0].id, created_book2.id.to_string());
    assert_eq!(page.content[1].id, created_book1.id.to_string());
}

/// Test that POST /books/list supports sort by releaseDate
#[tokio::test]
async fn test_komga_search_books_sort_by_release_date() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    // Book released in 2020
    let book1 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/old.cbz",
        "old.cbz",
        "hash_rd_1",
        "cbz",
        20,
    );
    let created_book1 = BookRepository::create(&db, &book1, None).await.unwrap();
    create_book_metadata_with_date(
        &db,
        created_book1.id,
        Some("Old Issue"),
        Some(2020),
        Some(3),
        Some(15),
    )
    .await;

    // Book released in 2024
    let book2 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/new.cbz",
        "new.cbz",
        "hash_rd_2",
        "cbz",
        20,
    );
    let created_book2 = BookRepository::create(&db, &book2, None).await.unwrap();
    create_book_metadata_with_date(
        &db,
        created_book2.id,
        Some("New Issue"),
        Some(2024),
        Some(6),
        Some(1),
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by releaseDate ascending
    let request = post_request_with_auth_json(
        "/komga/api/v1/books/list?sort=metadata.releaseDate,asc",
        &token,
        "{}",
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 2);
    // 2020 before 2024
    assert_eq!(page.content[0].id, created_book1.id.to_string());
    assert_eq!(page.content[1].id, created_book2.id.to_string());
}

/// Test that POST /books/list supports readStatus IN_PROGRESS filter
#[tokio::test]
async fn test_komga_search_books_read_status_in_progress() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    // Create two books
    let book1 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue1.cbz",
        "issue1.cbz",
        "hash_rp_1",
        "cbz",
        50,
    );
    let created_book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue2.cbz",
        "issue2.cbz",
        "hash_rp_2",
        "cbz",
        50,
    );
    let created_book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Get user_id from token - create admin user and look up
    let user = UserRepository::get_by_username(&db, "admin")
        .await
        .unwrap()
        .unwrap();

    // Mark book1 as in-progress (page 10 of 50)
    ReadProgressRepository::upsert(&db, user.id, created_book1.id, 10, false)
        .await
        .unwrap();

    // book2 has no read progress (unread)

    let app = create_test_router_with_komga(state);

    // Filter by readStatus IN_PROGRESS via condition (Komic format with operator/value)
    let body =
        r#"{"condition":{"allOf":[{"readStatus":{"operator":"is","value":"IN_PROGRESS"}}]}}"#;
    let request = post_request_with_auth_json("/komga/api/v1/books/list", &token, body);
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 1);
    assert_eq!(page.content[0].id, created_book1.id.to_string());
}

/// Test that POST /books/list supports sort by readProgress.readDate (LastRead)
#[tokio::test]
async fn test_komga_search_books_sort_by_read_date() {
    let (db, temp_dir) = setup_test_db().await;

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
        "hash_lr_1",
        "cbz",
        50,
    );
    let created_book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/issue2.cbz",
        "issue2.cbz",
        "hash_lr_2",
        "cbz",
        50,
    );
    let created_book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let user = UserRepository::get_by_username(&db, "admin")
        .await
        .unwrap()
        .unwrap();

    // Mark book1 as in-progress first
    ReadProgressRepository::upsert(&db, user.id, created_book1.id, 5, false)
        .await
        .unwrap();

    // Then mark book2 as in-progress (more recently)
    ReadProgressRepository::upsert(&db, user.id, created_book2.id, 10, false)
        .await
        .unwrap();

    let app = create_test_router_with_komga(state);

    // Sort by readProgress.readDate descending (most recently read first)
    // Also filter by IN_PROGRESS so we only get books with read progress
    let body =
        r#"{"condition":{"allOf":[{"readStatus":{"operator":"is","value":"IN_PROGRESS"}}]}}"#;
    let request = post_request_with_auth_json(
        "/komga/api/v1/books/list?sort=readProgress.readDate,desc",
        &token,
        body,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 2);
    // Most recently read first (book2)
    assert_eq!(page.content[0].id, created_book2.id.to_string());
    assert_eq!(page.content[1].id, created_book1.id.to_string());
}

/// Test that POST /books/list supports releaseDate condition filter with "after" operator
#[tokio::test]
async fn test_komga_search_books_release_date_filter_after() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    // Book released 2020-06-15
    let book1 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/old.cbz",
        "old.cbz",
        "hash_rdf_1",
        "cbz",
        20,
    );
    let created_book1 = BookRepository::create(&db, &book1, None).await.unwrap();
    create_book_metadata_with_date(
        &db,
        created_book1.id,
        Some("Old Issue"),
        Some(2020),
        Some(6),
        Some(15),
    )
    .await;

    // Book released 2025-01-10
    let book2 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/new.cbz",
        "new.cbz",
        "hash_rdf_2",
        "cbz",
        20,
    );
    let created_book2 = BookRepository::create(&db, &book2, None).await.unwrap();
    create_book_metadata_with_date(
        &db,
        created_book2.id,
        Some("New Issue"),
        Some(2025),
        Some(1),
        Some(10),
    )
    .await;

    // Book with no release date
    let book3 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/undated.cbz",
        "undated.cbz",
        "hash_rdf_3",
        "cbz",
        20,
    );
    let created_book3 = BookRepository::create(&db, &book3, None).await.unwrap();
    create_book_metadata_with_date(
        &db,
        created_book3.id,
        Some("Undated Issue"),
        None,
        None,
        None,
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Filter: releaseDate after 2024-01-01
    let body = r#"{"condition":{"allOf":[{"releaseDate":{"dateTime":"2024-01-01T00:00:00Z","operator":"after"}}]}}"#;
    let request = post_request_with_auth_json("/komga/api/v1/books/list", &token, body);
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    // Only book2 (2025-01-10) should be after 2024-01-01
    // book1 (2020-06-15) is before, book3 has no date
    assert_eq!(page.total_elements, 1);
    assert_eq!(page.content[0].id, created_book2.id.to_string());
}

/// Test that POST /books/list supports releaseDate condition filter with "before" operator
#[tokio::test]
async fn test_komga_search_books_release_date_filter_before() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    // Book released 2020-06-15
    let book1 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/old.cbz",
        "old.cbz",
        "hash_rdb_1",
        "cbz",
        20,
    );
    let created_book1 = BookRepository::create(&db, &book1, None).await.unwrap();
    create_book_metadata_with_date(
        &db,
        created_book1.id,
        Some("Old Issue"),
        Some(2020),
        Some(6),
        Some(15),
    )
    .await;

    // Book released 2025-01-10
    let book2 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/new.cbz",
        "new.cbz",
        "hash_rdb_2",
        "cbz",
        20,
    );
    let created_book2 = BookRepository::create(&db, &book2, None).await.unwrap();
    create_book_metadata_with_date(
        &db,
        created_book2.id,
        Some("New Issue"),
        Some(2025),
        Some(1),
        Some(10),
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Filter: releaseDate before 2022-01-01
    let body = r#"{"condition":{"allOf":[{"releaseDate":{"dateTime":"2022-01-01T00:00:00Z","operator":"before"}}]}}"#;
    let request = post_request_with_auth_json("/komga/api/v1/books/list", &token, body);
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    // Only book1 (2020-06-15) should be before 2022-01-01
    assert_eq!(page.total_elements, 1);
    assert_eq!(page.content[0].id, created_book1.id.to_string());
}

/// Test that POST /books/list supports combined releaseDate filter with sort
#[tokio::test]
async fn test_komga_search_books_release_date_filter_with_sort() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();

    // Book released 2023-03-01
    let book1 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/mar2023.cbz",
        "mar2023.cbz",
        "hash_rds_1",
        "cbz",
        20,
    );
    let created_book1 = BookRepository::create(&db, &book1, None).await.unwrap();
    create_book_metadata_with_date(
        &db,
        created_book1.id,
        Some("March 2023"),
        Some(2023),
        Some(3),
        Some(1),
    )
    .await;

    // Book released 2024-06-15
    let book2 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/jun2024.cbz",
        "jun2024.cbz",
        "hash_rds_2",
        "cbz",
        20,
    );
    let created_book2 = BookRepository::create(&db, &book2, None).await.unwrap();
    create_book_metadata_with_date(
        &db,
        created_book2.id,
        Some("June 2024"),
        Some(2024),
        Some(6),
        Some(15),
    )
    .await;

    // Book released 2025-01-10
    let book3 = create_test_book(
        series.id,
        library.id,
        "/comics/Batman/jan2025.cbz",
        "jan2025.cbz",
        "hash_rds_3",
        "cbz",
        20,
    );
    let created_book3 = BookRepository::create(&db, &book3, None).await.unwrap();
    create_book_metadata_with_date(
        &db,
        created_book3.id,
        Some("January 2025"),
        Some(2025),
        Some(1),
        Some(10),
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Filter: releaseDate after 2023-01-01, sort by releaseDate desc
    let body = r#"{"condition":{"allOf":[{"releaseDate":{"dateTime":"2023-01-01T00:00:00Z","operator":"after"}}]}}"#;
    let request = post_request_with_auth_json(
        "/komga/api/v1/books/list?sort=metadata.releaseDate,desc",
        &token,
        body,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    // All 3 books are after 2023-01-01, sorted desc by release date
    assert_eq!(page.total_elements, 3);
    assert_eq!(page.content[0].id, created_book3.id.to_string()); // 2025-01-10
    assert_eq!(page.content[1].id, created_book2.id.to_string()); // 2024-06-15
    assert_eq!(page.content[2].id, created_book1.id.to_string()); // 2023-03-01
}

/// Test that POST /books/list with unknown sort field falls back to default (title)
#[tokio::test]
async fn test_komga_search_books_unknown_sort_uses_default() {
    let (db, temp_dir) = setup_test_db().await;

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
        "hash_unk_1",
        "cbz",
        20,
    );
    BookRepository::create(&db, &book1, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Use an unknown sort field - should not error
    let request = post_request_with_auth_json(
        "/komga/api/v1/books/list?sort=unknownField,asc",
        &token,
        "{}",
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 1);
}

/// Test that POST /books/list supports the compound sort "series,metadata.numberSort,asc"
#[tokio::test]
async fn test_komga_search_books_sort_by_series_compound() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create two series
    let series_a = SeriesRepository::create(&db, library.id, "Alpha Series", None)
        .await
        .unwrap();
    let series_b = SeriesRepository::create(&db, library.id, "Beta Series", None)
        .await
        .unwrap();

    // Book in Beta series
    let book1 = create_test_book(
        series_b.id,
        library.id,
        "/comics/Beta/issue1.cbz",
        "issue1.cbz",
        "hash_cs_1",
        "cbz",
        20,
    );
    let created_book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    // Book in Alpha series
    let book2 = create_test_book(
        series_a.id,
        library.id,
        "/comics/Alpha/issue1.cbz",
        "issue1.cbz",
        "hash_cs_2",
        "cbz",
        20,
    );
    let created_book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by series,metadata.numberSort ascending (alphabetical series order)
    let request = post_request_with_auth_json(
        "/komga/api/v1/books/list?sort=series,metadata.numberSort,asc",
        &token,
        "{}",
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 2);
    // Alpha Series before Beta Series
    assert_eq!(page.content[0].id, created_book2.id.to_string());
    assert_eq!(page.content[1].id, created_book1.id.to_string());
}

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

// ============================================================================
// Series Metadata Fields Tests (genres, tags, links, alternate titles, authors)
// ============================================================================

/// Test that GET /series/{id} returns genres from the database
#[tokio::test]
async fn test_komga_series_returns_genres() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Black Clover", None)
        .await
        .unwrap();

    // Add genres to the series
    GenreRepository::set_genres_for_series(
        &db,
        series.id,
        vec![
            "action".to_string(),
            "fantasy".to_string(),
            "comedy".to_string(),
        ],
    )
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

    // Genres should be populated and sorted alphabetically
    assert_eq!(dto.metadata.genres.len(), 3);
    assert_eq!(dto.metadata.genres[0], "action");
    assert_eq!(dto.metadata.genres[1], "comedy");
    assert_eq!(dto.metadata.genres[2], "fantasy");
}

/// Test that GET /series/{id} returns tags from the database
#[tokio::test]
async fn test_komga_series_returns_tags() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Black Clover", None)
        .await
        .unwrap();

    // Add tags to the series
    TagRepository::set_tags_for_series(
        &db,
        series.id,
        vec![
            "magic".to_string(),
            "shounen".to_string(),
            "demons".to_string(),
        ],
    )
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

    // Tags should be populated and sorted alphabetically
    assert_eq!(dto.metadata.tags.len(), 3);
    assert_eq!(dto.metadata.tags[0], "demons");
    assert_eq!(dto.metadata.tags[1], "magic");
    assert_eq!(dto.metadata.tags[2], "shounen");
}

/// Test that GET /series/{id} returns external links from the database
#[tokio::test]
async fn test_komga_series_returns_links() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Black Clover", None)
        .await
        .unwrap();

    // Add external links
    ExternalLinkRepository::create(
        &db,
        series.id,
        "anilist",
        "https://anilist.co/manga/86123",
        Some("86123"),
    )
    .await
    .unwrap();
    ExternalLinkRepository::create(
        &db,
        series.id,
        "myanimelist",
        "https://myanimelist.net/manga/86337",
        Some("86337"),
    )
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

    // Links should be populated
    assert_eq!(dto.metadata.links.len(), 2);

    let urls: Vec<&str> = dto.metadata.links.iter().map(|l| l.url.as_str()).collect();
    assert!(urls.contains(&"https://anilist.co/manga/86123"));
    assert!(urls.contains(&"https://myanimelist.net/manga/86337"));
}

/// Test that GET /series/{id} returns alternate titles from the database
#[tokio::test]
async fn test_komga_series_returns_alternate_titles() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Black Clover", None)
        .await
        .unwrap();

    // Add alternate titles
    AlternateTitleRepository::create(&db, series.id, "Native", "ブラッククローバー")
        .await
        .unwrap();
    AlternateTitleRepository::create(&db, series.id, "Roman", "Black Clover")
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

    // Alternate titles should be populated
    assert_eq!(dto.metadata.alternate_titles.len(), 2);

    let labels: Vec<&str> = dto
        .metadata
        .alternate_titles
        .iter()
        .map(|at| at.label.as_str())
        .collect();
    assert!(labels.contains(&"Native"));
    assert!(labels.contains(&"Roman"));

    // Verify the Japanese title is present
    let native = dto
        .metadata
        .alternate_titles
        .iter()
        .find(|at| at.label == "Native")
        .unwrap();
    assert_eq!(native.title, "ブラッククローバー");
}

/// Test that GET /series/{id} returns aggregated book authors in booksMetadata
#[tokio::test]
async fn test_komga_series_returns_books_metadata_authors() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Black Clover", None)
        .await
        .unwrap();

    // Create a book with author metadata
    let book = create_test_book(
        series.id,
        library.id,
        "/manga/Black Clover/v01.cbz",
        "v01.cbz",
        "hash_author_1",
        "cbz",
        200,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    // Create book metadata with author fields populated
    create_book_metadata_with_authors(
        &db,
        created_book.id,
        Some("Yuuki Tabata"), // writer
        Some("Yuuki Tabata"), // penciller
        Some("Yuuki Tabata"), // colorist
        None,                 // letterer
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let uri = format!("/komga/api/v1/series/{}", series.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, response): (StatusCode, Option<KomgaSeriesDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let dto = response.unwrap();

    // Authors should be aggregated from book metadata
    assert!(!dto.books_metadata.authors.is_empty());

    let author_roles: Vec<(&str, &str)> = dto
        .books_metadata
        .authors
        .iter()
        .map(|a| (a.name.as_str(), a.role.as_str()))
        .collect();

    assert!(author_roles.contains(&("Yuuki Tabata", "writer")));
    assert!(author_roles.contains(&("Yuuki Tabata", "penciller")));
    assert!(author_roles.contains(&("Yuuki Tabata", "colorist")));
}

/// Test that GET /series/{id} returns all metadata fields together
#[tokio::test]
async fn test_komga_series_returns_all_metadata_fields() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Set up all metadata types
    GenreRepository::set_genres_for_series(
        &db,
        series.id,
        vec!["action".to_string(), "drama".to_string()],
    )
    .await
    .unwrap();

    TagRepository::set_tags_for_series(
        &db,
        series.id,
        vec!["magic".to_string(), "fantasy".to_string()],
    )
    .await
    .unwrap();

    ExternalLinkRepository::create(
        &db,
        series.id,
        "anilist",
        "https://anilist.co/manga/1",
        None,
    )
    .await
    .unwrap();

    AlternateTitleRepository::create(&db, series.id, "Japanese", "テスト")
        .await
        .unwrap();

    // Create a book with author metadata
    let book = create_test_book(
        series.id,
        library.id,
        "/manga/Test/v01.cbz",
        "v01.cbz",
        "hash_all_1",
        "cbz",
        100,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();
    create_book_metadata_with_authors(&db, created_book.id, Some("Author A"), None, None, None)
        .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let uri = format!("/komga/api/v1/series/{}", series.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, response): (StatusCode, Option<KomgaSeriesDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let dto = response.unwrap();

    // All fields should be populated
    assert_eq!(dto.metadata.genres.len(), 2);
    assert_eq!(dto.metadata.tags.len(), 2);
    assert_eq!(dto.metadata.links.len(), 1);
    assert_eq!(dto.metadata.alternate_titles.len(), 1);
    assert_eq!(dto.books_metadata.authors.len(), 1);
    assert_eq!(dto.books_metadata.authors[0].name, "Author A");
    assert_eq!(dto.books_metadata.authors[0].role, "writer");
}

/// Test that series list endpoint also returns populated metadata fields
#[tokio::test]
async fn test_komga_list_series_returns_metadata_fields() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Black Clover", None)
        .await
        .unwrap();

    // Add genres and tags
    GenreRepository::set_genres_for_series(
        &db,
        series.id,
        vec!["action".to_string(), "fantasy".to_string()],
    )
    .await
    .unwrap();

    TagRepository::set_tags_for_series(&db, series.id, vec!["shounen".to_string()])
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
    assert_eq!(page.total_elements, 1);

    let dto = &page.content[0];
    assert_eq!(dto.metadata.genres.len(), 2);
    assert!(dto.metadata.genres.contains(&"action".to_string()));
    assert!(dto.metadata.genres.contains(&"fantasy".to_string()));
    assert_eq!(dto.metadata.tags.len(), 1);
    assert_eq!(dto.metadata.tags[0], "shounen");
}

// ============================================================================
// Book Metadata Fields Tests (authors, summary, release_date, tags)
// ============================================================================

/// Test that GET /books/{id} returns authors from book metadata
#[tokio::test]
async fn test_komga_book_returns_metadata_authors() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/manga/Test/v01.cbz",
        "v01.cbz",
        "hash_bm_auth_1",
        "cbz",
        200,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    create_book_metadata_with_authors(
        &db,
        created_book.id,
        Some("Author A"),
        Some("Artist B"),
        Some("Colorist C"),
        Some("Letterer D"),
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let uri = format!("/komga/api/v1/books/{}", created_book.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, response): (StatusCode, Option<KomgaBookDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let dto = response.unwrap();

    assert_eq!(dto.metadata.authors.len(), 4);
    let author_roles: Vec<(&str, &str)> = dto
        .metadata
        .authors
        .iter()
        .map(|a| (a.name.as_str(), a.role.as_str()))
        .collect();
    assert!(author_roles.contains(&("Author A", "writer")));
    assert!(author_roles.contains(&("Artist B", "penciller")));
    assert!(author_roles.contains(&("Colorist C", "colorist")));
    assert!(author_roles.contains(&("Letterer D", "letterer")));
}

/// Test that GET /books/{id} returns summary, release_date, and tags
#[tokio::test]
async fn test_komga_book_returns_metadata_fields() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/manga/Test/v01.cbz",
        "v01.cbz",
        "hash_bm_fields_1",
        "cbz",
        200,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    // Create metadata with summary, release date, genre, and title
    create_book_metadata_full(
        &db,
        created_book.id,
        Some("Chapter 1: The Beginning"),
        Some("The adventure begins here."),
        Some(2024),
        Some(6),
        Some(15),
        Some("action, fantasy, comedy"),
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let uri = format!("/komga/api/v1/books/{}", created_book.id);
    let request = get_request_with_auth(&uri, &token);
    let (status, response): (StatusCode, Option<KomgaBookDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let dto = response.unwrap();

    assert_eq!(dto.metadata.title, "Chapter 1: The Beginning");
    assert_eq!(dto.metadata.summary, "The adventure begins here.");
    assert_eq!(dto.metadata.release_date, Some("2024-06-15".to_string()));
    assert_eq!(dto.metadata.tags.len(), 3);
    assert!(dto.metadata.tags.contains(&"action".to_string()));
    assert!(dto.metadata.tags.contains(&"fantasy".to_string()));
    assert!(dto.metadata.tags.contains(&"comedy".to_string()));
}

/// Test that POST /books/list returns metadata for all books in results
#[tokio::test]
async fn test_komga_search_books_returns_metadata() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book = create_test_book(
        series.id,
        library.id,
        "/manga/Test/v01.cbz",
        "v01.cbz",
        "hash_bm_search_1",
        "cbz",
        100,
    );
    let created_book = BookRepository::create(&db, &book, None).await.unwrap();

    create_book_metadata_with_authors(&db, created_book.id, Some("Writer X"), None, None, None)
        .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let request = post_request_with_auth_json("/komga/api/v1/books/list", &token, "{}");
    let (status, response): (StatusCode, Option<KomgaPage<KomgaBookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.total_elements, 1);

    let dto = &page.content[0];
    assert_eq!(dto.metadata.authors.len(), 1);
    assert_eq!(dto.metadata.authors[0].name, "Writer X");
    assert_eq!(dto.metadata.authors[0].role, "writer");
}

// Helper to create book metadata with full fields
#[allow(clippy::too_many_arguments)]
async fn create_book_metadata_full(
    db: &sea_orm::DatabaseConnection,
    book_id: uuid::Uuid,
    title: Option<&str>,
    summary: Option<&str>,
    year: Option<i32>,
    month: Option<i32>,
    day: Option<i32>,
    genre: Option<&str>,
) {
    use codex::db::entities::book_metadata;
    let metadata = book_metadata::Model {
        id: uuid::Uuid::new_v4(),
        book_id,
        title: title.map(|s| s.to_string()),
        title_sort: title.map(|s| s.to_string()),
        number: None,
        summary: summary.map(|s| s.to_string()),
        writer: None,
        penciller: None,
        inker: None,
        colorist: None,
        letterer: None,
        cover_artist: None,
        editor: None,
        publisher: None,
        imprint: None,
        genre: genre.map(|s| s.to_string()),
        language_iso: None,
        format_detail: None,
        black_and_white: None,
        manga: None,
        year,
        month,
        day,
        volume: None,
        count: None,
        isbns: None,
        title_lock: false,
        title_sort_lock: false,
        number_lock: false,
        summary_lock: false,
        writer_lock: false,
        penciller_lock: false,
        inker_lock: false,
        colorist_lock: false,
        letterer_lock: false,
        cover_artist_lock: false,
        editor_lock: false,
        publisher_lock: false,
        imprint_lock: false,
        genre_lock: false,
        language_iso_lock: false,
        format_detail_lock: false,
        black_and_white_lock: false,
        manga_lock: false,
        year_lock: false,
        month_lock: false,
        day_lock: false,
        volume_lock: false,
        count_lock: false,
        isbns_lock: false,
        book_type: None,
        subtitle: None,
        authors_json: None,
        translator: None,
        edition: None,
        original_title: None,
        original_year: None,
        series_position: None,
        series_total: None,
        subjects: None,
        awards_json: None,
        custom_metadata: None,
        book_type_lock: false,
        subtitle_lock: false,
        authors_json_lock: false,
        translator_lock: false,
        edition_lock: false,
        original_title_lock: false,
        original_year_lock: false,
        series_position_lock: false,
        series_total_lock: false,
        subjects_lock: false,
        awards_json_lock: false,
        custom_metadata_lock: false,
        cover_lock: false,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    BookMetadataRepository::upsert(db, &metadata).await.unwrap();
}

// Helper to create book metadata with author role fields
async fn create_book_metadata_with_authors(
    db: &sea_orm::DatabaseConnection,
    book_id: uuid::Uuid,
    writer: Option<&str>,
    penciller: Option<&str>,
    colorist: Option<&str>,
    letterer: Option<&str>,
) {
    use codex::db::entities::book_metadata;
    let metadata = book_metadata::Model {
        id: uuid::Uuid::new_v4(),
        book_id,
        title: None,
        title_sort: None,
        number: None,
        summary: None,
        writer: writer.map(|s| s.to_string()),
        penciller: penciller.map(|s| s.to_string()),
        inker: None,
        colorist: colorist.map(|s| s.to_string()),
        letterer: letterer.map(|s| s.to_string()),
        cover_artist: None,
        editor: None,
        publisher: None,
        imprint: None,
        genre: None,
        language_iso: None,
        format_detail: None,
        black_and_white: None,
        manga: None,
        year: None,
        month: None,
        day: None,
        volume: None,
        count: None,
        isbns: None,
        title_lock: false,
        title_sort_lock: false,
        number_lock: false,
        summary_lock: false,
        writer_lock: false,
        penciller_lock: false,
        inker_lock: false,
        colorist_lock: false,
        letterer_lock: false,
        cover_artist_lock: false,
        editor_lock: false,
        publisher_lock: false,
        imprint_lock: false,
        genre_lock: false,
        language_iso_lock: false,
        format_detail_lock: false,
        black_and_white_lock: false,
        manga_lock: false,
        year_lock: false,
        month_lock: false,
        day_lock: false,
        volume_lock: false,
        count_lock: false,
        isbns_lock: false,
        book_type: None,
        subtitle: None,
        authors_json: None,
        translator: None,
        edition: None,
        original_title: None,
        original_year: None,
        series_position: None,
        series_total: None,
        subjects: None,
        awards_json: None,
        custom_metadata: None,
        book_type_lock: false,
        subtitle_lock: false,
        authors_json_lock: false,
        translator_lock: false,
        edition_lock: false,
        original_title_lock: false,
        original_year_lock: false,
        series_position_lock: false,
        series_total_lock: false,
        subjects_lock: false,
        awards_json_lock: false,
        custom_metadata_lock: false,
        cover_lock: false,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    BookMetadataRepository::upsert(db, &metadata).await.unwrap();
}

// ============================================================================
// Series Sort Tests (Komga series list sorting)
// ============================================================================

/// Test that POST /series/list with sort=metadata.titleSort,asc sorts by title_sort ascending.
///
/// This creates series with explicit title_sort values that differ from alphabetical title order,
/// verifying that the sort actually uses title_sort (not title).
#[tokio::test]
async fn test_komga_search_series_sort_by_title_sort_asc() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with names out of alphabetical order
    let series_z = SeriesRepository::create(&db, library.id, "Zebra", None)
        .await
        .unwrap();
    let series_a = SeriesRepository::create(&db, library.id, "Apple", None)
        .await
        .unwrap();
    let series_m = SeriesRepository::create(&db, library.id, "Mango", None)
        .await
        .unwrap();

    // Set explicit title_sort values to control sort order
    // title_sort should determine the order, not the title itself
    SeriesMetadataRepository::update_title(
        &db,
        series_z.id,
        "Zebra".to_string(),
        Some("01 Zebra".to_string()),
    )
    .await
    .unwrap();
    SeriesMetadataRepository::update_title(
        &db,
        series_a.id,
        "Apple".to_string(),
        Some("03 Apple".to_string()),
    )
    .await
    .unwrap();
    SeriesMetadataRepository::update_title(
        &db,
        series_m.id,
        "Mango".to_string(),
        Some("02 Mango".to_string()),
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by metadata.titleSort ascending
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20&sort=metadata.titleSort,asc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 3);
    // Expected order by title_sort: "01 Zebra", "02 Mango", "03 Apple"
    assert_eq!(page.content[0].name, "Zebra");
    assert_eq!(page.content[1].name, "Mango");
    assert_eq!(page.content[2].name, "Apple");
}

/// Test that POST /series/list with sort=metadata.titleSort,desc sorts by title_sort descending
#[tokio::test]
async fn test_komga_search_series_sort_by_title_sort_desc() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    let series_z = SeriesRepository::create(&db, library.id, "Zebra", None)
        .await
        .unwrap();
    let series_a = SeriesRepository::create(&db, library.id, "Apple", None)
        .await
        .unwrap();
    let series_m = SeriesRepository::create(&db, library.id, "Mango", None)
        .await
        .unwrap();

    SeriesMetadataRepository::update_title(
        &db,
        series_z.id,
        "Zebra".to_string(),
        Some("01 Zebra".to_string()),
    )
    .await
    .unwrap();
    SeriesMetadataRepository::update_title(
        &db,
        series_a.id,
        "Apple".to_string(),
        Some("03 Apple".to_string()),
    )
    .await
    .unwrap();
    SeriesMetadataRepository::update_title(
        &db,
        series_m.id,
        "Mango".to_string(),
        Some("02 Mango".to_string()),
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by metadata.titleSort descending
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20&sort=metadata.titleSort,desc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 3);
    // Expected order by title_sort desc: "03 Apple", "02 Mango", "01 Zebra"
    assert_eq!(page.content[0].name, "Apple");
    assert_eq!(page.content[1].name, "Mango");
    assert_eq!(page.content[2].name, "Zebra");
}

/// Test that GET /series with sort=metadata.titleSort,asc also works (not just POST)
#[tokio::test]
async fn test_komga_list_series_sort_by_title_sort_asc_get() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series out of alphabetical order
    SeriesRepository::create(&db, library.id, "Zebra", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Apple", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Mango", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by metadata.titleSort ascending via GET
    let request = get_request_with_auth(
        "/komga/api/v1/series?page=0&size=20&sort=metadata.titleSort,asc",
        &token,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 3);
    // Default title_sort is None, so should fall back to title and sort alphabetically
    assert_eq!(page.content[0].name, "Apple");
    assert_eq!(page.content[1].name, "Mango");
    assert_eq!(page.content[2].name, "Zebra");
}

/// Test that POST /series/list with sort=createdDate,asc sorts by creation date ascending
#[tokio::test]
async fn test_komga_search_series_sort_by_created_date_asc() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with sequential timestamps
    let series1 = SeriesRepository::create(&db, library.id, "First Created", None)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let series2 = SeriesRepository::create(&db, library.id, "Second Created", None)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let series3 = SeriesRepository::create(&db, library.id, "Third Created", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by createdDate ascending (oldest first)
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20&sort=createdDate,asc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 3);
    assert_eq!(page.content[0].id, series1.id.to_string());
    assert_eq!(page.content[1].id, series2.id.to_string());
    assert_eq!(page.content[2].id, series3.id.to_string());
}

/// Test that POST /series/list with sort=createdDate,desc sorts by creation date descending
#[tokio::test]
async fn test_komga_search_series_sort_by_created_date_desc() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "First Created", None)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let series2 = SeriesRepository::create(&db, library.id, "Second Created", None)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let series3 = SeriesRepository::create(&db, library.id, "Third Created", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by createdDate descending (newest first)
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20&sort=createdDate,desc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 3);
    // Newest first
    assert_eq!(page.content[0].id, series3.id.to_string());
    assert_eq!(page.content[1].id, series2.id.to_string());
    assert_eq!(page.content[2].id, series1.id.to_string());
}

/// Test that POST /series/list with sort=lastModifiedDate,desc sorts by update date descending
#[tokio::test]
async fn test_komga_search_series_sort_by_last_modified_date_desc() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create three series
    let series1 = SeriesRepository::create(&db, library.id, "Series A", None)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let series2 = SeriesRepository::create(&db, library.id, "Series B", None)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let series3 = SeriesRepository::create(&db, library.id, "Series C", None)
        .await
        .unwrap();

    // Update series1 last so it has the most recent updated_at
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    SeriesRepository::update_name(&db, series1.id, "Series A Updated")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by lastModifiedDate descending (most recently modified first)
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20&sort=lastModifiedDate,desc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 3);
    // series1 was updated last, so it should be first
    assert_eq!(page.content[0].id, series1.id.to_string());
}

/// Test that POST /series/list with sort=lastModifiedDate,asc sorts by update date ascending
#[tokio::test]
async fn test_komga_search_series_sort_by_last_modified_date_asc() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Series A", None)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let series2 = SeriesRepository::create(&db, library.id, "Series B", None)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let series3 = SeriesRepository::create(&db, library.id, "Series C", None)
        .await
        .unwrap();

    // Update series1 last so it has the most recent updated_at
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    SeriesRepository::update_name(&db, series1.id, "Series A Updated")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by lastModifiedDate ascending (least recently modified first)
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20&sort=lastModifiedDate,asc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 3);
    // series2 was created second and never updated, series3 was created last but not updated after
    // series1 was updated most recently, so it should be last
    assert_eq!(page.content[2].id, series1.id.to_string());
}

/// Test that POST /series/list with sort=metadata.releaseDate,desc sorts by year descending
#[tokio::test]
async fn test_komga_search_series_sort_by_release_date_desc() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    let series_old = SeriesRepository::create(&db, library.id, "Old Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_year(&db, series_old.id, Some(1990))
        .await
        .unwrap();

    let series_new = SeriesRepository::create(&db, library.id, "New Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_year(&db, series_new.id, Some(2024))
        .await
        .unwrap();

    let series_mid = SeriesRepository::create(&db, library.id, "Mid Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_year(&db, series_mid.id, Some(2010))
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by releaseDate descending (newest year first)
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20&sort=metadata.releaseDate,desc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 3);
    // Newest year first
    assert_eq!(page.content[0].id, series_new.id.to_string());
    assert_eq!(page.content[1].id, series_mid.id.to_string());
    assert_eq!(page.content[2].id, series_old.id.to_string());
}

/// Test that POST /series/list with sort=metadata.releaseDate,asc sorts by year ascending
#[tokio::test]
async fn test_komga_search_series_sort_by_release_date_asc() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    let series_old = SeriesRepository::create(&db, library.id, "Old Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_year(&db, series_old.id, Some(1990))
        .await
        .unwrap();

    let series_new = SeriesRepository::create(&db, library.id, "New Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_year(&db, series_new.id, Some(2024))
        .await
        .unwrap();

    let series_mid = SeriesRepository::create(&db, library.id, "Mid Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_year(&db, series_mid.id, Some(2010))
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by releaseDate ascending (oldest year first)
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20&sort=metadata.releaseDate,asc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 3);
    // Oldest year first
    assert_eq!(page.content[0].id, series_old.id.to_string());
    assert_eq!(page.content[1].id, series_mid.id.to_string());
    assert_eq!(page.content[2].id, series_new.id.to_string());
}

/// Test that POST /series/list with sort=lastReadDate,desc sorts by most recently read
#[tokio::test]
async fn test_komga_search_series_sort_by_last_read_date_desc() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create three series, each with one book
    let series1 = SeriesRepository::create(&db, library.id, "Series A", None)
        .await
        .unwrap();
    let book1 = create_test_book(
        series1.id,
        library.id,
        "/comics/a/book1.cbz",
        "book1.cbz",
        "hash_read_sort_1",
        "cbz",
        10,
    );
    let created_book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let series2 = SeriesRepository::create(&db, library.id, "Series B", None)
        .await
        .unwrap();
    let book2 = create_test_book(
        series2.id,
        library.id,
        "/comics/b/book2.cbz",
        "book2.cbz",
        "hash_read_sort_2",
        "cbz",
        10,
    );
    let created_book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let series3 = SeriesRepository::create(&db, library.id, "Series C", None)
        .await
        .unwrap();
    let book3 = create_test_book(
        series3.id,
        library.id,
        "/comics/c/book3.cbz",
        "book3.cbz",
        "hash_read_sort_3",
        "cbz",
        10,
    );
    let created_book3 = BookRepository::create(&db, &book3, None).await.unwrap();

    // Create admin user and read books in specific order
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let admin = UserRepository::get_by_username(&db, "admin")
        .await
        .unwrap()
        .unwrap();

    // Read series2 first, then series3, then series1
    ReadProgressRepository::upsert(&db, admin.id, created_book2.id, 5, false)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    ReadProgressRepository::upsert(&db, admin.id, created_book3.id, 3, false)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    ReadProgressRepository::upsert(&db, admin.id, created_book1.id, 7, false)
        .await
        .unwrap();

    let app = create_test_router_with_komga(state);

    // Sort by lastReadDate descending (most recently read first)
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20&sort=lastReadDate,desc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 3);
    // Most recently read first: series1, series3, series2
    assert_eq!(page.content[0].id, series1.id.to_string());
    assert_eq!(page.content[1].id, series3.id.to_string());
    assert_eq!(page.content[2].id, series2.id.to_string());
}

/// Test that POST /series/list with sort=lastReadDate,asc sorts by least recently read
#[tokio::test]
async fn test_komga_search_series_sort_by_last_read_date_asc() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Series A", None)
        .await
        .unwrap();
    let book1 = create_test_book(
        series1.id,
        library.id,
        "/comics/a/book1.cbz",
        "book1.cbz",
        "hash_read_asc_1",
        "cbz",
        10,
    );
    let created_book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let series2 = SeriesRepository::create(&db, library.id, "Series B", None)
        .await
        .unwrap();
    let book2 = create_test_book(
        series2.id,
        library.id,
        "/comics/b/book2.cbz",
        "book2.cbz",
        "hash_read_asc_2",
        "cbz",
        10,
    );
    let created_book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let series3 = SeriesRepository::create(&db, library.id, "Series C", None)
        .await
        .unwrap();
    let book3 = create_test_book(
        series3.id,
        library.id,
        "/comics/c/book3.cbz",
        "book3.cbz",
        "hash_read_asc_3",
        "cbz",
        10,
    );
    let created_book3 = BookRepository::create(&db, &book3, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let admin = UserRepository::get_by_username(&db, "admin")
        .await
        .unwrap()
        .unwrap();

    // Read in order: series2, series3, series1
    ReadProgressRepository::upsert(&db, admin.id, created_book2.id, 5, false)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    ReadProgressRepository::upsert(&db, admin.id, created_book3.id, 3, false)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    ReadProgressRepository::upsert(&db, admin.id, created_book1.id, 7, false)
        .await
        .unwrap();

    let app = create_test_router_with_komga(state);

    // Sort by lastReadDate ascending (least recently read first)
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20&sort=lastReadDate,asc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 3);
    // Least recently read first: series2, series3, series1
    assert_eq!(page.content[0].id, series2.id.to_string());
    assert_eq!(page.content[1].id, series3.id.to_string());
    assert_eq!(page.content[2].id, series1.id.to_string());
}

/// Test that POST /series/list with unknown sort field falls back to default title sort
#[tokio::test]
async fn test_komga_search_series_sort_unknown_field_uses_default() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library.id, "Zebra", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Apple", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Mango", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Unknown sort field should fall back to default (title sort ascending)
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20&sort=unknownField,asc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 3);
    // Default sort is title ascending
    assert_eq!(page.content[0].name, "Apple");
    assert_eq!(page.content[1].name, "Mango");
    assert_eq!(page.content[2].name, "Zebra");
}

/// Test that POST /series/list with no sort parameter uses default title sort
#[tokio::test]
async fn test_komga_search_series_sort_no_param_uses_default() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library.id, "Zebra", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Apple", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Mango", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // No sort parameter at all
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 3);
    // Default sort is title ascending
    assert_eq!(page.content[0].name, "Apple");
    assert_eq!(page.content[1].name, "Mango");
    assert_eq!(page.content[2].name, "Zebra");
}

/// Test that title sort with pagination works correctly across pages
/// (sort happens at database level BEFORE pagination)
#[tokio::test]
async fn test_komga_search_series_sort_title_with_pagination() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create 5 series with title_sort values
    for (name, sort_key) in [
        ("Echo", "05"),
        ("Alpha", "01"),
        ("Charlie", "03"),
        ("Bravo", "02"),
        ("Delta", "04"),
    ] {
        let series = SeriesRepository::create(&db, library.id, name, None)
            .await
            .unwrap();
        SeriesMetadataRepository::update_title(
            &db,
            series.id,
            name.to_string(),
            Some(sort_key.to_string()),
        )
        .await
        .unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Get first page (2 items) sorted by title_sort ascending
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=2&sort=metadata.titleSort,asc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let page1 = response.unwrap();
    assert_eq!(page1.total_elements, 5);
    assert_eq!(page1.content.len(), 2);
    // First page: "01" (Alpha), "02" (Bravo)
    assert_eq!(page1.content[0].name, "Alpha");
    assert_eq!(page1.content[1].name, "Bravo");

    // Get second page
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=1&size=2&sort=metadata.titleSort,asc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let page2 = response.unwrap();
    assert_eq!(page2.content.len(), 2);
    // Second page: "03" (Charlie), "04" (Delta)
    assert_eq!(page2.content[0].name, "Charlie");
    assert_eq!(page2.content[1].name, "Delta");

    // Get third page
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=2&size=2&sort=metadata.titleSort,asc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page3 = response.unwrap();
    assert_eq!(page3.content.len(), 1);
    // Third page: "05" (Echo)
    assert_eq!(page3.content[0].name, "Echo");
}

/// Test that title sort works correctly when title_sort is NULL for all series.
///
/// In production, most series have title_sort = NULL (the default).
/// The sort should fall back to sorting by title when title_sort is NULL.
/// This test reproduces the production bug where NULL title_sort causes
/// series to appear in insertion order rather than alphabetical order.
#[tokio::test]
async fn test_komga_search_series_sort_by_title_with_null_title_sort() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with names that would be sorted differently than insertion order
    // Deliberately inserting out of alphabetical order to catch insertion-order bugs
    // These all have title_sort = NULL (the default), which is the common production case
    SeriesRepository::create(&db, library.id, "Kaiju No. 8", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "A Couple of Cuckoos", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "+Anima", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Fairy Tail's Fairy Girls", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "01 Locke & Key", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Air Gear", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "ALIVE: The Final Evolution", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Shinobi Life", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    // Sort by metadata.titleSort ascending - with all NULL title_sort values,
    // this should fall back to sorting by title alphabetically
    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20&sort=metadata.titleSort,asc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 8);

    // When title_sort is NULL for all series, the sort should fall back to title.
    // The database sorts case-sensitively by default (uppercase before lowercase in SQLite),
    // so the expected order is based on raw byte/codepoint ordering.
    let titles: Vec<&str> = page.content.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        titles,
        vec![
            "+Anima",
            "01 Locke & Key",
            "A Couple of Cuckoos",
            "ALIVE: The Final Evolution",
            "Air Gear",
            "Fairy Tail's Fairy Girls",
            "Kaiju No. 8",
            "Shinobi Life",
        ],
        "Series should be sorted alphabetically by title when title_sort is NULL, got: {:?}",
        titles
    );
}

/// Test that title sort works correctly with a mix of NULL and non-NULL title_sort values.
///
/// In production, some series may have title_sort set (e.g., from ComicInfo.xml metadata)
/// while others have NULL. The sort should use title_sort where available and fall back
/// to title for NULLs. NULLs should not cluster at the beginning/end.
#[tokio::test]
async fn test_komga_search_series_sort_by_title_mixed_null_and_set() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();

    // Series with NULL title_sort (default)
    SeriesRepository::create(&db, library.id, "Batman", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Daredevil", None)
        .await
        .unwrap();

    // Series with explicit title_sort that differs from title
    let series_the = SeriesRepository::create(&db, library.id, "The Amazing Spider-Man", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_title(
        &db,
        series_the.id,
        "The Amazing Spider-Man".to_string(),
        Some("Amazing Spider-Man, The".to_string()),
    )
    .await
    .unwrap();

    // Another series with NULL title_sort
    SeriesRepository::create(&db, library.id, "Cable", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_komga(state);

    let request = post_request_with_auth_json(
        "/komga/api/v1/series/list?page=0&size=20&sort=metadata.titleSort,asc",
        &token,
        r#"{"condition":{"allOf":[]},"fullTextSearch":""}"#,
    );
    let (status, response): (StatusCode, Option<KomgaPage<KomgaSeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page = response.unwrap();
    assert_eq!(page.content.len(), 4);

    // Expected order:
    // "The Amazing Spider-Man" (title_sort="Amazing Spider-Man, The" -> sorts under A)
    // "Batman" (title_sort=NULL, falls back to title "Batman")
    // "Cable" (title_sort=NULL, falls back to title "Cable")
    // "Daredevil" (title_sort=NULL, falls back to title "Daredevil")
    let titles: Vec<&str> = page.content.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        titles,
        vec!["The Amazing Spider-Man", "Batman", "Cable", "Daredevil",],
        "Series with title_sort should be interleaved with NULL title_sort series (sorted by title), got: {:?}",
        titles
    );
}
