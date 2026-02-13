#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::book::BookDto;
use codex::api::routes::v1::dto::series::{SearchSeriesRequest, SeriesDto, SeriesListResponse};
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    BookRepository, LibraryRepository, SeriesMetadataRepository, SeriesRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;

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

// ============================================================================
// List Series Tests
// ============================================================================

#[tokio::test]
async fn test_list_series_all() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/series", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_response = response.unwrap();
    assert_eq!(series_response.data.len(), 2);
    assert_eq!(series_response.total, 2);
    assert_eq!(series_response.page, 1); // 1-indexed pagination
}

#[tokio::test]
async fn test_list_series_by_library() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create two libraries with series
    let library1 = LibraryRepository::create(&db, "Library 1", "/lib1", ScanningStrategy::Default)
        .await
        .unwrap();

    let library2 = LibraryRepository::create(&db, "Library 2", "/lib2", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library1.id, "Lib1 Series 1", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library1.id, "Lib1 Series 2", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library2.id, "Lib2 Series 1", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Query all series (should return paginated response)
    let request = get_request_with_auth("/api/v1/series", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_response = response.unwrap();
    assert_eq!(series_response.data.len(), 3);
    assert_eq!(series_response.total, 3);
}

#[tokio::test]
async fn test_list_series_without_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = get_request("/api/v1/series");
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_list_series_pagination() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create 5 series
    for i in 1..=5 {
        SeriesRepository::create(&db, library.id, &format!("Series {}", i), None)
            .await
            .unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Test first page with page size of 2 (1-indexed)
    let request = get_request_with_auth("/api/v1/series?page=1&pageSize=2", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let page1 = response.unwrap();
    assert_eq!(page1.data.len(), 2);
    assert_eq!(page1.total, 5);
    assert_eq!(page1.page, 1);
    assert_eq!(page1.page_size, 2);

    // Test second page
    let request = get_request_with_auth("/api/v1/series?page=2&pageSize=2", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page2 = response.unwrap();
    assert_eq!(page2.data.len(), 2);
    assert_eq!(page2.total, 5);
    assert_eq!(page2.page, 2);
}

// ============================================================================
// Sort Series Tests
// ============================================================================

#[tokio::test]
async fn test_list_library_series_sort_by_name_asc() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with different names (out of alphabetical order)
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
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/series?sort=name,asc", library.id),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 3);
    assert_eq!(series_list.data[0].title, "Apple");
    assert_eq!(series_list.data[1].title, "Mango");
    assert_eq!(series_list.data[2].title, "Zebra");
}

#[tokio::test]
async fn test_list_library_series_sort_by_name_desc() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library.id, "Apple", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Mango", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Zebra", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/series?sort=name,desc", library.id),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 3);
    assert_eq!(series_list.data[0].title, "Zebra");
    assert_eq!(series_list.data[1].title, "Mango");
    assert_eq!(series_list.data[2].title, "Apple");
}

#[tokio::test]
async fn test_list_library_series_sort_by_date_added() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series (they will have sequential created_at timestamps)
    let series1 = SeriesRepository::create(&db, library.id, "First", None)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    let series2 = SeriesRepository::create(&db, library.id, "Second", None)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    let series3 = SeriesRepository::create(&db, library.id, "Third", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Sort by date added descending (newest first)
    let request = get_request_with_auth(
        &format!(
            "/api/v1/libraries/{}/series?sort=date_added,desc",
            library.id
        ),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 3);
    assert_eq!(series_list.data[0].id, series3.id); // Newest
    assert_eq!(series_list.data[1].id, series2.id);
    assert_eq!(series_list.data[2].id, series1.id); // Oldest
}

#[tokio::test]
async fn test_list_library_series_sort_by_release_date() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series and update their years via metadata repository
    let series1 = SeriesRepository::create(&db, library.id, "Old Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_year(&db, series1.id, Some(1990))
        .await
        .unwrap();

    let series2 = SeriesRepository::create(&db, library.id, "New Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_year(&db, series2.id, Some(2024))
        .await
        .unwrap();

    let series3 = SeriesRepository::create(&db, library.id, "Mid Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_year(&db, series3.id, Some(2010))
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Sort by release date descending (newest first)
    let request = get_request_with_auth(
        &format!(
            "/api/v1/libraries/{}/series?sort=release_date,desc",
            library.id
        ),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 3);
    assert_eq!(series_list.data[0].year, Some(2024)); // Newest
    assert_eq!(series_list.data[1].year, Some(2010));
    assert_eq!(series_list.data[2].year, Some(1990)); // Oldest
}

#[tokio::test]
async fn test_list_library_series_sort_by_book_count_asc() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create 3 series with different numbers of books
    let series1 = SeriesRepository::create(&db, library.id, "Many Books", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Few Books", None)
        .await
        .unwrap();
    let _series3 = SeriesRepository::create(&db, library.id, "No Books", None)
        .await
        .unwrap();

    // Add 3 books to series1
    for i in 0..3 {
        let book = create_test_book(
            series1.id,
            library.id,
            &format!("/lib/many/book{}.cbz", i),
            &format!("book{}.cbz", i),
            None,
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    // Add 1 book to series2
    let book = create_test_book(
        series2.id,
        library.id,
        "/lib/few/book0.cbz",
        "book0.cbz",
        None,
    );
    BookRepository::create(&db, &book, None).await.unwrap();

    // series3 has 0 books

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Sort by book_count ascending (fewest first)
    let request = get_request_with_auth(
        &format!(
            "/api/v1/libraries/{}/series?sort=book_count,asc",
            library.id
        ),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 3);
    assert_eq!(series_list.data[0].title, "No Books"); // 0 books
    assert_eq!(series_list.data[0].book_count, 0);
    assert_eq!(series_list.data[1].title, "Few Books"); // 1 book
    assert_eq!(series_list.data[1].book_count, 1);
    assert_eq!(series_list.data[2].title, "Many Books"); // 3 books
    assert_eq!(series_list.data[2].book_count, 3);
}

#[tokio::test]
async fn test_list_library_series_sort_by_book_count_desc() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create 3 series with different numbers of books
    let series1 = SeriesRepository::create(&db, library.id, "Many Books", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Few Books", None)
        .await
        .unwrap();
    let _series3 = SeriesRepository::create(&db, library.id, "No Books", None)
        .await
        .unwrap();

    // Add 3 books to series1
    for i in 0..3 {
        let book = create_test_book(
            series1.id,
            library.id,
            &format!("/lib/many/book{}.cbz", i),
            &format!("book{}.cbz", i),
            None,
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    // Add 1 book to series2
    let book = create_test_book(
        series2.id,
        library.id,
        "/lib/few/book0.cbz",
        "book0.cbz",
        None,
    );
    BookRepository::create(&db, &book, None).await.unwrap();

    // series3 has 0 books

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Sort by book_count descending (most first)
    let request = get_request_with_auth(
        &format!(
            "/api/v1/libraries/{}/series?sort=book_count,desc",
            library.id
        ),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 3);
    assert_eq!(series_list.data[0].title, "Many Books"); // 3 books
    assert_eq!(series_list.data[0].book_count, 3);
    assert_eq!(series_list.data[1].title, "Few Books"); // 1 book
    assert_eq!(series_list.data[1].book_count, 1);
    assert_eq!(series_list.data[2].title, "No Books"); // 0 books
    assert_eq!(series_list.data[2].book_count, 0);
}

#[tokio::test]
async fn test_list_library_series_sort_with_pagination() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create 5 series alphabetically
    for name in ["Alpha", "Beta", "Charlie", "Delta", "Echo"] {
        SeriesRepository::create(&db, library.id, name, None)
            .await
            .unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Get first page (2 items) sorted by name ascending (1-indexed)
    let request = get_request_with_auth(
        &format!(
            "/api/v1/libraries/{}/series?sort=name,asc&page=1&pageSize=2",
            library.id
        ),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let page1 = response.unwrap();
    assert_eq!(page1.data.len(), 2);
    assert_eq!(page1.total, 5);
    assert_eq!(page1.data[0].title, "Alpha");
    assert_eq!(page1.data[1].title, "Beta");

    // Get second page
    let request = get_request_with_auth(
        &format!(
            "/api/v1/libraries/{}/series?sort=name,asc&page=2&pageSize=2",
            library.id
        ),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page2 = response.unwrap();
    assert_eq!(page2.data.len(), 2);
    assert_eq!(page2.data[0].title, "Charlie");
    assert_eq!(page2.data[1].title, "Delta");
}

#[tokio::test]
async fn test_list_library_series_sort_invalid_field_uses_default() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
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
    let app = create_test_router(state).await;

    // Invalid sort field should fall back to default (name,asc)
    let request = get_request_with_auth(
        &format!(
            "/api/v1/libraries/{}/series?sort=invalid_field,asc",
            library.id
        ),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2);
    // Should be sorted by name (default)
    assert_eq!(series_list.data[0].title, "Series A");
    assert_eq!(series_list.data[1].title, "Series B");
}

// ============================================================================
// Get Series by ID Tests
// ============================================================================

#[tokio::test]
async fn test_get_series_by_id() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(&format!("/api/v1/series/{}", series.id), &token);
    let (status, response): (StatusCode, Option<SeriesDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let retrieved = response.unwrap();
    assert_eq!(retrieved.id, series.id);
    assert_eq!(retrieved.title, "Test Series");
}

#[tokio::test]
async fn test_get_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/series/{}", fake_id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert_eq!(error.error, "NotFound");
}

// ============================================================================
// Search Series Tests
// ============================================================================

#[tokio::test]
async fn test_search_series_by_name() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library.id, "Batman Comics", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Superman Comics", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Batman Graphic Novels", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let search_request = SearchSeriesRequest {
        query: "Batman".to_string(),
        library_id: None,
        full: false,
    };

    let request = post_json_request_with_auth("/api/v1/series/search", &search_request, &token);
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series = response.unwrap();
    assert_eq!(series.len(), 2);
    assert!(series.iter().all(|s| s.title.contains("Batman")));
}

#[tokio::test]
async fn test_search_series_no_results() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let search_request = SearchSeriesRequest {
        query: "NonExistent".to_string(),
        library_id: None,
        full: false,
    };

    let request = post_json_request_with_auth("/api/v1/series/search", &search_request, &token);
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series = response.unwrap();
    assert_eq!(series.len(), 0);
}

#[tokio::test]
async fn test_search_series_without_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let search_request = SearchSeriesRequest {
        query: "Test".to_string(),
        library_id: None,
        full: false,
    };

    let request = post_json_request("/api/v1/series/search", &search_request);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

// ============================================================================
// Get Series Books with Soft Delete Tests
// ============================================================================

#[tokio::test]
async fn test_get_series_books_excludes_deleted_by_default() {
    let (db, _temp_dir) = setup_test_db().await;

    // Setup library and series
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 3 books
    let book1 = create_test_book(
        series.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        Some("Book 1"),
    );
    let book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        Some("Book 2"),
    );
    let book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let book3 = create_test_book(
        series.id,
        library.id,
        "/book3.cbz",
        "book3.cbz",
        Some("Book 3"),
    );
    let book3 = BookRepository::create(&db, &book3, None).await.unwrap();

    // Mark book2 as deleted
    BookRepository::mark_deleted(&db, book2.id, true, None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Get series books without include_deleted parameter (should exclude deleted)
    let request = get_request_with_auth(&format!("/api/v1/series/{}/books", series.id), &token);
    let (status, response): (StatusCode, Option<Vec<BookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books = response.unwrap();

    // Should only return 2 books (book1 and book3)
    assert_eq!(books.len(), 2);

    let book_ids: Vec<uuid::Uuid> = books.iter().map(|b| b.id).collect();
    assert!(book_ids.contains(&book1.id));
    assert!(book_ids.contains(&book3.id));
    assert!(!book_ids.contains(&book2.id)); // Deleted book should not be included
}

#[tokio::test]
async fn test_get_series_books_includes_deleted_when_requested() {
    let (db, _temp_dir) = setup_test_db().await;

    // Setup library and series
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 3 books
    let book1 = create_test_book(
        series.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        Some("Book 1"),
    );
    let book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        Some("Book 2"),
    );
    let book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let book3 = create_test_book(
        series.id,
        library.id,
        "/book3.cbz",
        "book3.cbz",
        Some("Book 3"),
    );
    let book3 = BookRepository::create(&db, &book3, None).await.unwrap();

    // Mark book2 as deleted
    BookRepository::mark_deleted(&db, book2.id, true, None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Get series books with includeDeleted=true
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/books?includeDeleted=true", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<Vec<BookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books = response.unwrap();

    // Should return all 3 books including the deleted one
    assert_eq!(books.len(), 3);

    let book_ids: Vec<uuid::Uuid> = books.iter().map(|b| b.id).collect();
    assert!(book_ids.contains(&book1.id));
    assert!(book_ids.contains(&book2.id)); // Deleted book should be included
    assert!(book_ids.contains(&book3.id));
}

#[tokio::test]
async fn test_get_series_books_with_all_deleted() {
    let (db, _temp_dir) = setup_test_db().await;

    // Setup library and series
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 2 books and mark both as deleted
    let book1 = create_test_book(
        series.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        Some("Book 1"),
    );
    let book1 = BookRepository::create(&db, &book1, None).await.unwrap();
    BookRepository::mark_deleted(&db, book1.id, true, None)
        .await
        .unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        Some("Book 2"),
    );
    let book2 = BookRepository::create(&db, &book2, None).await.unwrap();
    BookRepository::mark_deleted(&db, book2.id, true, None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Get series books without include_deleted (should return empty)
    let request = get_request_with_auth(&format!("/api/v1/series/{}/books", series.id), &token);
    let (status, response): (StatusCode, Option<Vec<BookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books = response.unwrap();
    assert_eq!(books.len(), 0); // No active books

    // Get series books with includeDeleted=true (should return both)
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/books?includeDeleted=true", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<Vec<BookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books = response.unwrap();
    assert_eq!(books.len(), 2); // Both deleted books returned
}

#[tokio::test]
async fn test_get_series_books_include_deleted_false_explicit() {
    let (db, _temp_dir) = setup_test_db().await;

    // Setup library and series
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 2 books, mark one as deleted
    let book1 = create_test_book(
        series.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        Some("Book 1"),
    );
    let book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        Some("Book 2"),
    );
    let book2 = BookRepository::create(&db, &book2, None).await.unwrap();
    BookRepository::mark_deleted(&db, book2.id, true, None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Explicitly set includeDeleted=false
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/books?includeDeleted=false", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<Vec<BookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books = response.unwrap();

    // Should only return 1 active book
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, book1.id);
}

// ============================================================================
// Library Series Tests (with Pagination)
// ============================================================================

#[tokio::test]
async fn test_list_library_series() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create two libraries with series
    let library1 = LibraryRepository::create(&db, "Library 1", "/lib1", ScanningStrategy::Default)
        .await
        .unwrap();
    let library2 = LibraryRepository::create(&db, "Library 2", "/lib2", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series in each library
    for i in 1..=3 {
        SeriesRepository::create(&db, library1.id, &format!("Lib1 Series {}", i), None)
            .await
            .unwrap();
    }
    for i in 1..=2 {
        SeriesRepository::create(&db, library2.id, &format!("Lib2 Series {}", i), None)
            .await
            .unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request series from library 1
    let request =
        get_request_with_auth(&format!("/api/v1/libraries/{}/series", library1.id), &token);
    let (status, response): (
        StatusCode,
        Option<codex::api::routes::v1::dto::series::SeriesListResponse>,
    ) = make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 3);
    assert_eq!(series_list.total, 3);
    assert!(series_list.data.iter().all(|s| s.title.starts_with("Lib1")));

    // Request series from library 2
    let request =
        get_request_with_auth(&format!("/api/v1/libraries/{}/series", library2.id), &token);
    let (status, response): (
        StatusCode,
        Option<codex::api::routes::v1::dto::series::SeriesListResponse>,
    ) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2);
    assert_eq!(series_list.total, 2);
    assert!(series_list.data.iter().all(|s| s.title.starts_with("Lib2")));
}

#[tokio::test]
async fn test_list_library_series_with_pagination() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create 15 series
    for i in 1..=15 {
        SeriesRepository::create(&db, library.id, &format!("Series {:02}", i), None)
            .await
            .unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // Request first page (pageSize=10, page=1, 1-indexed)
    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/series?page=1&pageSize=10", library.id),
        &token,
    );
    let (status, response): (
        StatusCode,
        Option<codex::api::routes::v1::dto::series::SeriesListResponse>,
    ) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page1 = response.unwrap();
    assert_eq!(page1.data.len(), 10);
    assert_eq!(page1.total, 15);
    assert_eq!(page1.page, 1);

    // Request second page (page=2)
    let app2 = create_test_router(state).await;
    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/series?page=2&pageSize=10", library.id),
        &token,
    );
    let (status, response): (
        StatusCode,
        Option<codex::api::routes::v1::dto::series::SeriesListResponse>,
    ) = make_json_request(app2, request).await;

    assert_eq!(status, StatusCode::OK);
    let page2 = response.unwrap();
    assert_eq!(page2.data.len(), 5);
    assert_eq!(page2.total, 15);
    assert_eq!(page2.page, 2);

    // Verify different series on each page
    assert_ne!(page1.data[0].id, page2.data[0].id);
}

#[tokio::test]
async fn test_list_library_series_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library with no series
    let library =
        LibraryRepository::create(&db, "Empty Library", "/empty", ScanningStrategy::Default)
            .await
            .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request =
        get_request_with_auth(&format!("/api/v1/libraries/{}/series", library.id), &token);
    let (status, response): (
        StatusCode,
        Option<codex::api::routes::v1::dto::series::SeriesListResponse>,
    ) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 0);
    assert_eq!(series_list.total, 0);
}

// ============================================================================
// Started Series Tests
// ============================================================================

#[tokio::test]
async fn test_list_in_progress_series() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create multiple series
    let series1 = SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Series 3", None)
        .await
        .unwrap();

    // Create books in each series
    let book1 = create_test_book(
        series1.id,
        library.id,
        "/lib/s1/book1.cbz",
        "book1.cbz",
        Some("Book 1"),
    );
    let book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series2.id,
        library.id,
        "/lib/s2/book1.cbz",
        "book1.cbz",
        Some("Book 2"),
    );
    let book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let book3 = create_test_book(
        series3.id,
        library.id,
        "/lib/s3/book1.cbz",
        "book1.cbz",
        Some("Book 3"),
    );
    let _book3 = BookRepository::create(&db, &book3, None).await.unwrap();

    // Create admin user and get token
    let state = create_test_auth_state(db.clone()).await;
    let password_hash = password::hash_password("admin123").unwrap();
    let admin = create_test_user("admin", "admin@example.com", &password_hash, true);
    let admin_user = UserRepository::create(&db, &admin).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(
            admin_user.id,
            admin_user.username.clone(),
            admin_user.get_role(),
        )
        .unwrap();

    // Add reading progress for books in series1 and series2 (in-progress) for the admin user
    use codex::db::repositories::ReadProgressRepository;
    ReadProgressRepository::upsert(&db, admin_user.id, book1.id, 5, false)
        .await
        .unwrap();
    ReadProgressRepository::upsert(&db, admin_user.id, book2.id, 5, false)
        .await
        .unwrap();

    // Mark series3's book as completed (should not be in in-progress series)
    // Note: The endpoint filters for non-completed books only
    let app = create_test_router(state).await;

    // Request in-progress series (should return series1 and series2)
    let request = get_request_with_auth("/api/v1/series/in-progress", &token);
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.len(), 2); // Only series with in-progress books

    // Verify the series are the expected ones
    let series_ids: Vec<_> = series_list.iter().map(|s| s.id).collect();
    assert!(series_ids.contains(&series1.id));
    assert!(series_ids.contains(&series2.id));
    assert!(!series_ids.contains(&series3.id)); // No in-progress books
}

