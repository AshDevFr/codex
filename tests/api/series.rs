#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::book::BookDto;
use codex::api::dto::series::{SearchSeriesRequest, SeriesDto, SeriesListResponse};
use codex::api::error::ErrorResponse;
use codex::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, UserRepository,
};
use codex::db::ScanningStrategy;
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
        .generate_token(created.id, created.username, created.is_admin)
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
    assert_eq!(series_response.page, 0);
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

    // Test first page with page size of 2
    let request = get_request_with_auth("/api/v1/series?page=0&page_size=2", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let page1 = response.unwrap();
    assert_eq!(page1.data.len(), 2);
    assert_eq!(page1.total, 5);
    assert_eq!(page1.page, 0);
    assert_eq!(page1.page_size, 2);

    // Test second page
    let request = get_request_with_auth("/api/v1/series?page=1&page_size=2", &token);
    let (status, response): (StatusCode, Option<SeriesListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page2 = response.unwrap();
    assert_eq!(page2.data.len(), 2);
    assert_eq!(page2.total, 5);
    assert_eq!(page2.page, 1);
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
    assert_eq!(retrieved.name, "Test Series");
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
    };

    let request = post_json_request_with_auth("/api/v1/series/search", &search_request, &token);
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series = response.unwrap();
    assert_eq!(series.len(), 2);
    assert!(series.iter().all(|s| s.name.contains("Batman")));
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
    let book1 = create_test_book(series.id, "/book1.cbz", "book1.cbz", Some("Book 1"));
    let book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(series.id, "/book2.cbz", "book2.cbz", Some("Book 2"));
    let book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let book3 = create_test_book(series.id, "/book3.cbz", "book3.cbz", Some("Book 3"));
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
    let book1 = create_test_book(series.id, "/book1.cbz", "book1.cbz", Some("Book 1"));
    let book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(series.id, "/book2.cbz", "book2.cbz", Some("Book 2"));
    let book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let book3 = create_test_book(series.id, "/book3.cbz", "book3.cbz", Some("Book 3"));
    let book3 = BookRepository::create(&db, &book3, None).await.unwrap();

    // Mark book2 as deleted
    BookRepository::mark_deleted(&db, book2.id, true, None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Get series books with include_deleted=true
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/books?include_deleted=true", series.id),
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
    let book1 = create_test_book(series.id, "/book1.cbz", "book1.cbz", Some("Book 1"));
    let book1 = BookRepository::create(&db, &book1, None).await.unwrap();
    BookRepository::mark_deleted(&db, book1.id, true, None)
        .await
        .unwrap();

    let book2 = create_test_book(series.id, "/book2.cbz", "book2.cbz", Some("Book 2"));
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

    // Get series books with include_deleted=true (should return both)
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/books?include_deleted=true", series.id),
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
    let book1 = create_test_book(series.id, "/book1.cbz", "book1.cbz", Some("Book 1"));
    let book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(series.id, "/book2.cbz", "book2.cbz", Some("Book 2"));
    let book2 = BookRepository::create(&db, &book2, None).await.unwrap();
    BookRepository::mark_deleted(&db, book2.id, true, None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Explicitly set include_deleted=false
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/books?include_deleted=false", series.id),
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
        Option<codex::api::dto::series::SeriesListResponse>,
    ) = make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 3);
    assert_eq!(series_list.total, 3);
    assert!(series_list.data.iter().all(|s| s.name.starts_with("Lib1")));

    // Request series from library 2
    let request =
        get_request_with_auth(&format!("/api/v1/libraries/{}/series", library2.id), &token);
    let (status, response): (
        StatusCode,
        Option<codex::api::dto::series::SeriesListResponse>,
    ) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.data.len(), 2);
    assert_eq!(series_list.total, 2);
    assert!(series_list.data.iter().all(|s| s.name.starts_with("Lib2")));
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

    // Request first page (page_size=10, page=0)
    let request = get_request_with_auth(
        &format!(
            "/api/v1/libraries/{}/series?page=0&page_size=10",
            library.id
        ),
        &token,
    );
    let (status, response): (
        StatusCode,
        Option<codex::api::dto::series::SeriesListResponse>,
    ) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page1 = response.unwrap();
    assert_eq!(page1.data.len(), 10);
    assert_eq!(page1.total, 15);
    assert_eq!(page1.page, 0);

    // Request second page (page=1)
    let app2 = create_test_router(state).await;
    let request = get_request_with_auth(
        &format!(
            "/api/v1/libraries/{}/series?page=1&page_size=10",
            library.id
        ),
        &token,
    );
    let (status, response): (
        StatusCode,
        Option<codex::api::dto::series::SeriesListResponse>,
    ) = make_json_request(app2, request).await;

    assert_eq!(status, StatusCode::OK);
    let page2 = response.unwrap();
    assert_eq!(page2.data.len(), 5);
    assert_eq!(page2.total, 15);
    assert_eq!(page2.page, 1);

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
        Option<codex::api::dto::series::SeriesListResponse>,
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
async fn test_list_started_series() {
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
    let book1 = create_test_book(series1.id, "/lib/s1/book1.cbz", "book1.cbz", Some("Book 1"));
    let book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(series2.id, "/lib/s2/book1.cbz", "book1.cbz", Some("Book 2"));
    let book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let book3 = create_test_book(series3.id, "/lib/s3/book1.cbz", "book1.cbz", Some("Book 3"));
    let _book3 = BookRepository::create(&db, &book3, None).await.unwrap();

    // Create admin user and get token
    let state = create_test_auth_state(db.clone()).await;
    let password_hash = password::hash_password("admin123").unwrap();
    let admin = create_test_user("admin", "admin@example.com", &password_hash, true);
    let admin_user = UserRepository::create(&db, &admin).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(admin_user.id, admin_user.username, admin_user.is_admin)
        .unwrap();

    // Add reading progress for books in series1 and series2 (in-progress) for the admin user
    use codex::db::repositories::ReadProgressRepository;
    ReadProgressRepository::upsert(&db, admin_user.id, book1.id, 5, false)
        .await
        .unwrap();
    ReadProgressRepository::upsert(&db, admin_user.id, book2.id, 5, false)
        .await
        .unwrap();

    // Mark series3's book as completed (should not be in started series)
    // Note: The endpoint filters for non-completed books only
    let app = create_test_router(state).await;

    // Request started series (should return series1 and series2)
    let request = get_request_with_auth("/api/v1/series/started", &token);
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
async fn test_list_library_started_series() {
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
    let book1 = create_test_book(series1.id, "/lib1/book1.cbz", "book1.cbz", Some("Book 1"));
    let book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book(series2.id, "/lib2/book1.cbz", "book1.cbz", Some("Book 2"));
    let book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    // Create admin user and get token
    let state = create_test_auth_state(db.clone()).await;
    let password_hash = password::hash_password("admin123").unwrap();
    let admin = create_test_user("admin", "admin@example.com", &password_hash, true);
    let admin_user = UserRepository::create(&db, &admin).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(admin_user.id, admin_user.username, admin_user.is_admin)
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

    // Request started series from library 1
    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/series/started", library1.id),
        &token,
    );
    let (status, response): (StatusCode, Option<Vec<SeriesDto>>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let series_list = response.unwrap();
    assert_eq!(series_list.len(), 1);
    assert_eq!(series_list[0].id, series1.id);

    // Request started series from library 2
    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/series/started", library2.id),
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
fn create_test_book(
    series_id: uuid::Uuid,
    path: &str,
    name: &str,
    title: Option<&str>,
) -> codex::db::entities::books::Model {
    use chrono::Utc;
    codex::db::entities::books::Model {
        id: uuid::Uuid::new_v4(),
        series_id,
        title: title.map(|s| s.to_string()),
        number: None,
        file_path: path.to_string(),
        file_name: name.to_string(),
        file_size: 1024,
        file_hash: format!("hash_{}", uuid::Uuid::new_v4()),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count: 10,
        deleted: false,
        analyzed: false,
        modified_at: Utc::now(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}
