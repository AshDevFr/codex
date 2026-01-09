#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::book::{BookDto, BookListResponse};
use codex::api::error::ErrorResponse;
use codex::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, UserRepository,
};
use codex::db::ScanningStrategy;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use sea_orm::prelude::Decimal;

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

// Helper to create a test book
fn create_test_book_model(
    series_id: uuid::Uuid,
    path: &str,
    name: &str,
    title: Option<String>,
) -> codex::db::entities::books::Model {
    use chrono::Utc;
    codex::db::entities::books::Model {
        id: uuid::Uuid::new_v4(),
        series_id,
        title,
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

// ============================================================================
// List Books Tests (Without Filter)
// ============================================================================

#[tokio::test]
async fn test_list_all_books() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series")
        .await
        .unwrap();

    // Create 5 test books
    for i in 1..=5 {
        let book = create_test_book_model(
            series.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        BookRepository::create(&db, &book).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request all books without series_id filter
    let request = get_request_with_auth("/api/v1/books", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 5);
    assert_eq!(book_list.total, 5);
    assert_eq!(book_list.page, 0);
}

#[tokio::test]
async fn test_list_all_books_with_pagination() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series")
        .await
        .unwrap();

    // Create 15 test books
    for i in 1..=15 {
        let book = create_test_book_model(
            series.id,
            &format!("/test/book{:02}.cbz", i),
            &format!("book{:02}.cbz", i),
            Some(format!("Book {:02}", i)),
        );
        BookRepository::create(&db, &book).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // Request first page (page_size=10, page=0)
    let request = get_request_with_auth("/api/v1/books?page=0&page_size=10", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page1 = response.unwrap();
    assert_eq!(page1.data.len(), 10);
    assert_eq!(page1.total, 15);
    assert_eq!(page1.page, 0);

    // Request second page (page=1)
    let app2 = create_test_router(state).await;
    let request = get_request_with_auth("/api/v1/books?page=1&page_size=10", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app2, request).await;

    assert_eq!(status, StatusCode::OK);
    let page2 = response.unwrap();
    assert_eq!(page2.data.len(), 5);
    assert_eq!(page2.total, 15);
    assert_eq!(page2.page, 1);

    // Verify different books on each page
    assert_ne!(page1.data[0].id, page2.data[0].id);
}

#[tokio::test]
async fn test_list_all_books_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request all books when there are none
    let request = get_request_with_auth("/api/v1/books", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 0);
    assert_eq!(book_list.total, 0);
}

#[tokio::test]
async fn test_list_all_books_excludes_deleted() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series")
        .await
        .unwrap();

    // Create 3 test books
    let mut book_ids = vec![];
    for i in 1..=3 {
        let book = create_test_book_model(
            series.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        let created = BookRepository::create(&db, &book).await.unwrap();
        book_ids.push(created.id);
    }

    // Mark one book as deleted
    BookRepository::mark_deleted(&db, book_ids[1], true)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request all books (should exclude deleted)
    let request = get_request_with_auth("/api/v1/books", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 2);
    assert_eq!(book_list.total, 2);

    // Verify the deleted book is not in the list
    assert!(!book_list.data.iter().any(|b| b.id == book_ids[1]));
}

#[tokio::test]
async fn test_list_all_books_ordered_by_title() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series")
        .await
        .unwrap();

    // Create books with different titles (not in alphabetical order)
    let titles = vec!["Zebra", "Apple", "Monkey", "Banana"];
    for title in titles {
        let book = create_test_book_model(
            series.id,
            &format!("/test/{}.cbz", title),
            &format!("{}.cbz", title),
            Some(title.to_string()),
        );
        BookRepository::create(&db, &book).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request all books
    let request = get_request_with_auth("/api/v1/books", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 4);

    // Verify books are ordered by title (alphabetically)
    assert_eq!(book_list.data[0].title, "Apple");
    assert_eq!(book_list.data[1].title, "Banana");
    assert_eq!(book_list.data[2].title, "Monkey");
    assert_eq!(book_list.data[3].title, "Zebra");
}

#[tokio::test]
async fn test_list_all_books_across_multiple_series() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and two series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Series 1")
        .await
        .unwrap();

    let series2 = SeriesRepository::create(&db, library.id, "Series 2")
        .await
        .unwrap();

    // Create books in series 1
    for i in 1..=3 {
        let book = create_test_book_model(
            series1.id,
            &format!("/test/s1/book{}.cbz", i),
            &format!("s1_book{}.cbz", i),
            Some(format!("Series 1 Book {}", i)),
        );
        BookRepository::create(&db, &book).await.unwrap();
    }

    // Create books in series 2
    for i in 1..=2 {
        let book = create_test_book_model(
            series2.id,
            &format!("/test/s2/book{}.cbz", i),
            &format!("s2_book{}.cbz", i),
            Some(format!("Series 2 Book {}", i)),
        );
        BookRepository::create(&db, &book).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request all books (should include both series)
    let request = get_request_with_auth("/api/v1/books", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 5);
    assert_eq!(book_list.total, 5);

    // Verify books from both series are present
    let series1_count = book_list
        .data
        .iter()
        .filter(|b| b.series_id == series1.id)
        .count();
    let series2_count = book_list
        .data
        .iter()
        .filter(|b| b.series_id == series2.id)
        .count();
    assert_eq!(series1_count, 3);
    assert_eq!(series2_count, 2);
}

// ============================================================================
// List Books by Series Tests (Existing Functionality)
// ============================================================================

#[tokio::test]
async fn test_list_books_by_series() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series")
        .await
        .unwrap();

    // Create test books
    for i in 1..=3 {
        let book = create_test_book_model(
            series.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        BookRepository::create(&db, &book).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request books for specific series
    let request = get_request_with_auth(&format!("/api/v1/books?series_id={}", series.id), &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 3);
    assert_eq!(book_list.total, 3);
    assert!(book_list.data.iter().all(|b| b.series_id == series.id));
}

// ============================================================================
// Authorization Tests
// ============================================================================

#[tokio::test]
async fn test_list_books_requires_authentication() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Request without authentication
    let request = get_request("/api/v1/books");
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(response.is_some());
}