#[tokio::test]
async fn test_list_library_in_progress_series() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create two libraries
    let library1 = LibraryRepository::create(&db, "Library 1", "/lib1", ScanningStrategy::Default)
        .await
        .unwrap();
    let library2 = LibraryRepository::create(&db, "Library 2", "/lib2", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series in each library
    let series1 = SeriesRepository::create(&db, library1.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library2.id, "Series 2", None)
        .await
        .unwrap();

    // Create books
    let book1 = create_test_book(
        series1.id,
        library1.id,
        "/lib1/book1.cbz",
        "book1.cbz",
        Some("Book 1"),
    );
    let book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series2.id,
        library2.id,
        "/lib2/book1.cbz",
        "book1.cbz",
        Some("Book 2"),
    );
    let book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    // Create admin user and get token
    let state = create_test_auth_state(db.clone()).await;
    let password_hash = password::hash_password("admin123").unwrap();
    let admin = create_test_user("admin", "admin@example.com", &password_hash, true);
    let admin_user = UserRepository::create(&db, &admin).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(
            admin_user.id,
            admin_user.username.clone(),
            admin_user.get_role(),
        )
        .unwrap();

    // Add reading progress for both books for the admin user
    use codex::db::repositories::ReadProgressRepository;
    ReadProgressRepository::upsert(&db, admin_user.id, book1.id, 5, false)
        .await
        .unwrap();
    ReadProgressRepository::upsert(&db, admin_user.id, book2.id, 5, false)
        .await
        .unwrap();
    let app = create_test_router(state).await;

    // Request in-progress series from library 1
    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/series/in-progress", library1.id),
        &token,
    );
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.len(), 1);
    assert_eq!(series_list[0].id, series1.id);

    // Request in-progress series from library 2
    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/series/in-progress", library2.id),
        &token,
    );
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.len(), 1);
    assert_eq!(series_list[0].id, series2.id);
}

// Helper function for creating test books
// Note: title is now in book_metadata table, not books table
fn create_test_book(
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
    path: &str,
    name: &str,
    _title: Option<&str>, // No longer used - title is in book_metadata
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
        page_count: 10,
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
// Recently Added Series Tests
// ============================================================================

#[tokio::test]
async fn test_list_recently_added_series() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with delays to ensure different created_at timestamps
    let series1 = SeriesRepository::create(&db, library.id, "First Series", None)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    let series2 = SeriesRepository::create(&db, library.id, "Second Series", None)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    let series3 = SeriesRepository::create(&db, library.id, "Third Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/series/recently-added?limit=50", &token);
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.len(), 3);
    // Should be ordered by created_at descending (newest first)
    assert_eq!(series_list[0].id, series3.id);
    assert_eq!(series_list[1].id, series2.id);
    assert_eq!(series_list[2].id, series1.id);
}

#[tokio::test]
async fn test_list_recently_added_series_with_limit() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create 5 series
    for i in 1..=5 {
        SeriesRepository::create(&db, library.id, &format!("Series {}", i), None)
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request with limit=2
    let request = get_request_with_auth("/api/v1/series/recently-added?limit=2", &token);
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.len(), 2);
}

#[tokio::test]
async fn test_list_library_recently_added_series() {
    let (db, _temp_dir) = setup_test_db().await;

    let library1 = LibraryRepository::create(&db, "Library 1", "/lib1", ScanningStrategy::Default)
        .await
        .unwrap();
    let library2 = LibraryRepository::create(&db, "Library 2", "/lib2", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series in each library
    SeriesRepository::create(&db, library1.id, "Lib1 Series 1", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library1.id, "Lib1 Series 2", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library2.id, "Lib2 Series 1", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request recently added series from library 1
    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/series/recently-added", library1.id),
        &token,
    );
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.len(), 2);
    assert!(series_list.iter().all(|s| s.title.starts_with("Lib1")));
}

// ============================================================================
// Recently Updated Series Tests
// ============================================================================

#[tokio::test]
async fn test_list_recently_updated_series() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series
    let mut series1 = SeriesRepository::create(&db, library.id, "First Series", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Second Series", None)
        .await
        .unwrap();
    let mut series3 = SeriesRepository::create(&db, library.id, "Third Series", None)
        .await
        .unwrap();

    // Update series1 and series3 to change their updated_at (update path field which is on series model)
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    series3.path = "/updated/path3".to_string();
    SeriesRepository::update(&db, &series3, None).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    series1.path = "/updated/path1".to_string();
    SeriesRepository::update(&db, &series1, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/series/recently-updated?limit=50", &token);
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.len(), 3);
    // Should be ordered by updated_at descending (most recently updated first)
    assert_eq!(series_list[0].id, series1.id);
    assert_eq!(series_list[1].id, series3.id);
    assert_eq!(series_list[2].id, series2.id);
}

#[tokio::test]
async fn test_list_library_recently_updated_series() {
    let (db, _temp_dir) = setup_test_db().await;

    let library1 = LibraryRepository::create(&db, "Library 1", "/lib1", ScanningStrategy::Default)
        .await
        .unwrap();
    let library2 = LibraryRepository::create(&db, "Library 2", "/lib2", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create and update series in library 1
    let mut series1 = SeriesRepository::create(&db, library1.id, "Lib1 Series", None)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    series1.path = "/updated/lib1/path".to_string();
    SeriesRepository::update(&db, &series1, None).await.unwrap();

    // Create series in library 2 (not updated)
    SeriesRepository::create(&db, library2.id, "Lib2 Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request recently updated series from library 1
    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/series/recently-updated", library1.id),
        &token,
    );
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.len(), 1);
    assert_eq!(series_list[0].id, series1.id);
}

// ============================================================================
// Series Download Tests
// ============================================================================

#[tokio::test]
async fn test_download_series_success() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a library with a temp directory path
    let library_path = temp_dir.path().join("library");
    std::fs::create_dir_all(&library_path).unwrap();

    let library = LibraryRepository::create(
        &db,
        "Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create actual CBZ files on disk
    let book1_path = library_path.join("book1.cbz");
    let book2_path = library_path.join("book2.cbz");
    create_test_cbz(&book1_path, 3);
    create_test_cbz(&book2_path, 5);

    // Create books in database with actual file paths
    let book1 = create_test_book(
        series.id,
        library.id,
        book1_path.to_str().unwrap(),
        "book1.cbz",
        Some("Book 1"),
    );
    let _book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        book2_path.to_str().unwrap(),
        "book2.cbz",
        Some("Book 2"),
    );
    let _book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(&format!("/api/v1/series/{}/download", series.id), &token);
    let (status, body) = make_raw_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Verify we got a valid zip file
    let zip_data = body;
    assert!(!zip_data.is_empty());

    // Verify we can read the zip and it contains the expected files
    use std::io::Cursor;
    let reader = Cursor::new(&zip_data);
    let mut archive = zip::ZipArchive::new(reader).unwrap();
    assert_eq!(archive.len(), 2);

    // Verify the files are present
    let file_names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(file_names.contains(&"book1.cbz".to_string()));
    assert!(file_names.contains(&"book2.cbz".to_string()));
}

#[tokio::test]
async fn test_download_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/series/{}/download", fake_id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert_eq!(error.error, "NotFound");
}

#[tokio::test]
async fn test_download_series_no_books() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Empty Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(&format!("/api/v1/series/{}/download", series.id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert!(error.message.contains("no books"));
}

#[tokio::test]
async fn test_download_series_excludes_deleted_books() {
    let (db, temp_dir) = setup_test_db().await;

    let library_path = temp_dir.path().join("library");
    std::fs::create_dir_all(&library_path).unwrap();

    let library = LibraryRepository::create(
        &db,
        "Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create actual CBZ files
    let book1_path = library_path.join("book1.cbz");
    let book2_path = library_path.join("book2.cbz");
    create_test_cbz(&book1_path, 3);
    create_test_cbz(&book2_path, 3);

    // Create books in database
    let book1 = create_test_book(
        series.id,
        library.id,
        book1_path.to_str().unwrap(),
        "book1.cbz",
        Some("Book 1"),
    );
    BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        book2_path.to_str().unwrap(),
        "book2.cbz",
        Some("Book 2"),
    );
    let book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    // Mark book2 as deleted
    BookRepository::mark_deleted(&db, book2.id, true, None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(&format!("/api/v1/series/{}/download", series.id), &token);
    let (status, body) = make_raw_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Verify the zip only contains the non-deleted book
    use std::io::Cursor;
    let reader = Cursor::new(&body);
    let mut archive = zip::ZipArchive::new(reader).unwrap();
    assert_eq!(archive.len(), 1);
    assert_eq!(archive.by_index(0).unwrap().name(), "book1.cbz");
}

#[tokio::test]
async fn test_download_series_handles_duplicate_filenames() {
    let (db, temp_dir) = setup_test_db().await;

    let library_path = temp_dir.path().join("library");
    let subdir1 = library_path.join("vol1");
    let subdir2 = library_path.join("vol2");
    std::fs::create_dir_all(&subdir1).unwrap();
    std::fs::create_dir_all(&subdir2).unwrap();

    let library = LibraryRepository::create(
        &db,
        "Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create CBZ files with the same filename in different directories
    let book1_path = subdir1.join("chapter.cbz");
    let book2_path = subdir2.join("chapter.cbz");
    create_test_cbz(&book1_path, 3);
    create_test_cbz(&book2_path, 3);

    // Create books with duplicate filenames
    let book1 = create_test_book(
        series.id,
        library.id,
        book1_path.to_str().unwrap(),
        "chapter.cbz",
        Some("Chapter Vol 1"),
    );
    BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        book2_path.to_str().unwrap(),
        "chapter.cbz",
        Some("Chapter Vol 2"),
    );
    BookRepository::create(&db, &book2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(&format!("/api/v1/series/{}/download", series.id), &token);
    let (status, body) = make_raw_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Verify both files are present with unique names
    use std::io::Cursor;
    let reader = Cursor::new(&body);
    let mut archive = zip::ZipArchive::new(reader).unwrap();
    assert_eq!(archive.len(), 2);

    let file_names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();

    // One should be "chapter.cbz" and the other "chapter (1).cbz"
    assert!(file_names.contains(&"chapter.cbz".to_string()));
    assert!(file_names.contains(&"chapter (1).cbz".to_string()));
}

#[tokio::test]
async fn test_download_series_without_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request(&format!("/api/v1/series/{}/download", fake_id));
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_download_series_skips_missing_files() {
    let (db, temp_dir) = setup_test_db().await;

    let library_path = temp_dir.path().join("library");
    std::fs::create_dir_all(&library_path).unwrap();

    let library = LibraryRepository::create(
        &db,
        "Library",
        library_path.to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create only one CBZ file
    let book1_path = library_path.join("book1.cbz");
    create_test_cbz(&book1_path, 3);

    // Path for a book that won't exist on disk
    let book2_path = library_path.join("book2.cbz");

    // Create both books in database (but only one exists on disk)
    let book1 = create_test_book(
        series.id,
        library.id,
        book1_path.to_str().unwrap(),
        "book1.cbz",
        Some("Book 1"),
    );
    BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(
        series.id,
        library.id,
        book2_path.to_str().unwrap(),
        "book2.cbz",
        Some("Book 2"),
    );
    BookRepository::create(&db, &book2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(&format!("/api/v1/series/{}/download", series.id), &token);
    let (status, body) = make_raw_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Verify only the existing file is in the zip
    use std::io::Cursor;
    let reader = Cursor::new(&body);
    let mut archive = zip::ZipArchive::new(reader).unwrap();
    assert_eq!(archive.len(), 1);
    assert_eq!(archive.by_index(0).unwrap().name(), "book1.cbz");
}

/// Helper function to create a simple CBZ file for testing
fn create_test_cbz(path: &std::path::Path, num_pages: usize) {
    use std::fs::File;
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    let file = File::create(path).unwrap();
    let mut zip = ZipWriter::new(file);

    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // Add pages
    for i in 1..=num_pages {
        let page_data = create_simple_png();
        let filename = format!("page{:03}.png", i);
        zip.start_file(&filename, options).unwrap();
        zip.write_all(&page_data).unwrap();
    }

    zip.finish().unwrap();
}

/// Create a minimal valid PNG (1x1 pixel)
fn create_simple_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, // IHDR chunk length
        0x49, 0x48, 0x44, 0x52, // "IHDR"
        0x00, 0x00, 0x00, 0x01, // width: 1
        0x00, 0x00, 0x00, 0x01, // height: 1
        0x08, 0x02, 0x00, 0x00, 0x00, // bit depth, color type, compression, filter, interlace
        0x90, 0x77, 0x53, 0xDE, // CRC
        0x00, 0x00, 0x00, 0x0C, // IDAT chunk length
        0x49, 0x44, 0x41, 0x54, // "IDAT"
        0x08, 0x99, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x03, 0x01, 0x01,
        0x00, // compressed data
        0x18, 0xDD, 0x8D, 0xB4, // CRC
        0x00, 0x00, 0x00, 0x00, // IEND chunk length
        0x49, 0x45, 0x4E, 0x44, // "IEND"
        0xAE, 0x42, 0x60, 0x82, // CRC
    ]
}

// ============================================================================
// Series Metadata PUT (Replace) Tests
// ============================================================================

#[tokio::test]
async fn test_replace_series_metadata_success() {
    use codex::api::routes::v1::dto::series::{
        ReplaceSeriesMetadataRequest, SeriesMetadataResponse,
    };

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request_body = ReplaceSeriesMetadataRequest {
        title: Some("Updated Title".to_string()),
        title_sort: Some("Test Sort Name".to_string()),
        summary: Some("A great series".to_string()),
        publisher: Some("DC Comics".to_string()),
        imprint: None,
        status: None,
        age_rating: None,
        language: None,
        year: Some(2020),
        reading_direction: Some("ltr".to_string()),
        total_book_count: None,
        custom_metadata: Some(serde_json::json!({"tag": "value"})),
    };

    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata", series.id),
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metadata = response.unwrap();
    assert_eq!(metadata.id, series.id);
    assert_eq!(metadata.title, "Updated Title".to_string());
    assert_eq!(metadata.title_sort, Some("Test Sort Name".to_string()));
    assert_eq!(metadata.summary, Some("A great series".to_string()));
    assert_eq!(metadata.publisher, Some("DC Comics".to_string()));
    assert_eq!(metadata.year, Some(2020));
    assert_eq!(metadata.reading_direction, Some("ltr".to_string()));
    assert_eq!(
        metadata.custom_metadata,
        Some(serde_json::json!({"tag": "value"}))
    );
}

#[tokio::test]
async fn test_replace_series_metadata_clears_omitted_fields() {
    use codex::api::routes::v1::dto::series::{
        ReplaceSeriesMetadataRequest, SeriesMetadataResponse,
    };

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with initial metadata
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_summary(&db, series.id, Some("Initial summary".to_string()))
        .await
        .unwrap();
    SeriesMetadataRepository::update_publisher(
        &db,
        series.id,
        Some("Initial publisher".to_string()),
        None,
    )
    .await
    .unwrap();
    SeriesMetadataRepository::update_year(&db, series.id, Some(2000))
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // PUT with only some fields - omitted fields should be cleared
    let request_body = ReplaceSeriesMetadataRequest {
        title: None, // Keep existing title
        title_sort: None,
        summary: Some("New summary".to_string()),
        publisher: None, // Should clear publisher
        imprint: None,
        status: None,
        age_rating: None,
        language: None,
        year: None, // Should clear year
        reading_direction: None,
        total_book_count: None,
        custom_metadata: None,
    };

    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata", series.id),
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metadata = response.unwrap();
    assert_eq!(metadata.summary, Some("New summary".to_string()));
    assert_eq!(metadata.publisher, None); // Cleared
    assert_eq!(metadata.year, None); // Cleared
}

#[tokio::test]
async fn test_replace_series_metadata_not_found() {
    use codex::api::routes::v1::dto::series::ReplaceSeriesMetadataRequest;

    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request_body = ReplaceSeriesMetadataRequest {
        title: None,
        title_sort: None,
        summary: Some("Summary".to_string()),
        publisher: None,
        imprint: None,
        status: None,
        age_rating: None,
        language: None,
        year: None,
        reading_direction: None,
        total_book_count: None,
        custom_metadata: None,
    };

    let request = put_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata", fake_id),
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert_eq!(error.error, "NotFound");
}

#[tokio::test]
async fn test_replace_series_metadata_without_auth() {
    use codex::api::routes::v1::dto::series::ReplaceSeriesMetadataRequest;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let request_body = ReplaceSeriesMetadataRequest {
        title: None,
        title_sort: None,
        summary: Some("Summary".to_string()),
        publisher: None,
        imprint: None,
        status: None,
        age_rating: None,
        language: None,
        year: None,
        reading_direction: None,
        total_book_count: None,
        custom_metadata: None,
    };

    let request = put_json_request(
        &format!("/api/v1/series/{}/metadata", series.id),
        &request_body,
    );
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

// ============================================================================
// Series Metadata PATCH (Partial Update) Tests
// ============================================================================

#[tokio::test]
async fn test_patch_series_metadata_partial_update() {
    use codex::api::routes::v1::dto::series::SeriesMetadataResponse;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with initial metadata
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_summary(&db, series.id, Some("Original summary".to_string()))
        .await
        .unwrap();
    SeriesMetadataRepository::update_publisher(
        &db,
        series.id,
        Some("Original publisher".to_string()),
        None,
    )
    .await
    .unwrap();
    SeriesMetadataRepository::update_year(&db, series.id, Some(2000))
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // PATCH only updates summary - other fields should be unchanged
    let request = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata", series.id),
        &serde_json::json!({
            "summary": "Updated summary"
        }),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metadata = response.unwrap();
    assert_eq!(metadata.summary, Some("Updated summary".to_string()));
    assert_eq!(metadata.publisher, Some("Original publisher".to_string())); // Unchanged
    assert_eq!(metadata.year, Some(2000)); // Unchanged
}

#[tokio::test]
async fn test_patch_series_metadata_explicit_null_clears_field() {
    use codex::api::routes::v1::dto::series::SeriesMetadataResponse;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with initial metadata
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_summary(&db, series.id, Some("Original summary".to_string()))
        .await
        .unwrap();
    SeriesMetadataRepository::update_publisher(
        &db,
        series.id,
        Some("Original publisher".to_string()),
        None,
    )
    .await
    .unwrap();
    SeriesMetadataRepository::update_year(&db, series.id, Some(2000))
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // PATCH with explicit null should clear the field, but omitted fields stay unchanged
    let request = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata", series.id),
        &serde_json::json!({
            "publisher": null
        }),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metadata = response.unwrap();
    assert_eq!(metadata.summary, Some("Original summary".to_string())); // Unchanged
    assert_eq!(metadata.publisher, None); // Cleared by explicit null
    assert_eq!(metadata.year, Some(2000)); // Unchanged
}

#[tokio::test]
async fn test_patch_series_metadata_multiple_fields() {
    use codex::api::routes::v1::dto::series::SeriesMetadataResponse;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // PATCH multiple fields at once
    let request = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata", series.id),
        &serde_json::json!({
            "titleSort": "Sort Name",
            "summary": "A great summary",
            "publisher": "Marvel",
            "year": 2024,
            "readingDirection": "rtl"
        }),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metadata = response.unwrap();
    assert_eq!(metadata.title_sort, Some("Sort Name".to_string()));
    assert_eq!(metadata.summary, Some("A great summary".to_string()));
    assert_eq!(metadata.publisher, Some("Marvel".to_string()));
    assert_eq!(metadata.year, Some(2024));
    assert_eq!(metadata.reading_direction, Some("rtl".to_string()));
}

#[tokio::test]
async fn test_patch_series_metadata_empty_body_no_changes() {
    use codex::api::routes::v1::dto::series::SeriesMetadataResponse;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with initial metadata
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_summary(&db, series.id, Some("Original summary".to_string()))
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // PATCH with empty body - nothing should change
    let request = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata", series.id),
        &serde_json::json!({}),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metadata = response.unwrap();
    assert_eq!(metadata.summary, Some("Original summary".to_string())); // Unchanged
}

#[tokio::test]
async fn test_patch_series_metadata_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = patch_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata", fake_id),
        &serde_json::json!({"summary": "Test"}),
        &token,
    );
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert_eq!(error.error, "NotFound");
}

#[tokio::test]
async fn test_patch_series_metadata_without_auth() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let request = patch_json_request(
        &format!("/api/v1/series/{}/metadata", series.id),
        &serde_json::json!({"summary": "Test"}),
    );
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

// ============================================================================
// Series Filtering by Genres/Tags Tests
// ============================================================================

#[tokio::test]
async fn test_list_series_filter_by_single_genre() {
    use codex::db::repositories::GenreRepository;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with different genres
    let series1 = SeriesRepository::create(&db, library.id, "Action Series", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Comedy Series", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Action Comedy Series", None)
        .await
        .unwrap();

    // Assign genres
    GenreRepository::set_genres_for_series(&db, series1.id, vec!["Action".to_string()])
        .await
        .unwrap();
    GenreRepository::set_genres_for_series(&db, series2.id, vec!["Comedy".to_string()])
        .await
        .unwrap();
    GenreRepository::set_genres_for_series(
        &db,
        series3.id,
        vec!["Action".to_string(), "Comedy".to_string()],
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by Action genre
    let request = get_request_with_auth("/api/v1/series?genres=Action", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2);
    assert_eq!(series_list.total, 2);

    let series_ids: Vec<_> = series_list.data.iter().map(|s| s.id).collect();
    assert!(series_ids.contains(&series1.id));
    assert!(series_ids.contains(&series3.id));
    assert!(!series_ids.contains(&series2.id));
}

#[tokio::test]
async fn test_list_series_filter_by_multiple_genres_and_logic() {
    use codex::db::repositories::GenreRepository;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with different genre combinations
    let series1 = SeriesRepository::create(&db, library.id, "Action Only", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Comedy Only", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Action and Comedy", None)
        .await
        .unwrap();
    let series4 = SeriesRepository::create(&db, library.id, "Action and Drama", None)
        .await
        .unwrap();

    // Assign genres
    GenreRepository::set_genres_for_series(&db, series1.id, vec!["Action".to_string()])
        .await
        .unwrap();
    GenreRepository::set_genres_for_series(&db, series2.id, vec!["Comedy".to_string()])
        .await
        .unwrap();
    GenreRepository::set_genres_for_series(
        &db,
        series3.id,
        vec!["Action".to_string(), "Comedy".to_string()],
    )
    .await
    .unwrap();
    GenreRepository::set_genres_for_series(
        &db,
        series4.id,
        vec!["Action".to_string(), "Drama".to_string()],
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by Action AND Comedy (series must have BOTH)
    let request = get_request_with_auth("/api/v1/series?genres=Action,Comedy", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].id, series3.id);
}

#[tokio::test]
async fn test_list_series_filter_by_single_tag() {
    use codex::db::repositories::TagRepository;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series
    let series1 = SeriesRepository::create(&db, library.id, "Completed Series", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Ongoing Series", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Favorite Completed", None)
        .await
        .unwrap();

    // Assign tags
    TagRepository::set_tags_for_series(&db, series1.id, vec!["Completed".to_string()])
        .await
        .unwrap();
    TagRepository::set_tags_for_series(&db, series2.id, vec!["Ongoing".to_string()])
        .await
        .unwrap();
    TagRepository::set_tags_for_series(
        &db,
        series3.id,
        vec!["Completed".to_string(), "Favorite".to_string()],
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by Completed tag
    let request = get_request_with_auth("/api/v1/series?tags=Completed", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2);
    assert_eq!(series_list.total, 2);

    let series_ids: Vec<_> = series_list.data.iter().map(|s| s.id).collect();
    assert!(series_ids.contains(&series1.id));
    assert!(series_ids.contains(&series3.id));
    assert!(!series_ids.contains(&series2.id));
}

#[tokio::test]
async fn test_list_series_filter_by_multiple_tags_and_logic() {
    use codex::db::repositories::TagRepository;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Just Completed", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Just Favorite", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Completed and Favorite", None)
        .await
        .unwrap();

    TagRepository::set_tags_for_series(&db, series1.id, vec!["Completed".to_string()])
        .await
        .unwrap();
    TagRepository::set_tags_for_series(&db, series2.id, vec!["Favorite".to_string()])
        .await
        .unwrap();
    TagRepository::set_tags_for_series(
        &db,
        series3.id,
        vec!["Completed".to_string(), "Favorite".to_string()],
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by Completed AND Favorite (series must have BOTH)
    let request = get_request_with_auth("/api/v1/series?tags=Completed,Favorite", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].id, series3.id);
}

#[tokio::test]
async fn test_list_series_filter_by_genre_and_tag_combined() {
    use codex::db::repositories::{GenreRepository, TagRepository};

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Action Completed", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Action Ongoing", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Comedy Completed", None)
        .await
        .unwrap();

    // Assign genres and tags
    GenreRepository::set_genres_for_series(&db, series1.id, vec!["Action".to_string()])
        .await
        .unwrap();
    TagRepository::set_tags_for_series(&db, series1.id, vec!["Completed".to_string()])
        .await
        .unwrap();

    GenreRepository::set_genres_for_series(&db, series2.id, vec!["Action".to_string()])
        .await
        .unwrap();
    TagRepository::set_tags_for_series(&db, series2.id, vec!["Ongoing".to_string()])
        .await
        .unwrap();

    GenreRepository::set_genres_for_series(&db, series3.id, vec!["Comedy".to_string()])
        .await
        .unwrap();
    TagRepository::set_tags_for_series(&db, series3.id, vec!["Completed".to_string()])
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by Action genre AND Completed tag
    let request = get_request_with_auth("/api/v1/series?genres=Action&tags=Completed", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].id, series1.id);
}

#[tokio::test]
async fn test_list_series_filter_by_nonexistent_genre() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by a genre that doesn't exist
    let request = get_request_with_auth("/api/v1/series?genres=NonExistent", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 0);
    assert_eq!(series_list.total, 0);
}

#[tokio::test]
async fn test_list_series_filter_with_library_id() {
    use codex::db::repositories::GenreRepository;

    let (db, _temp_dir) = setup_test_db().await;

    let library1 = LibraryRepository::create(&db, "Library 1", "/lib1", ScanningStrategy::Default)
        .await
        .unwrap();
    let library2 = LibraryRepository::create(&db, "Library 2", "/lib2", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library1.id, "Lib1 Action", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library2.id, "Lib2 Action", None)
        .await
        .unwrap();

    // Both series have Action genre
    GenreRepository::set_genres_for_series(&db, series1.id, vec!["Action".to_string()])
        .await
        .unwrap();
    GenreRepository::set_genres_for_series(&db, series2.id, vec!["Action".to_string()])
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by Action genre AND library 1
    let request = get_request_with_auth(
        &format!("/api/v1/series?genres=Action&libraryId={}", library1.id),
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].id, series1.id);
}

#[tokio::test]
async fn test_list_series_filter_with_pagination() {
    use codex::db::repositories::GenreRepository;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create 5 series with Action genre
    for i in 1..=5 {
        let series = SeriesRepository::create(&db, library.id, &format!("Action {}", i), None)
            .await
            .unwrap();
        GenreRepository::set_genres_for_series(&db, series.id, vec!["Action".to_string()])
            .await
            .unwrap();
    }

    // Create 2 series without Action genre
    for i in 1..=2 {
        SeriesRepository::create(&db, library.id, &format!("Comedy {}", i), None)
            .await
            .unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by Action with pagination (1-indexed)
    let request = get_request_with_auth("/api/v1/series?genres=Action&page=1&pageSize=2", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let page1 = response.unwrap();
    assert_eq!(page1.data.len(), 2);
    assert_eq!(page1.total, 5);
    assert_eq!(page1.page, 1);

    // Get second page
    let request = get_request_with_auth("/api/v1/series?genres=Action&page=2&pageSize=2", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page2 = response.unwrap();
    assert_eq!(page2.data.len(), 2);
    assert_eq!(page2.total, 5);
    assert_eq!(page2.page, 2);
}

#[tokio::test]
async fn test_list_series_filter_case_insensitive() {
    use codex::db::repositories::GenreRepository;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();
    GenreRepository::set_genres_for_series(&db, series.id, vec!["Action".to_string()])
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by "action" (lowercase) should match "Action"
    let request = get_request_with_auth("/api/v1/series?genres=action", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].id, series.id);
}

#[tokio::test]
async fn test_list_series_filter_empty_string_ignored() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series without any genres
    SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Empty genre filter should return all series (no filtering)
    let request = get_request_with_auth("/api/v1/series?genres=", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2);
}

// ============================================================================
// POST /series/list Filtering Tests
// ============================================================================

use codex::api::routes::v1::dto::filter::{
    BoolOperator, FieldOperator, SeriesCondition, SeriesListRequest, UuidOperator,
};

#[tokio::test]
async fn test_list_series_filtered_no_condition() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // POST with no condition should return all series
    let request_body = SeriesListRequest::default();
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2);
    assert_eq!(series_list.total, 2);
}

#[tokio::test]
async fn test_list_series_filtered_by_library_id() {
    let (db, _temp_dir) = setup_test_db().await;

    let library1 = LibraryRepository::create(&db, "Library 1", "/lib1", ScanningStrategy::Default)
        .await
        .unwrap();
    let library2 = LibraryRepository::create(&db, "Library 2", "/lib2", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library1.id, "Lib1 Series 1", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library1.id, "Lib1 Series 2", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library2.id, "Lib2 Series 1", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by library1 ID
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::LibraryId {
            library_id: UuidOperator::Is { value: library1.id },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2);
    assert!(series_list.data.iter().all(|s| s.title.starts_with("Lib1")));
}

#[tokio::test]
async fn test_list_series_filtered_by_genre() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::GenreRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Action Series", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Comedy Series", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Mixed Series", None)
        .await
        .unwrap();

    // Add genres
    GenreRepository::add_genre_to_series(&db, series1.id, "Action")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series2.id, "Comedy")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series3.id, "Action")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series3.id, "Comedy")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by genre = "Action"
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Genre {
            genre: FieldOperator::Is {
                value: "Action".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2); // Action Series and Mixed Series
}

#[tokio::test]
async fn test_list_series_filtered_all_of() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::GenreRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Action Only", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Comedy Only", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Action Comedy", None)
        .await
        .unwrap();

    // Add genres
    GenreRepository::add_genre_to_series(&db, series1.id, "Action")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series2.id, "Comedy")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series3.id, "Action")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series3.id, "Comedy")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // AllOf: Action AND Comedy (should only match series3)
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::AllOf {
            all_of: vec![
                SeriesCondition::Genre {
                    genre: FieldOperator::Is {
                        value: "Action".to_string(),
                    },
                },
                SeriesCondition::Genre {
                    genre: FieldOperator::Is {
                        value: "Comedy".to_string(),
                    },
                },
            ],
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].title, "Action Comedy");
}

#[tokio::test]
async fn test_list_series_filtered_any_of() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::GenreRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Action Series", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Drama Series", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Horror Series", None)
        .await
        .unwrap();

    // Add genres
    GenreRepository::add_genre_to_series(&db, series1.id, "Action")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series2.id, "Drama")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series3.id, "Horror")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // AnyOf: Action OR Drama (should match series1 and series2)
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::AnyOf {
            any_of: vec![
                SeriesCondition::Genre {
                    genre: FieldOperator::Is {
                        value: "Action".to_string(),
                    },
                },
                SeriesCondition::Genre {
                    genre: FieldOperator::Is {
                        value: "Drama".to_string(),
                    },
                },
            ],
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2);
}

#[tokio::test]
async fn test_list_series_filtered_pagination() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create 5 series
    for i in 1..=5 {
        SeriesRepository::create(&db, library.id, &format!("Series {}", i), None)
            .await
            .unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request page 1, pageSize 2 (1-indexed) - pagination is now in query params (camelCase)
    let request_body = SeriesListRequest::default();
    let request = post_json_request_with_auth(
        "/api/v1/series/list?page=1&pageSize=2",
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let page1 = response.unwrap();
    assert_eq!(page1.data.len(), 2);
    assert_eq!(page1.total, 5);
    assert_eq!(page1.page, 1);

    // Request page 2
    let request = post_json_request_with_auth(
        "/api/v1/series/list?page=2&pageSize=2",
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page2 = response.unwrap();
    assert_eq!(page2.data.len(), 2);
    assert_eq!(page2.page, 2);
}

#[tokio::test]
async fn test_list_series_filtered_genre_contains() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::GenreRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    // Add genres with "Action" substring
    GenreRepository::add_genre_to_series(&db, series1.id, "Action")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series2.id, "Live Action")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by genre containing "Action"
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Genre {
            genre: FieldOperator::Contains {
                value: "Action".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2); // Both have "Action" in genre name
}

#[tokio::test]
async fn test_list_series_filtered_by_name() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library.id, "Naruto", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "One Piece", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Naruto Shippuden", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by name "Is"
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Name {
            name: FieldOperator::Is {
                value: "Naruto".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].title, "Naruto");

    // Filter by name "Contains"
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Name {
            name: FieldOperator::Contains {
                value: "Naruto".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2); // Naruto and Naruto Shippuden

    // Filter by name "BeginsWith"
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Name {
            name: FieldOperator::BeginsWith {
                value: "One".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].title, "One Piece");
}

#[tokio::test]
async fn test_list_series_filtered_by_status() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::SeriesMetadataRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Ongoing Series", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Ended Series", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Hiatus Series", None)
        .await
        .unwrap();

    // Set statuses
    SeriesMetadataRepository::update_status(&db, series1.id, Some("ongoing".to_string()))
        .await
        .unwrap();
    SeriesMetadataRepository::update_status(&db, series2.id, Some("ended".to_string()))
        .await
        .unwrap();
    SeriesMetadataRepository::update_status(&db, series3.id, Some("hiatus".to_string()))
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by status = "ongoing"
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Status {
            status: FieldOperator::Is {
                value: "ongoing".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].title, "Ongoing Series");

    // Filter by status != "ended"
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Status {
            status: FieldOperator::IsNot {
                value: "ended".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2); // ongoing and hiatus
}

#[tokio::test]
async fn test_list_series_filtered_by_publisher() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::SeriesMetadataRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Shueisha Series", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Kodansha Series", None)
        .await
        .unwrap();
    let _series3 = SeriesRepository::create(&db, library.id, "No Publisher Series", None)
        .await
        .unwrap();

    // Set publishers
    SeriesMetadataRepository::update_publisher(&db, series1.id, Some("Shueisha".to_string()), None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_publisher(&db, series2.id, Some("Kodansha".to_string()), None)
        .await
        .unwrap();
    // series3 has no publisher (NULL)

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by publisher = "Shueisha"
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Publisher {
            publisher: FieldOperator::Is {
                value: "Shueisha".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].title, "Shueisha Series");

    // Filter by publisher IsNull (no publisher set)
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Publisher {
            publisher: FieldOperator::IsNull,
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].title, "No Publisher Series");

    // Filter by publisher IsNotNull (has publisher)
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Publisher {
            publisher: FieldOperator::IsNotNull,
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2); // Shueisha and Kodansha series
}

#[tokio::test]
async fn test_list_series_filtered_by_language() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::SeriesMetadataRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Japanese Series", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "English Series", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Korean Series", None)
        .await
        .unwrap();

    // Set languages
    SeriesMetadataRepository::update_language(&db, series1.id, Some("ja".to_string()))
        .await
        .unwrap();
    SeriesMetadataRepository::update_language(&db, series2.id, Some("en".to_string()))
        .await
        .unwrap();
    SeriesMetadataRepository::update_language(&db, series3.id, Some("ko".to_string()))
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by language = "ja"
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Language {
            language: FieldOperator::Is {
                value: "ja".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].title, "Japanese Series");

    // Filter by language != "en"
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Language {
            language: FieldOperator::IsNot {
                value: "en".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2); // ja and ko
}

#[tokio::test]
async fn test_list_series_filtered_combined_metadata() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::{GenreRepository, SeriesMetadataRepository};

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "My Hero Academia", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Attack on Titan", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "One Punch Man", None)
        .await
        .unwrap();

    // Set metadata
    SeriesMetadataRepository::update_status(&db, series1.id, Some("ongoing".to_string()))
        .await
        .unwrap();
    SeriesMetadataRepository::update_status(&db, series2.id, Some("ended".to_string()))
        .await
        .unwrap();
    SeriesMetadataRepository::update_status(&db, series3.id, Some("ongoing".to_string()))
        .await
        .unwrap();

    GenreRepository::add_genre_to_series(&db, series1.id, "Action")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series2.id, "Action")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series3.id, "Comedy")
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Combined filter: status = "ongoing" AND genre = "Action"
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::AllOf {
            all_of: vec![
                SeriesCondition::Status {
                    status: FieldOperator::Is {
                        value: "ongoing".to_string(),
                    },
                },
                SeriesCondition::Genre {
                    genre: FieldOperator::Is {
                        value: "Action".to_string(),
                    },
                },
            ],
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].title, "My Hero Academia");
}

// ============================================================================
// ReadStatus Filtering Tests
// ============================================================================

#[tokio::test]
async fn test_list_series_filtered_by_read_status_unread() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::ReadProgressRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create 3 series with books
    let series1 = SeriesRepository::create(&db, library.id, "Unread Series", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "In Progress Series", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Read Series", None)
        .await
        .unwrap();

    // Create books for each series
    let book1_model = create_test_book(
        series1.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        Some("Book 1"),
    );
    let _book1 = BookRepository::create(&db, &book1_model, None)
        .await
        .unwrap();
    let book2_model = create_test_book(
        series2.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        Some("Book 2"),
    );
    let book2 = BookRepository::create(&db, &book2_model, None)
        .await
        .unwrap();
    let book3_model = create_test_book(
        series3.id,
        library.id,
        "/book3.cbz",
        "book3.cbz",
        Some("Book 3"),
    );
    let book3 = BookRepository::create(&db, &book3_model, None)
        .await
        .unwrap();

    // Create admin user
    let state = create_test_auth_state(db.clone()).await;
    let password_hash = password::hash_password("admin123").unwrap();
    let admin = create_test_user("admin", "admin@example.com", &password_hash, true);
    let admin_user = UserRepository::create(&db, &admin).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(
            admin_user.id,
            admin_user.username.clone(),
            admin_user.get_role(),
        )
        .unwrap();

    // Set read progress:
    // - series1: No progress (unread)
    // - series2: In progress (not completed, page > 0)
    ReadProgressRepository::upsert(&db, admin_user.id, book2.id, 5, false)
        .await
        .unwrap();
    // - series3: Completed (read)
    ReadProgressRepository::upsert(&db, admin_user.id, book3.id, 10, true)
        .await
        .unwrap();

    let app = create_test_router(state).await;

    // Filter for unread series
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::ReadStatus {
            read_status: FieldOperator::Is {
                value: "unread".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].title, "Unread Series");
}

#[tokio::test]
async fn test_list_series_filtered_by_read_status_in_progress() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::ReadProgressRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Unread Series", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "In Progress Series", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Read Series", None)
        .await
        .unwrap();

    let book1_model = create_test_book(
        series1.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        Some("Book 1"),
    );
    let _book1 = BookRepository::create(&db, &book1_model, None)
        .await
        .unwrap();
    let book2_model = create_test_book(
        series2.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        Some("Book 2"),
    );
    let book2 = BookRepository::create(&db, &book2_model, None)
        .await
        .unwrap();
    let book3_model = create_test_book(
        series3.id,
        library.id,
        "/book3.cbz",
        "book3.cbz",
        Some("Book 3"),
    );
    let book3 = BookRepository::create(&db, &book3_model, None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let password_hash = password::hash_password("admin123").unwrap();
    let admin = create_test_user("admin", "admin@example.com", &password_hash, true);
    let admin_user = UserRepository::create(&db, &admin).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(
            admin_user.id,
            admin_user.username.clone(),
            admin_user.get_role(),
        )
        .unwrap();

    // series2: In progress
    ReadProgressRepository::upsert(&db, admin_user.id, book2.id, 5, false)
        .await
        .unwrap();
    // series3: Completed
    ReadProgressRepository::upsert(&db, admin_user.id, book3.id, 10, true)
        .await
        .unwrap();

    let app = create_test_router(state).await;

    // Filter for in_progress series
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::ReadStatus {
            read_status: FieldOperator::Is {
                value: "in_progress".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].title, "In Progress Series");
}

#[tokio::test]
async fn test_list_series_filtered_by_read_status_read() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::ReadProgressRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Unread Series", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "In Progress Series", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Read Series", None)
        .await
        .unwrap();

    let book1_model = create_test_book(
        series1.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        Some("Book 1"),
    );
    let _book1 = BookRepository::create(&db, &book1_model, None)
        .await
        .unwrap();
    let book2_model = create_test_book(
        series2.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        Some("Book 2"),
    );
    let book2 = BookRepository::create(&db, &book2_model, None)
        .await
        .unwrap();
    let book3_model = create_test_book(
        series3.id,
        library.id,
        "/book3.cbz",
        "book3.cbz",
        Some("Book 3"),
    );
    let book3 = BookRepository::create(&db, &book3_model, None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let password_hash = password::hash_password("admin123").unwrap();
    let admin = create_test_user("admin", "admin@example.com", &password_hash, true);
    let admin_user = UserRepository::create(&db, &admin).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(
            admin_user.id,
            admin_user.username.clone(),
            admin_user.get_role(),
        )
        .unwrap();

    // series2: In progress
    ReadProgressRepository::upsert(&db, admin_user.id, book2.id, 5, false)
        .await
        .unwrap();
    // series3: Completed
    ReadProgressRepository::upsert(&db, admin_user.id, book3.id, 10, true)
        .await
        .unwrap();

    let app = create_test_router(state).await;

    // Filter for read series
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::ReadStatus {
            read_status: FieldOperator::Is {
                value: "read".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].title, "Read Series");
}

#[tokio::test]
async fn test_list_series_filtered_by_read_status_not_read() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::ReadProgressRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Unread Series", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "In Progress Series", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Read Series", None)
        .await
        .unwrap();

    let book1_model = create_test_book(
        series1.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        Some("Book 1"),
    );
    let _book1 = BookRepository::create(&db, &book1_model, None)
        .await
        .unwrap();
    let book2_model = create_test_book(
        series2.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        Some("Book 2"),
    );
    let book2 = BookRepository::create(&db, &book2_model, None)
        .await
        .unwrap();
    let book3_model = create_test_book(
        series3.id,
        library.id,
        "/book3.cbz",
        "book3.cbz",
        Some("Book 3"),
    );
    let book3 = BookRepository::create(&db, &book3_model, None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let password_hash = password::hash_password("admin123").unwrap();
    let admin = create_test_user("admin", "admin@example.com", &password_hash, true);
    let admin_user = UserRepository::create(&db, &admin).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(
            admin_user.id,
            admin_user.username.clone(),
            admin_user.get_role(),
        )
        .unwrap();

    // series2: In progress
    ReadProgressRepository::upsert(&db, admin_user.id, book2.id, 5, false)
        .await
        .unwrap();
    // series3: Completed
    ReadProgressRepository::upsert(&db, admin_user.id, book3.id, 10, true)
        .await
        .unwrap();

    let app = create_test_router(state).await;

    // Filter for NOT read series (should include unread and in_progress)
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::ReadStatus {
            read_status: FieldOperator::IsNot {
                value: "read".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2);
    let names: Vec<_> = series_list.data.iter().map(|s| s.title.as_str()).collect();
    assert!(names.contains(&"Unread Series"));
    assert!(names.contains(&"In Progress Series"));
}

// ============================================================================
// Sorting Tests
// ============================================================================

#[tokio::test]
async fn test_list_series_sort_by_name_uses_title_sort() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with titles that would sort differently with title_sort
    // Title: "The Batman" with title_sort: "Batman, The" -> should sort as "B"
    // Title: "Avengers" with title_sort: None -> should sort as "A"
    // Title: "A Spider-Man" with title_sort: "Spider-Man" -> should sort as "S"

    // SeriesRepository::create already creates series_metadata, so we just update title_sort
    let series1 = SeriesRepository::create(&db, library.id, "The Batman", None)
        .await
        .unwrap();
    // Set title_sort to "Batman, The" so it sorts under B
    SeriesMetadataRepository::update_title(
        &db,
        series1.id,
        "The Batman".to_string(),
        Some("Batman, The".to_string()),
    )
    .await
    .unwrap();

    let _series2 = SeriesRepository::create(&db, library.id, "Avengers", None)
        .await
        .unwrap();
    // No title_sort update needed, should sort by title "Avengers" under A

    let series3 = SeriesRepository::create(&db, library.id, "A Spider-Man", None)
        .await
        .unwrap();
    // Set title_sort to "Spider-Man" so it sorts under S instead of A
    SeriesMetadataRepository::update_title(
        &db,
        series3.id,
        "A Spider-Man".to_string(),
        Some("Spider-Man".to_string()),
    )
    .await
    .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Sort by name ascending - should use title_sort
    // Expected order: Avengers (A), Batman, The (B), Spider-Man (S)
    let request = get_request_with_auth("/api/v1/series?sort=name,asc", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 3);

    // Verify sort order uses title_sort
    let titles: Vec<&str> = series_list.data.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["Avengers", "The Batman", "A Spider-Man"],
        "Sort should use title_sort field: Avengers (A) < Batman, The (B) < Spider-Man (S)"
    );
}

#[tokio::test]
async fn test_list_series_sort_by_name_descending() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // SeriesRepository::create already creates series_metadata
    let _series1 = SeriesRepository::create(&db, library.id, "Alpha", None)
        .await
        .unwrap();

    let _series2 = SeriesRepository::create(&db, library.id, "Beta", None)
        .await
        .unwrap();

    let _series3 = SeriesRepository::create(&db, library.id, "Gamma", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Sort by name descending
    let request = get_request_with_auth("/api/v1/series?sort=name,desc", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    let titles: Vec<&str> = series_list.data.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, vec!["Gamma", "Beta", "Alpha"]);
}

#[tokio::test]
async fn test_list_series_sort_by_date_added() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series in specific order - SeriesRepository::create already creates metadata
    let _series1 = SeriesRepository::create(&db, library.id, "First", None)
        .await
        .unwrap();

    // Small delay to ensure different timestamps
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let _series2 = SeriesRepository::create(&db, library.id, "Second", None)
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let _series3 = SeriesRepository::create(&db, library.id, "Third", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // Sort by date added ascending (oldest first)
    let request = get_request_with_auth("/api/v1/series?sort=created_at,asc", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    let titles: Vec<&str> = series_list.data.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, vec!["First", "Second", "Third"]);

    // Sort by date added descending (newest first)
    let app2 = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/series?sort=created_at,desc", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app2, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    let titles: Vec<&str> = series_list.data.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, vec!["Third", "Second", "First"]);
}

#[tokio::test]
async fn test_list_series_sort_with_pagination() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create 5 series: Alpha, Beta, Gamma, Delta, Epsilon
    // SeriesRepository::create already creates metadata
    for name in &["Alpha", "Beta", "Gamma", "Delta", "Epsilon"] {
        let _series = SeriesRepository::create(&db, library.id, name, None)
            .await
            .unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // Get page 1 with pageSize=2, sorted by name ascending (1-indexed)
    // Should get Alpha, Beta
    let request = get_request_with_auth("/api/v1/series?sort=name,asc&page=1&pageSize=2", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2);
    assert_eq!(series_list.total, 5);
    assert_eq!(series_list.page, 1);
    let titles: Vec<&str> = series_list.data.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, vec!["Alpha", "Beta"]);

    // Get page 2 with pageSize=2
    // Should get Delta, Epsilon (D and E after A, B)
    let app2 = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/series?sort=name,asc&page=2&pageSize=2", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app2, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2);
    let titles: Vec<&str> = series_list.data.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, vec!["Delta", "Epsilon"]);

    // Get page 3 with pageSize=2
    // Should get Gamma (only 1 remaining)
    let app3 = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/series?sort=name,asc&page=3&pageSize=2", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app3, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    let titles: Vec<&str> = series_list.data.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, vec!["Gamma"]);
}

// ============================================================================
// NULL title_sort handling tests
// ============================================================================

#[tokio::test]
async fn test_list_series_sort_by_name_with_null_title_sort() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with names that would be sorted differently than insertion order.
    // All have title_sort = NULL (the default), which is the common production case.
    // Deliberately inserting out of alphabetical order to catch insertion-order bugs.
    SeriesRepository::create(&db, library.id, "Kaiju No. 8", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "A Couple of Cuckoos", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Fairy Tail's Fairy Girls", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "+Anima", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Shinobi Life", None)
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

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Sort by name ascending - with all NULL title_sort values,
    // this should fall back to sorting by title alphabetically
    let request = get_request_with_auth("/api/v1/series?sort=name,asc", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 8);

    let titles: Vec<&str> = series_list.data.iter().map(|s| s.title.as_str()).collect();
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
        "Series should be sorted alphabetically by title when title_sort is NULL"
    );
}

#[tokio::test]
async fn test_list_series_sort_by_name_mixed_null_and_set_title_sort() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
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
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/series?sort=name,asc", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 4);

    // Expected order:
    // "The Amazing Spider-Man" (title_sort="Amazing Spider-Man, The" -> sorts under A)
    // "Batman" (title_sort=NULL, falls back to title "Batman")
    // "Cable" (title_sort=NULL, falls back to title "Cable")
    // "Daredevil" (title_sort=NULL, falls back to title "Daredevil")
    let titles: Vec<&str> = series_list.data.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["The Amazing Spider-Man", "Batman", "Cable", "Daredevil"],
        "Series with title_sort should be interleaved with NULL title_sort series"
    );
}

#[tokio::test]
async fn test_list_series_filtered_sort_by_name_with_null_title_sort() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series out of alphabetical order, all with NULL title_sort
    SeriesRepository::create(&db, library.id, "Kaiju No. 8", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "A Couple of Cuckoos", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Fairy Tail's Fairy Girls", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Shinobi Life", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Use POST /series/list endpoint (exercises list_by_ids_sorted code path)
    let request = post_request_with_auth_json(
        "/api/v1/series/list?sort=name,asc",
        &token,
        r#"{"condition":null,"fullTextSearch":null}"#,
    );
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 4);

    let titles: Vec<&str> = series_list.data.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        titles,
        vec![
            "A Couple of Cuckoos",
            "Fairy Tail's Fairy Girls",
            "Kaiju No. 8",
            "Shinobi Life",
        ],
        "POST /series/list should sort alphabetically by title when title_sort is NULL"
    );
}

// ============================================================================
// Full Parameter Tests (full=true)
// ============================================================================

#[tokio::test]
async fn test_list_series_with_full_parameter() {
    use codex::api::routes::v1::dto::series::FullSeriesListResponse;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series with metadata
    let series1 = SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    // Add some metadata
    SeriesMetadataRepository::update_publisher(
        &db,
        series1.id,
        Some("Publisher A".to_string()),
        None,
    )
    .await
    .unwrap();
    SeriesMetadataRepository::update_summary(&db, series2.id, Some("A great series".to_string()))
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request with full=true
    let request = get_request_with_auth("/api/v1/series?full=true", &token);
    let (status, response): (StatusCode, Option<FullSeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let full_response = response.unwrap();
    assert_eq!(full_response.data.len(), 2);
    assert_eq!(full_response.total, 2);

    // Verify full responses contain metadata
    let s1 = full_response
        .data
        .iter()
        .find(|s| s.id == series1.id)
        .unwrap();
    assert_eq!(s1.metadata.publisher.as_deref(), Some("Publisher A"));

    let s2 = full_response
        .data
        .iter()
        .find(|s| s.id == series2.id)
        .unwrap();
    assert_eq!(s2.metadata.summary.as_deref(), Some("A great series"));
}

#[tokio::test]
async fn test_list_series_full_with_pagination() {
    use codex::api::routes::v1::dto::series::FullSeriesListResponse;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create 5 series
    for i in 1..=5 {
        SeriesRepository::create(&db, library.id, &format!("Series {}", i), None)
            .await
            .unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // Request first page with full=true
    let request = get_request_with_auth("/api/v1/series?full=true&page=1&pageSize=2", &token);
    let (status, response): (StatusCode, Option<FullSeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page1 = response.unwrap();
    assert_eq!(page1.data.len(), 2);
    assert_eq!(page1.total, 5);
    assert_eq!(page1.page, 1);

    // Request second page
    let app2 = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/series?full=true&page=2&pageSize=2", &token);
    let (status, response): (StatusCode, Option<FullSeriesListResponse>) =
        make_json_request(app2, request).await;

    assert_eq!(status, StatusCode::OK);
    let page2 = response.unwrap();
    assert_eq!(page2.data.len(), 2);
    assert_eq!(page2.page, 2);
}

#[tokio::test]
async fn test_list_series_filtered_with_full() {
    use codex::api::routes::v1::dto::series::FullSeriesListResponse;

    let (db, _temp_dir) = setup_test_db().await;

    // Create two libraries
    let library1 = LibraryRepository::create(&db, "Library 1", "/lib1", ScanningStrategy::Default)
        .await
        .unwrap();
    let library2 = LibraryRepository::create(&db, "Library 2", "/lib2", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series in each library
    SeriesRepository::create(&db, library1.id, "Lib1 Series", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library2.id, "Lib2 Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request from library1 with full=true using POST /series/list
    let request = post_json_request_with_auth(
        "/api/v1/series/list?full=true",
        &serde_json::json!({
            "condition": {
                "libraryId": {
                    "operator": "is",
                    "value": library1.id.to_string()
                }
            }
        }),
        &token,
    );
    let (status, response): (StatusCode, Option<FullSeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let full_response = response.unwrap();
    assert_eq!(full_response.data.len(), 1);
    assert_eq!(full_response.data[0].library_id, library1.id);
}

#[tokio::test]
async fn test_search_series_with_full() {
    use codex::api::routes::v1::dto::series::FullSeriesResponse;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library.id, "Batman Year One", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Superman Red Son", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Search with full=true
    let search_request = SearchSeriesRequest {
        query: "Batman".to_string(),
        library_id: None,
        full: true,
    };

    let request = post_json_request_with_auth(
        "/api/v1/series/search",
        &serde_json::to_value(&search_request).unwrap(),
        &token,
    );
    let (status, response): (StatusCode, Option<Vec<FullSeriesResponse>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let results = response.unwrap();
    assert_eq!(results.len(), 1);
    assert!(!results[0].metadata.title.is_empty()); // Full response has metadata with title
}

#[tokio::test]
async fn test_recently_added_series_with_full() {
    use codex::api::routes::v1::dto::series::FullSeriesResponse;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request with full=true
    let request = get_request_with_auth("/api/v1/series/recently-added?full=true&limit=10", &token);
    let (status, response): (StatusCode, Option<Vec<FullSeriesResponse>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let results = response.unwrap();
    assert_eq!(results.len(), 2);
    // Verify it's a full response (has metadata struct with title)
    assert!(results.iter().all(|s| !s.metadata.title.is_empty()));
}

#[tokio::test]
async fn test_recently_updated_series_with_full() {
    use codex::api::routes::v1::dto::series::FullSeriesResponse;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request =
        get_request_with_auth("/api/v1/series/recently-updated?full=true&limit=10", &token);
    let (status, response): (StatusCode, Option<Vec<FullSeriesResponse>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let results = response.unwrap();
    assert_eq!(results.len(), 1);
    assert!(!results[0].metadata.title.is_empty());
}

#[tokio::test]
async fn test_in_progress_series_with_full() {
    use codex::api::routes::v1::dto::series::FullSeriesResponse;
    use codex::db::repositories::ReadProgressRepository;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "In Progress Series", None)
        .await
        .unwrap();

    // Create a book with reading progress
    let book = create_test_book(series.id, library.id, "/lib/book1.cbz", "book1.cbz", None);
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let password_hash = password::hash_password("admin123").unwrap();
    let admin = create_test_user("admin", "admin@example.com", &password_hash, true);
    let admin_user = UserRepository::create(&db, &admin).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(
            admin_user.id,
            admin_user.username.clone(),
            admin_user.get_role(),
        )
        .unwrap();

    // Add reading progress
    ReadProgressRepository::upsert(&db, admin_user.id, book.id, 5, false)
        .await
        .unwrap();

    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/series/in-progress?full=true", &token);
    let (status, response): (StatusCode, Option<Vec<FullSeriesResponse>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let results = response.unwrap();
    assert_eq!(results.len(), 1);
    assert!(!results[0].metadata.title.is_empty());
}

#[tokio::test]
async fn test_library_series_with_full() {
    use codex::api::routes::v1::dto::series::FullSeriesListResponse;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/series?full=true", library.id),
        &token,
    );
    let (status, response): (StatusCode, Option<FullSeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let full_response = response.unwrap();
    assert_eq!(full_response.data.len(), 2);
    // Verify metadata is included
    assert!(
        full_response
            .data
            .iter()
            .all(|s| !s.metadata.title.is_empty())
    );
}

#[tokio::test]
async fn test_get_series_books_with_full() {
    use codex::api::routes::v1::dto::book::FullBookResponse;

    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create books
    let book1 = create_test_book(series.id, library.id, "/lib/book1.cbz", "book1.cbz", None);
    BookRepository::create(&db, &book1, None).await.unwrap();
    let book2 = create_test_book(series.id, library.id, "/lib/book2.cbz", "book2.cbz", None);
    BookRepository::create(&db, &book2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/books?full=true", series.id),
        &token,
    );
    let (status, response): (StatusCode, Option<Vec<FullBookResponse>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books = response.unwrap();
    assert_eq!(books.len(), 2);
    // Verify full book responses have metadata (field exists)
    for book in &books {
        let _ = book.metadata.locks.summary_lock;
    }
}

// ============================================================================
// Completion Filter Tests
// ============================================================================

#[tokio::test]
async fn test_list_series_filtered_by_completion_complete() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::SeriesMetadataRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create a complete series (3 books out of 3 expected)
    let complete_series = SeriesRepository::create(&db, library.id, "Complete Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_total_book_count(&db, complete_series.id, Some(3))
        .await
        .unwrap();
    for i in 1..=3 {
        let book = create_test_book(
            complete_series.id,
            library.id,
            &format!("/lib/complete/book{}.cbz", i),
            &format!("book{}.cbz", i),
            None,
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    // Create an incomplete series (2 books out of 5 expected)
    let incomplete_series = SeriesRepository::create(&db, library.id, "Incomplete Series", None)
        .await
        .unwrap();
    SeriesMetadataRepository::update_total_book_count(&db, incomplete_series.id, Some(5))
        .await
        .unwrap();
    for i in 1..=2 {
        let book = create_test_book(
            incomplete_series.id,
            library.id,
            &format!("/lib/incomplete/book{}.cbz", i),
            &format!("book{}.cbz", i),
            None,
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    // Create a series without total_book_count (should be excluded from both filters)
    let no_count_series = SeriesRepository::create(&db, library.id, "No Count Series", None)
        .await
        .unwrap();
    let book = create_test_book(
        no_count_series.id,
        library.id,
        "/lib/nocount/book1.cbz",
        "book1.cbz",
        None,
    );
    BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by completion = true (complete series)
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Completion {
            completion: BoolOperator::IsTrue,
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].title, "Complete Series");

    // Filter by completion = false (incomplete series, i.e., missing books)
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Completion {
            completion: BoolOperator::IsFalse,
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].title, "Incomplete Series");
}

#[tokio::test]
async fn test_list_series_filtered_by_completion_with_no_total_book_count() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series without total_book_count
    let series1 = SeriesRepository::create(&db, library.id, "Series Without Count", None)
        .await
        .unwrap();
    let book = create_test_book(
        series1.id,
        library.id,
        "/lib/series1/book1.cbz",
        "book1.cbz",
        None,
    );
    BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by completion = true should return empty (no series have total_book_count)
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Completion {
            completion: BoolOperator::IsTrue,
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(
        series_list.data.len(),
        0,
        "Series without total_book_count should not appear in complete filter"
    );

    // Filter by completion = false should also return empty
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::Completion {
            completion: BoolOperator::IsFalse,
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(
        series_list.data.len(),
        0,
        "Series without total_book_count should not appear in incomplete filter"
    );
}

// ============================================================================
// HasExternalSourceId Filter Tests
// ============================================================================

#[tokio::test]
async fn test_list_series_filtered_by_has_external_source_id() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::SeriesExternalIdRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create a series WITH an external source ID
    let series_with_id = SeriesRepository::create(&db, library.id, "Series With External ID", None)
        .await
        .unwrap();
    SeriesExternalIdRepository::upsert(
        &db,
        series_with_id.id,
        "plugin:mangabaka",
        "12345",
        Some("https://example.com/manga/12345"),
        None, // metadata_hash
    )
    .await
    .unwrap();

    // Create another series WITH a different external source ID
    let series_with_id2 =
        SeriesRepository::create(&db, library.id, "Series With ComicInfo ID", None)
            .await
            .unwrap();
    SeriesExternalIdRepository::upsert(
        &db,
        series_with_id2.id,
        "comicinfo",
        "CVD-98765",
        None, // external_url
        None, // metadata_hash
    )
    .await
    .unwrap();

    // Create a series WITHOUT an external source ID
    let _series_without_id =
        SeriesRepository::create(&db, library.id, "Series Without External ID", None)
            .await
            .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by hasExternalSourceId = true (series WITH external IDs)
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::HasExternalSourceId {
            has_external_source_id: BoolOperator::IsTrue,
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2);
    let titles: Vec<&str> = series_list.data.iter().map(|s| s.title.as_str()).collect();
    assert!(titles.contains(&"Series With External ID"));
    assert!(titles.contains(&"Series With ComicInfo ID"));

    // Filter by hasExternalSourceId = false (series WITHOUT external IDs)
    let request_body = SeriesListRequest {
        condition: Some(SeriesCondition::HasExternalSourceId {
            has_external_source_id: BoolOperator::IsFalse,
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/series/list", &request_body, &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 1);
    assert_eq!(series_list.data[0].title, "Series Without External ID");
}

// ============================================================================
// Reprocess Title Tests
// ============================================================================

use codex::api::routes::v1::dto::series::{
    EnqueueReprocessTitleRequest, EnqueueReprocessTitleResponse,
};
use codex::db::repositories::TaskRepository;
use codex::services::metadata::preprocessing::PreprocessingRule;
use codex::tasks::handlers::ReprocessSeriesTitleHandler;
use codex::tasks::handlers::TaskHandler;

#[tokio::test]
async fn test_reprocess_series_title_success() {
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

    // Create series with name that includes "(Digital)" suffix
    let series = SeriesRepository::create(&db, library.id, "One Piece (Digital)", None)
        .await
        .unwrap();

    // Verify initial title matches the name
    let initial_metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(initial_metadata.title, "One Piece (Digital)");

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Reprocess the title (enqueues a task)
    let request_body = EnqueueReprocessTitleRequest { dry_run: false };
    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/title/reprocess", series.id),
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

    // Execute the task to verify it works
    let task = TaskRepository::get_by_id(&db, result.task_ids[0])
        .await
        .unwrap()
        .unwrap();
    let handler = ReprocessSeriesTitleHandler::new();
    let task_result = handler.handle(&task, &db, None).await.unwrap();
    assert!(task_result.message.unwrap().contains("Title changed"));

    // Verify database was updated
    let updated_metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_metadata.title, "One Piece");
}

#[tokio::test]
async fn test_reprocess_series_title_dry_run() {
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

    // Create series
    let series = SeriesRepository::create(&db, library.id, "Naruto (Digital)", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Dry run reprocess
    let request_body = EnqueueReprocessTitleRequest { dry_run: true };
    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/title/reprocess", series.id),
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
    assert!(result.message.contains("would change"));

    // Verify database was NOT updated (dry run)
    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(metadata.title, "Naruto (Digital)");
}

#[tokio::test]
async fn test_reprocess_series_title_locked() {
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

    // Lock the title
    use codex::db::entities::series_metadata;
    let metadata = series_metadata::Entity::find_by_id(series.id)
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

    // Try to reprocess (enqueues a task)
    let request_body = EnqueueReprocessTitleRequest { dry_run: false };
    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/title/reprocess", series.id),
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<EnqueueReprocessTitleResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(result.success);
    assert_eq!(result.tasks_enqueued, 1);

    // Execute the task - it should report skipped due to lock
    let task = TaskRepository::get_by_id(&db, result.task_ids[0])
        .await
        .unwrap()
        .unwrap();
    let handler = ReprocessSeriesTitleHandler::new();
    let task_result = handler.handle(&task, &db, None).await.unwrap();
    assert!(task_result.message.unwrap().contains("Skipped"));

    // Verify title was not changed
    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(metadata.title, "Bleach (Digital)");
}

#[tokio::test]
async fn test_reprocess_series_title_no_change() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library with preprocessing rules
    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    // Add a rule that won't match
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

    // Create series without "(Digital)" suffix
    let series = SeriesRepository::create(&db, library.id, "Death Note", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Reprocess (enqueues a task)
    let request_body = EnqueueReprocessTitleRequest { dry_run: false };
    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/title/reprocess", series.id),
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<EnqueueReprocessTitleResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(result.success);
    assert_eq!(result.tasks_enqueued, 1);

    // Execute the task - it should report unchanged
    let task = TaskRepository::get_by_id(&db, result.task_ids[0])
        .await
        .unwrap()
        .unwrap();
    let handler = ReprocessSeriesTitleHandler::new();
    let task_result = handler.handle(&task, &db, None).await.unwrap();
    assert!(task_result.message.unwrap().contains("unchanged"));
}

#[tokio::test]
async fn test_reprocess_series_title_clears_title_sort() {
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
    let series = SeriesRepository::create(&db, library.id, "Attack on Titan (Digital)", None)
        .await
        .unwrap();

    // Set a custom title_sort
    use codex::db::entities::series_metadata;
    let metadata = series_metadata::Entity::find_by_id(series.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut active_meta: series_metadata::ActiveModel = metadata.into();
    active_meta.title_sort = Set(Some("attack on titan digital".to_string()));
    active_meta.update(&db).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Reprocess (enqueues a task)
    let request_body = EnqueueReprocessTitleRequest { dry_run: false };
    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/title/reprocess", series.id),
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<EnqueueReprocessTitleResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(result.success);
    assert_eq!(result.tasks_enqueued, 1);

    // Execute the task
    let task = TaskRepository::get_by_id(&db, result.task_ids[0])
        .await
        .unwrap()
        .unwrap();
    let handler = ReprocessSeriesTitleHandler::new();
    let task_result = handler.handle(&task, &db, None).await.unwrap();
    assert!(task_result.message.unwrap().contains("Title changed"));

    // Verify title_sort was cleared
    let metadata = SeriesMetadataRepository::get_by_series_id(&db, series.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(metadata.title, "Attack on Titan");
    assert!(metadata.title_sort.is_none());
}

#[tokio::test]
async fn test_reprocess_series_title_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Try to reprocess non-existent series
    let request_body = EnqueueReprocessTitleRequest { dry_run: false };
    let non_existent_id = uuid::Uuid::new_v4();
    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/title/reprocess", non_existent_id),
        &request_body,
        &token,
    );
    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}
