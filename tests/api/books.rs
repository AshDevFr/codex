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
use tower::ServiceExt;

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
// Note: title is now in book_metadata table, not books table
fn create_test_book_model(
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
    path: &str,
    name: &str,
    _title: Option<String>, // No longer used - title is in book_metadata
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
        modified_at: Utc::now(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        thumbnail_path: None,
        thumbnail_generated_at: None,
    }
}

// Helper to create book with metadata (title is now in book_metadata table)
async fn create_test_book_with_metadata(
    db: &sea_orm::DatabaseConnection,
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
    path: &str,
    name: &str,
    title: Option<String>,
) -> codex::db::entities::books::Model {
    use codex::db::repositories::{BookMetadataRepository, BookRepository};

    let book = create_test_book_model(series_id, library_id, path, name, title.clone());
    let created = BookRepository::create(db, &book, None).await.unwrap();

    // Create metadata with title if provided
    if title.is_some() {
        BookMetadataRepository::create_with_title_and_number(db, created.id, title, None)
            .await
            .unwrap();
    }

    created
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

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 5 test books
    for i in 1..=5 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
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

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 15 test books
    for i in 1..=15 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{:02}.cbz", i),
            &format!("book{:02}.cbz", i),
            Some(format!("Book {:02}", i)),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
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

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 3 test books
    let mut book_ids = vec![];
    for i in 1..=3 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        let created = BookRepository::create(&db, &book, None).await.unwrap();
        book_ids.push(created.id);
    }

    // Mark one book as deleted
    BookRepository::mark_deleted(&db, book_ids[1], true, None)
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

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create books with different titles (not in alphabetical order)
    let titles = vec!["Zebra", "Apple", "Monkey", "Banana"];
    for title in titles {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/{}.cbz", title),
            &format!("{}.cbz", title),
            Some(title.to_string()),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
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

    let series1 = SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();

    let series2 = SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    // Create books in series 1
    for i in 1..=3 {
        let book = create_test_book_model(
            series1.id,
            library.id,
            &format!("/test/s1/book{}.cbz", i),
            &format!("s1_book{}.cbz", i),
            Some(format!("Series 1 Book {}", i)),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    // Create books in series 2
    for i in 1..=2 {
        let book = create_test_book_model(
            series2.id,
            library.id,
            &format!("/test/s2/book{}.cbz", i),
            &format!("s2_book{}.cbz", i),
            Some(format!("Series 2 Book {}", i)),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
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

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create test books
    for i in 1..=3 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
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

// ============================================================================
// List Library Books Tests
// ============================================================================

#[tokio::test]
async fn test_list_library_books() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create two libraries
    let library1 = LibraryRepository::create(&db, "Library 1", "/test1", ScanningStrategy::Default)
        .await
        .unwrap();
    let library2 = LibraryRepository::create(&db, "Library 2", "/test2", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series in each library
    let series1 = SeriesRepository::create(&db, library1.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library2.id, "Series 2", None)
        .await
        .unwrap();

    // Create books in each series
    for i in 1..=3 {
        let book = create_test_book_model(
            series1.id,
            library1.id,
            &format!("/test1/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    for i in 1..=2 {
        let book = create_test_book_model(
            series2.id,
            library2.id,
            &format!("/test2/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request books from library 1
    let request =
        get_request_with_auth(&format!("/api/v1/libraries/{}/books", library1.id), &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 3);
    assert_eq!(book_list.total, 3);

    // Request books from library 2
    let request =
        get_request_with_auth(&format!("/api/v1/libraries/{}/books", library2.id), &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 2);
    assert_eq!(book_list.total, 2);
}

// ============================================================================
// In-Progress Books Tests
// ============================================================================

#[tokio::test]
async fn test_list_in_progress_books() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create books
    let mut book_ids = Vec::new();
    for i in 1..=5 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        let created = BookRepository::create(&db, &book, None).await.unwrap();
        book_ids.push(created.id);
    }

    let state = create_test_auth_state(db.clone()).await;

    // Create admin user and get token
    let password_hash = password::hash_password("admin123").unwrap();
    let admin = create_test_user("admin", "admin@example.com", &password_hash, true);
    let admin_user = UserRepository::create(&db, &admin).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(admin_user.id, admin_user.username, admin_user.is_admin)
        .unwrap();

    // Add reading progress for the admin user
    use codex::db::repositories::ReadProgressRepository;

    // Mark 3 books as in-progress
    for i in 0..3 {
        ReadProgressRepository::upsert(&db, admin_user.id, book_ids[i], 5, false)
            .await
            .unwrap();
    }

    // Mark 1 book as completed
    ReadProgressRepository::upsert(&db, admin_user.id, book_ids[3], 10, true)
        .await
        .unwrap();

    let app = create_test_router(state).await;

    // Request in-progress books (should return 3)
    let request = get_request_with_auth("/api/v1/books/in-progress", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 3); // Only in-progress books, not completed
    assert_eq!(book_list.total, 3);
}

// ============================================================================
// On-Deck Books Tests
// ============================================================================

#[tokio::test]
async fn test_list_on_deck_books() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create books in the series (numbered 1-5)
    // Note: number is now in book_metadata table
    use codex::db::repositories::BookMetadataRepository;
    let mut book_ids = Vec::new();
    for i in 1..=5 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        let created = BookRepository::create(&db, &book, None).await.unwrap();
        // Create book metadata with number
        BookMetadataRepository::create_with_title_and_number(
            &db,
            created.id,
            Some(format!("Book {}", i)),
            Some(sea_orm::prelude::Decimal::from(i)),
        )
        .await
        .unwrap();
        book_ids.push(created.id);
    }

    let state = create_test_auth_state(db.clone()).await;

    // Create admin user and get token
    let password_hash = password::hash_password("admin123").unwrap();
    let admin = create_test_user("admin", "admin@example.com", &password_hash, true);
    let admin_user = UserRepository::create(&db, &admin).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(admin_user.id, admin_user.username, admin_user.is_admin)
        .unwrap();

    use codex::db::repositories::ReadProgressRepository;

    // Mark first 2 books as completed
    ReadProgressRepository::upsert(&db, admin_user.id, book_ids[0], 10, true)
        .await
        .unwrap();
    ReadProgressRepository::upsert(&db, admin_user.id, book_ids[1], 10, true)
        .await
        .unwrap();

    let app = create_test_router(state).await;

    // Request on-deck books - should return book 3 (first unread book in series with completed books)
    let request = get_request_with_auth("/api/v1/books/on-deck", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 1); // Only 1 on-deck book (first unread in series)
    assert_eq!(book_list.total, 1);
    assert_eq!(book_list.data[0].id, book_ids[2]); // Book 3 (0-indexed as 2)
}

#[tokio::test]
async fn test_list_on_deck_excludes_series_with_in_progress() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create books in the series (numbered 1-5)
    // Note: number is now in book_metadata table
    use codex::db::repositories::BookMetadataRepository;
    let mut book_ids = Vec::new();
    for i in 1..=5 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        let created = BookRepository::create(&db, &book, None).await.unwrap();
        // Create book metadata with number
        BookMetadataRepository::create_with_title_and_number(
            &db,
            created.id,
            Some(format!("Book {}", i)),
            Some(sea_orm::prelude::Decimal::from(i)),
        )
        .await
        .unwrap();
        book_ids.push(created.id);
    }

    let state = create_test_auth_state(db.clone()).await;

    // Create admin user and get token
    let password_hash = password::hash_password("admin123").unwrap();
    let admin = create_test_user("admin", "admin@example.com", &password_hash, true);
    let admin_user = UserRepository::create(&db, &admin).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(admin_user.id, admin_user.username, admin_user.is_admin)
        .unwrap();

    use codex::db::repositories::ReadProgressRepository;

    // Mark first book as completed
    ReadProgressRepository::upsert(&db, admin_user.id, book_ids[0], 10, true)
        .await
        .unwrap();

    // Mark second book as in-progress (not completed)
    ReadProgressRepository::upsert(&db, admin_user.id, book_ids[1], 5, false)
        .await
        .unwrap();

    let app = create_test_router(state).await;

    // Request on-deck books - should be empty because series has in-progress book
    let request = get_request_with_auth("/api/v1/books/on-deck", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 0); // No on-deck books - series has in-progress book
    assert_eq!(book_list.total, 0);
}

#[tokio::test]
async fn test_list_on_deck_empty_when_no_completed_books() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create books in the series
    for i in 1..=5 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;

    // Create admin user and get token
    let password_hash = password::hash_password("admin123").unwrap();
    let admin = create_test_user("admin", "admin@example.com", &password_hash, true);
    let admin_user = UserRepository::create(&db, &admin).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(admin_user.id, admin_user.username, admin_user.is_admin)
        .unwrap();

    let app = create_test_router(state).await;

    // Request on-deck books - should be empty because no books are completed
    let request = get_request_with_auth("/api/v1/books/on-deck", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 0); // No on-deck books - no completed books
    assert_eq!(book_list.total, 0);
}

// ============================================================================
// Recently Added Books Tests
// ============================================================================

#[tokio::test]
async fn test_list_recently_added_books() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create books with small delays to ensure different created_at timestamps
    for i in 1..=5 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request recently added books
    let request = get_request_with_auth("/api/v1/books/recently-added", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 5);
    assert_eq!(book_list.total, 5);

    // Verify books are ordered by created_at descending (most recent first)
    for i in 0..book_list.data.len() - 1 {
        assert!(
            book_list.data[i].created_at >= book_list.data[i + 1].created_at,
            "Books should be ordered by created_at descending"
        );
    }
}

#[tokio::test]
async fn test_list_library_recently_added_books() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create two libraries
    let library1 = LibraryRepository::create(&db, "Library 1", "/test1", ScanningStrategy::Default)
        .await
        .unwrap();
    let library2 = LibraryRepository::create(&db, "Library 2", "/test2", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series in each library
    let series1 = SeriesRepository::create(&db, library1.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library2.id, "Series 2", None)
        .await
        .unwrap();

    // Create books in library 1 with metadata
    for i in 1..=3 {
        create_test_book_with_metadata(
            &db,
            series1.id,
            library1.id,
            &format!("/test1/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Library 1 Book {}", i)),
        )
        .await;
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Create books in library 2 with metadata
    for i in 1..=2 {
        create_test_book_with_metadata(
            &db,
            series2.id,
            library2.id,
            &format!("/test2/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Library 2 Book {}", i)),
        )
        .await;
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request recently added books from library 1
    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/books/recently-added", library1.id),
        &token,
    );
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 3);
    assert_eq!(book_list.total, 3);

    // Verify all books are from library 1
    for book in &book_list.data {
        assert!(book.title.contains("Library 1"));
    }
}

// ============================================================================
// Series Name and Title Fallback Tests
// ============================================================================

#[tokio::test]
async fn test_books_include_series_name() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series Name", None)
        .await
        .unwrap();

    // Create a book with a title (in book_metadata table)
    create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "/test/book1.cbz",
        "book1.cbz",
        Some("Book Title".to_string()),
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request the book
    let request = get_request_with_auth("/api/v1/books", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 1);

    // Verify series name is included
    let returned_book = &book_list.data[0];
    assert_eq!(returned_book.series_name, "Test Series Name");
    assert_eq!(returned_book.title, "Book Title");
}

#[tokio::test]
async fn test_books_use_filename_when_title_is_none() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create a book without a title (title is None)
    let book = create_test_book_model(
        series.id,
        library.id,
        "/test/mybook.cbz",
        "mybook.cbz",
        None,
    );
    BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request the book
    let request = get_request_with_auth("/api/v1/books", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 1);

    // Verify filename (without extension) is used as title
    let returned_book = &book_list.data[0];
    assert_eq!(returned_book.title, "mybook");
}

#[tokio::test]
async fn test_books_filename_fallback_with_multiple_extensions() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create a book with multiple dots in filename
    let book = create_test_book_model(
        series.id,
        library.id,
        "/test/book.vol.1.cbz",
        "book.vol.1.cbz",
        None,
    );
    BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request the book
    let request = get_request_with_auth("/api/v1/books", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 1);

    // Verify filename without last extension is used
    let returned_book = &book_list.data[0];
    assert_eq!(returned_book.title, "book.vol.1");
}

// ============================================================================
// Books With Errors Tests
// ============================================================================

// Helper to create a test book with an analysis error
// Note: title is now in book_metadata table, not books table
fn create_test_book_with_error(
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
    path: &str,
    name: &str,
    error: &str,
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
        page_count: 0,
        deleted: false,
        analyzed: false,
        analysis_error: Some(error.to_string()),
        modified_at: Utc::now(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        thumbnail_path: None,
        thumbnail_generated_at: None,
    }
}

#[tokio::test]
async fn test_list_books_with_errors() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 2 books without errors
    for i in 1..=2 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/good{}.cbz", i),
            &format!("good{}.cbz", i),
            Some(format!("Good Book {}", i)),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    // Create 3 books with errors
    for i in 1..=3 {
        let book = create_test_book_with_error(
            series.id,
            library.id,
            &format!("/test/bad{}.cbz", i),
            &format!("bad{}.cbz", i),
            &format!("Failed to parse CBZ: invalid archive {}", i),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request books with errors
    let request = get_request_with_auth("/api/v1/books/with-errors", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 3);
    assert_eq!(book_list.total, 3);

    // Verify all returned books have analysis errors
    for book in &book_list.data {
        assert!(book.analysis_error.is_some());
        assert!(book
            .analysis_error
            .as_ref()
            .unwrap()
            .contains("Failed to parse CBZ"));
    }
}

#[tokio::test]
async fn test_list_books_with_errors_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create books without errors
    for i in 1..=3 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request books with errors (should be empty)
    let request = get_request_with_auth("/api/v1/books/with-errors", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 0);
    assert_eq!(book_list.total, 0);
}

#[tokio::test]
async fn test_list_library_books_with_errors() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create two libraries
    let library1 = LibraryRepository::create(&db, "Library 1", "/test1", ScanningStrategy::Default)
        .await
        .unwrap();
    let library2 = LibraryRepository::create(&db, "Library 2", "/test2", ScanningStrategy::Default)
        .await
        .unwrap();

    let series1 = SeriesRepository::create(&db, library1.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library2.id, "Series 2", None)
        .await
        .unwrap();

    // Create 2 books with errors in library1
    for i in 1..=2 {
        let book = create_test_book_with_error(
            series1.id,
            library1.id,
            &format!("/test1/bad{}.cbz", i),
            &format!("bad{}.cbz", i),
            &format!("Error in library 1: {}", i),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    // Create 3 books with errors in library2
    for i in 1..=3 {
        let book = create_test_book_with_error(
            series2.id,
            library2.id,
            &format!("/test2/bad{}.cbz", i),
            &format!("bad{}.cbz", i),
            &format!("Error in library 2: {}", i),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request books with errors for library1
    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/books/with-errors", library1.id),
        &token,
    );
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 2);
    assert_eq!(book_list.total, 2);

    // Verify all returned books are from library1 (via series1) and have errors
    for book in &book_list.data {
        assert_eq!(book.series_id, series1.id);
        assert!(book.analysis_error.is_some());
        assert!(book
            .analysis_error
            .as_ref()
            .unwrap()
            .contains("Error in library 1"));
    }
}

#[tokio::test]
async fn test_list_library_books_with_errors_nonexistent_library() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request books with errors for non-existent library
    // API returns 200 with empty list (consistent with list_library_books behavior)
    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/books/with-errors", fake_id),
        &token,
    );
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 0);
    assert_eq!(book_list.total, 0);
}

#[tokio::test]
async fn test_list_series_books_with_errors() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library with two series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series1 = SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    // Create 2 books with errors in series1
    for i in 1..=2 {
        let book = create_test_book_with_error(
            series1.id,
            library.id,
            &format!("/test/series1/bad{}.cbz", i),
            &format!("bad{}.cbz", i),
            &format!("Error in series 1: {}", i),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    // Create 1 good book in series1
    let good_book = create_test_book_model(
        series1.id,
        library.id,
        "/test/series1/good.cbz",
        "good.cbz",
        Some("Good Book".to_string()),
    );
    BookRepository::create(&db, &good_book, None).await.unwrap();

    // Create 3 books with errors in series2
    for i in 1..=3 {
        let book = create_test_book_with_error(
            series2.id,
            library.id,
            &format!("/test/series2/bad{}.cbz", i),
            &format!("bad{}.cbz", i),
            &format!("Error in series 2: {}", i),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request books with errors for series1
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/books/with-errors", series1.id),
        &token,
    );
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 2);
    assert_eq!(book_list.total, 2);

    // Verify all returned books are from series1 and have errors
    for book in &book_list.data {
        assert_eq!(book.series_id, series1.id);
        assert!(book.analysis_error.is_some());
        assert!(book
            .analysis_error
            .as_ref()
            .unwrap()
            .contains("Error in series 1"));
    }
}

#[tokio::test]
async fn test_list_series_books_with_errors_nonexistent_series() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request books with errors for non-existent series
    // API returns 200 with empty list (consistent with list_series_books behavior)
    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(
        &format!("/api/v1/series/{}/books/with-errors", fake_id),
        &token,
    );
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 0);
    assert_eq!(book_list.total, 0);
}

#[tokio::test]
async fn test_list_books_with_errors_pagination() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 15 books with errors
    for i in 1..=15 {
        let book = create_test_book_with_error(
            series.id,
            library.id,
            &format!("/test/bad{:02}.cbz", i),
            &format!("bad{:02}.cbz", i),
            &format!("Error {}", i),
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request first page (10 items) - pages are 0-indexed
    let request = get_request_with_auth("/api/v1/books/with-errors?page=0&page_size=10", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 10);
    assert_eq!(book_list.total, 15);
    assert_eq!(book_list.page, 0);
    assert_eq!(book_list.page_size, 10);

    // Request second page
    let app2 = create_test_router(create_test_auth_state(db.clone()).await).await;
    let request = get_request_with_auth("/api/v1/books/with-errors?page=1&page_size=10", &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app2, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 5);
    assert_eq!(book_list.total, 15);
    assert_eq!(book_list.page, 1);
}

#[tokio::test]
async fn test_list_books_with_errors_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // Request without auth token
    let request = get_request("/api/v1/books/with-errors");
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Recently Read Books Tests
// ============================================================================

#[tokio::test]
async fn test_list_recently_read_books() {
    let (db, _temp_dir) = setup_test_db().await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create books
    let book1 = create_test_book_model(
        series.id,
        library.id,
        "/test/book1.cbz",
        "book1.cbz",
        Some("Book 1".to_string()),
    );
    let book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book_model(
        series.id,
        library.id,
        "/test/book2.cbz",
        "book2.cbz",
        Some("Book 2".to_string()),
    );
    let book2 = BookRepository::create(&db, &book2, None).await.unwrap();

    let book3 = create_test_book_model(
        series.id,
        library.id,
        "/test/book3.cbz",
        "book3.cbz",
        Some("Book 3".to_string()),
    );
    let book3 = BookRepository::create(&db, &book3, None).await.unwrap();

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
            admin_user.is_admin,
        )
        .unwrap();

    // Add reading progress for books in a specific order
    use codex::db::repositories::ReadProgressRepository;
    ReadProgressRepository::upsert(&db, admin_user.id, book1.id, 5, false)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    ReadProgressRepository::upsert(&db, admin_user.id, book3.id, 3, false)
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    ReadProgressRepository::upsert(&db, admin_user.id, book2.id, 7, true)
        .await
        .unwrap();

    let app = create_test_router(state).await;

    // Request recently read books
    let request = get_request_with_auth("/api/v1/books/recently-read?limit=50", &token);
    let (status, response): (StatusCode, Option<Vec<BookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books = response.unwrap();
    assert_eq!(books.len(), 3);
    // Should be ordered by updated_at descending (most recent first)
    assert_eq!(books[0].id, book2.id);
    assert_eq!(books[1].id, book3.id);
    assert_eq!(books[2].id, book1.id);
}

#[tokio::test]
async fn test_list_recently_read_books_with_limit() {
    let (db, _temp_dir) = setup_test_db().await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 5 books
    let mut books = vec![];
    for i in 1..=5 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
        );
        let created = BookRepository::create(&db, &book, None).await.unwrap();
        books.push(created);
    }

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
            admin_user.is_admin,
        )
        .unwrap();

    // Add reading progress for all books
    use codex::db::repositories::ReadProgressRepository;
    for book in &books {
        ReadProgressRepository::upsert(&db, admin_user.id, book.id, 1, false)
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    }

    let app = create_test_router(state).await;

    // Request with limit=2
    let request = get_request_with_auth("/api/v1/books/recently-read?limit=2", &token);
    let (status, response): (StatusCode, Option<Vec<BookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert_eq!(result.len(), 2);
}

#[tokio::test]
async fn test_list_library_recently_read_books() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create two libraries
    let library1 = LibraryRepository::create(&db, "Library 1", "/test1", ScanningStrategy::Default)
        .await
        .unwrap();
    let library2 = LibraryRepository::create(&db, "Library 2", "/test2", ScanningStrategy::Default)
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
    let book1 = create_test_book_model(
        series1.id,
        library1.id,
        "/test1/book1.cbz",
        "book1.cbz",
        Some("Lib1 Book".to_string()),
    );
    let book1 = BookRepository::create(&db, &book1, None).await.unwrap();

    let book2 = create_test_book_model(
        series2.id,
        library2.id,
        "/test2/book1.cbz",
        "book1.cbz",
        Some("Lib2 Book".to_string()),
    );
    let book2 = BookRepository::create(&db, &book2, None).await.unwrap();

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
            admin_user.is_admin,
        )
        .unwrap();

    // Add reading progress for both books
    use codex::db::repositories::ReadProgressRepository;
    ReadProgressRepository::upsert(&db, admin_user.id, book1.id, 5, false)
        .await
        .unwrap();
    ReadProgressRepository::upsert(&db, admin_user.id, book2.id, 5, false)
        .await
        .unwrap();

    let app = create_test_router(state).await;

    // Request recently read books from library 1
    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/books/recently-read", library1.id),
        &token,
    );
    let (status, response): (StatusCode, Option<Vec<BookDto>>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let books = response.unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, book1.id);

    // Request recently read books from library 2
    let request = get_request_with_auth(
        &format!("/api/v1/libraries/{}/books/recently-read", library2.id),
        &token,
    );
    let (status, response): (StatusCode, Option<Vec<BookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books = response.unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, book2.id);
}

#[tokio::test]
async fn test_list_recently_read_books_empty_when_no_progress() {
    let (db, _temp_dir) = setup_test_db().await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create a book (but don't add reading progress)
    let book = create_test_book_model(
        series.id,
        library.id,
        "/test/book1.cbz",
        "book1.cbz",
        Some("Book 1".to_string()),
    );
    BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request recently read books (should be empty)
    let request = get_request_with_auth("/api/v1/books/recently-read", &token);
    let (status, response): (StatusCode, Option<Vec<BookDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books = response.unwrap();
    assert_eq!(books.len(), 0);
}

#[tokio::test]
async fn test_list_recently_read_books_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // Request without auth token
    let request = get_request("/api/v1/books/recently-read");
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// =============================================================================
// Book File Download Tests
// =============================================================================

#[tokio::test]
async fn test_get_book_file_success() {
    use common::files::create_test_cbz;

    let (db, temp_dir) = setup_test_db().await;

    // Create test CBZ file
    let cbz_path = create_test_cbz(&temp_dir, 3, false);

    // Create library and series
    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create book with real file path
    let book = create_test_book_model(
        series.id,
        library.id,
        cbz_path.to_str().unwrap(),
        "test_comic.cbz",
        Some("Test Comic".to_string()),
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request the file
    let request = get_request_with_auth(&format!("/api/v1/books/{}/file", book.id), &token);
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Check headers
    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(content_type, "application/zip");

    let content_disposition = response
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_disposition.contains("attachment"));
    assert!(content_disposition.contains("test_comic.cbz"));

    // Verify we got file content
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(!body.is_empty());
}

#[tokio::test]
async fn test_get_book_file_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let app = create_test_router(state).await;

    // Request non-existent book
    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/books/{}/file", fake_id), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_book_file_missing_on_disk() {
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
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create book with non-existent file path
    let book = create_test_book_model(
        series.id,
        library.id,
        "/nonexistent/path/book.cbz",
        "book.cbz",
        Some("Missing Book".to_string()),
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request the file
    let request = get_request_with_auth(&format!("/api/v1/books/{}/file", book.id), &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_book_file_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;

    let app = create_test_router(state).await;

    // Request without auth token
    let fake_id = uuid::Uuid::new_v4();
    let request = get_request(&format!("/api/v1/books/{}/file", fake_id));
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Book Metadata Tests (PUT and PATCH)
// ============================================================================

use codex::api::dto::book::{BookMetadataResponse, ReplaceBookMetadataRequest};

#[tokio::test]
async fn test_replace_book_metadata_creates_record() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book = create_test_book_model(
        series.id,
        library.id,
        "/path/book.cbz",
        "book.cbz",
        Some("Test Book".to_string()),
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request_body = ReplaceBookMetadataRequest {
        summary: Some("A great book".to_string()),
        writer: Some("Frank Miller".to_string()),
        penciller: Some("David Mazzucchelli".to_string()),
        inker: None,
        colorist: None,
        letterer: None,
        cover_artist: None,
        editor: None,
        publisher: Some("DC Comics".to_string()),
        imprint: None,
        genre: Some("Superhero".to_string()),
        web: None,
        language_iso: Some("en".to_string()),
        format_detail: None,
        black_and_white: Some(false),
        manga: Some(false),
        year: Some(1987),
        month: Some(2),
        day: None,
        volume: None,
        count: None,
        isbns: None,
    };

    let request = put_json_request_with_auth(
        &format!("/api/v1/books/{}/metadata", book.id),
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<BookMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metadata = response.unwrap();
    assert_eq!(metadata.book_id, book.id);
    assert_eq!(metadata.summary, Some("A great book".to_string()));
    assert_eq!(metadata.writer, Some("Frank Miller".to_string()));
    assert_eq!(metadata.publisher, Some("DC Comics".to_string()));
    assert_eq!(metadata.year, Some(1987));
    assert_eq!(metadata.inker, None); // Was omitted
}

#[tokio::test]
async fn test_replace_book_metadata_clears_omitted_fields() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book = create_test_book_model(
        series.id,
        library.id,
        "/path/book.cbz",
        "book.cbz",
        Some("Test Book".to_string()),
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // First, create metadata with some fields
    let request_body = ReplaceBookMetadataRequest {
        summary: Some("Original summary".to_string()),
        writer: Some("Original writer".to_string()),
        penciller: None,
        inker: None,
        colorist: None,
        letterer: None,
        cover_artist: None,
        editor: None,
        publisher: Some("Original publisher".to_string()),
        imprint: None,
        genre: None,
        web: None,
        language_iso: None,
        format_detail: None,
        black_and_white: None,
        manga: None,
        year: Some(2020),
        month: None,
        day: None,
        volume: None,
        count: None,
        isbns: None,
    };

    let request = put_json_request_with_auth(
        &format!("/api/v1/books/{}/metadata", book.id),
        &request_body,
        &token,
    );
    let (status, _): (StatusCode, Option<BookMetadataResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Now replace with fewer fields - others should be cleared
    let app = create_test_router(state).await;
    let request_body = ReplaceBookMetadataRequest {
        summary: Some("New summary".to_string()),
        writer: None, // Was set, should be cleared
        penciller: None,
        inker: None,
        colorist: None,
        letterer: None,
        cover_artist: None,
        editor: None,
        publisher: None, // Was set, should be cleared
        imprint: None,
        genre: None,
        web: None,
        language_iso: None,
        format_detail: None,
        black_and_white: None,
        manga: None,
        year: None, // Was set, should be cleared
        month: None,
        day: None,
        volume: None,
        count: None,
        isbns: None,
    };

    let request = put_json_request_with_auth(
        &format!("/api/v1/books/{}/metadata", book.id),
        &request_body,
        &token,
    );
    let (status, response): (StatusCode, Option<BookMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metadata = response.unwrap();
    assert_eq!(metadata.summary, Some("New summary".to_string()));
    assert_eq!(metadata.writer, None); // Cleared
    assert_eq!(metadata.publisher, None); // Cleared
    assert_eq!(metadata.year, None); // Cleared
}

#[tokio::test]
async fn test_replace_book_metadata_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request_body = ReplaceBookMetadataRequest {
        summary: Some("Test".to_string()),
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
        web: None,
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
    };

    let request = put_json_request_with_auth(
        &format!("/api/v1/books/{}/metadata", fake_id),
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
async fn test_patch_book_metadata_partial_update() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book = create_test_book_model(
        series.id,
        library.id,
        "/path/book.cbz",
        "book.cbz",
        Some("Test Book".to_string()),
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // First create metadata
    let request = put_json_request_with_auth(
        &format!("/api/v1/books/{}/metadata", book.id),
        &serde_json::json!({
            "summary": "Original summary",
            "writer": "Original writer",
            "publisher": "Original publisher"
        }),
        &token,
    );
    let (status, _): (StatusCode, Option<BookMetadataResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Now PATCH to update only summary
    let app = create_test_router(state).await;
    let request = patch_json_request_with_auth(
        &format!("/api/v1/books/{}/metadata", book.id),
        &serde_json::json!({
            "summary": "Updated summary"
        }),
        &token,
    );
    let (status, response): (StatusCode, Option<BookMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metadata = response.unwrap();
    assert_eq!(metadata.summary, Some("Updated summary".to_string())); // Updated
    assert_eq!(metadata.writer, Some("Original writer".to_string())); // Unchanged
    assert_eq!(metadata.publisher, Some("Original publisher".to_string())); // Unchanged
}

#[tokio::test]
async fn test_patch_book_metadata_explicit_null_clears() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book = create_test_book_model(
        series.id,
        library.id,
        "/path/book.cbz",
        "book.cbz",
        Some("Test Book".to_string()),
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state.clone()).await;

    // First create metadata
    let request = put_json_request_with_auth(
        &format!("/api/v1/books/{}/metadata", book.id),
        &serde_json::json!({
            "summary": "A summary",
            "writer": "A writer",
            "publisher": "A publisher"
        }),
        &token,
    );
    let (status, _): (StatusCode, Option<BookMetadataResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // PATCH with null to clear a specific field
    let app = create_test_router(state).await;
    let request = patch_json_request_with_auth(
        &format!("/api/v1/books/{}/metadata", book.id),
        &serde_json::json!({
            "writer": null
        }),
        &token,
    );
    let (status, response): (StatusCode, Option<BookMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metadata = response.unwrap();
    assert_eq!(metadata.summary, Some("A summary".to_string())); // Unchanged
    assert_eq!(metadata.writer, None); // Cleared by null
    assert_eq!(metadata.publisher, Some("A publisher".to_string())); // Unchanged
}

#[tokio::test]
async fn test_patch_book_metadata_creates_record_if_missing() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book = create_test_book_model(
        series.id,
        library.id,
        "/path/book.cbz",
        "book.cbz",
        Some("Test Book".to_string()),
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // PATCH without existing metadata record - should create one
    let request = patch_json_request_with_auth(
        &format!("/api/v1/books/{}/metadata", book.id),
        &serde_json::json!({
            "summary": "New summary",
            "writer": "New writer"
        }),
        &token,
    );
    let (status, response): (StatusCode, Option<BookMetadataResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metadata = response.unwrap();
    assert_eq!(metadata.book_id, book.id);
    assert_eq!(metadata.summary, Some("New summary".to_string()));
    assert_eq!(metadata.writer, Some("New writer".to_string()));
}

#[tokio::test]
async fn test_patch_book_metadata_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = patch_json_request_with_auth(
        &format!("/api/v1/books/{}/metadata", fake_id),
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
async fn test_book_metadata_without_auth() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book = create_test_book_model(
        series.id,
        library.id,
        "/path/book.cbz",
        "book.cbz",
        Some("Test Book".to_string()),
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    // PUT without auth
    let request = put_json_request(
        &format!("/api/v1/books/{}/metadata", book.id),
        &serde_json::json!({"summary": "Test"}),
    );

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");

    // PATCH without auth
    let request = patch_json_request(
        &format!("/api/v1/books/{}/metadata", book.id),
        &serde_json::json!({"summary": "Test"}),
    );

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

// ============================================================================
// POST /books/list Filtering Tests
// ============================================================================

use codex::api::dto::filter::{
    BookCondition, BookListRequest, BoolOperator, FieldOperator, UuidOperator,
};

#[tokio::test]
async fn test_list_books_filtered_no_condition() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();

    let book1 = create_test_book_model(series.id, library.id, "/book1.cbz", "book1.cbz", None);
    let book2 = create_test_book_model(series.id, library.id, "/book2.cbz", "book2.cbz", None);
    BookRepository::create(&db, &book1, None).await.unwrap();
    BookRepository::create(&db, &book2, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // POST with no condition should return all books
    let request_body = BookListRequest::default();
    let request = post_json_request_with_auth("/api/v1/books/list", &request_body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books_list = response.unwrap();
    assert_eq!(books_list.data.len(), 2);
    assert_eq!(books_list.total, 2);
}

#[tokio::test]
async fn test_list_books_filtered_by_series_id() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series1 = SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    let book1 = create_test_book_model(series1.id, library.id, "/s1b1.cbz", "s1b1.cbz", None);
    let book2 = create_test_book_model(series1.id, library.id, "/s1b2.cbz", "s1b2.cbz", None);
    let book3 = create_test_book_model(series2.id, library.id, "/s2b1.cbz", "s2b1.cbz", None);
    BookRepository::create(&db, &book1, None).await.unwrap();
    BookRepository::create(&db, &book2, None).await.unwrap();
    BookRepository::create(&db, &book3, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by series1 ID
    let request_body = BookListRequest {
        condition: Some(BookCondition::SeriesId {
            series_id: UuidOperator::Is { value: series1.id },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/books/list", &request_body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books_list = response.unwrap();
    assert_eq!(books_list.data.len(), 2);
    assert!(books_list.data.iter().all(|b| b.series_id == series1.id));
}

#[tokio::test]
async fn test_list_books_filtered_by_library_id() {
    let (db, _temp_dir) = setup_test_db().await;

    let library1 = LibraryRepository::create(&db, "Library 1", "/lib1", ScanningStrategy::Default)
        .await
        .unwrap();
    let library2 = LibraryRepository::create(&db, "Library 2", "/lib2", ScanningStrategy::Default)
        .await
        .unwrap();
    let series1 = SeriesRepository::create(&db, library1.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library2.id, "Series 2", None)
        .await
        .unwrap();

    let book1 = create_test_book_model(series1.id, library1.id, "/l1b1.cbz", "l1b1.cbz", None);
    let book2 = create_test_book_model(series1.id, library1.id, "/l1b2.cbz", "l1b2.cbz", None);
    let book3 = create_test_book_model(series2.id, library2.id, "/l2b1.cbz", "l2b1.cbz", None);
    BookRepository::create(&db, &book1, None).await.unwrap();
    BookRepository::create(&db, &book2, None).await.unwrap();
    BookRepository::create(&db, &book3, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by library1 ID
    let request_body = BookListRequest {
        condition: Some(BookCondition::LibraryId {
            library_id: UuidOperator::Is { value: library1.id },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/books/list", &request_body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books_list = response.unwrap();
    assert_eq!(books_list.data.len(), 2);
}

#[tokio::test]
async fn test_list_books_filtered_by_title() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();

    // Create books with metadata (title is now in book_metadata table)
    create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        Some("Chapter 1".to_string()),
    )
    .await;
    create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        Some("Chapter 2".to_string()),
    )
    .await;
    create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "/book3.cbz",
        "book3.cbz",
        Some("Volume 1".to_string()),
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by title containing "Chapter"
    let request_body = BookListRequest {
        condition: Some(BookCondition::Title {
            title: FieldOperator::Contains {
                value: "Chapter".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/books/list", &request_body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books_list = response.unwrap();
    assert_eq!(books_list.data.len(), 2); // Chapter 1 and Chapter 2
}

#[tokio::test]
async fn test_list_books_filtered_all_of() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series1 = SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    // Create books with metadata (title is now in book_metadata table)
    create_test_book_with_metadata(
        &db,
        series1.id,
        library.id,
        "/s1ch1.cbz",
        "s1ch1.cbz",
        Some("Chapter 1".to_string()),
    )
    .await;
    create_test_book_with_metadata(
        &db,
        series1.id,
        library.id,
        "/s1ch2.cbz",
        "s1ch2.cbz",
        Some("Volume 1".to_string()),
    )
    .await;
    create_test_book_with_metadata(
        &db,
        series2.id,
        library.id,
        "/s2ch1.cbz",
        "s2ch1.cbz",
        Some("Chapter 1".to_string()),
    )
    .await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // AllOf: Series1 AND title contains "Chapter" (should only match book1)
    let request_body = BookListRequest {
        condition: Some(BookCondition::AllOf {
            all_of: vec![
                BookCondition::SeriesId {
                    series_id: UuidOperator::Is { value: series1.id },
                },
                BookCondition::Title {
                    title: FieldOperator::Contains {
                        value: "Chapter".to_string(),
                    },
                },
            ],
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/books/list", &request_body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books_list = response.unwrap();
    assert_eq!(books_list.data.len(), 1);
    assert_eq!(books_list.data[0].title, "Chapter 1");
}

#[tokio::test]
async fn test_list_books_filtered_any_of() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series1 = SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();
    let series3 = SeriesRepository::create(&db, library.id, "Series 3", None)
        .await
        .unwrap();

    let book1 = create_test_book_model(series1.id, library.id, "/s1b1.cbz", "s1b1.cbz", None);
    let book2 = create_test_book_model(series2.id, library.id, "/s2b1.cbz", "s2b1.cbz", None);
    let book3 = create_test_book_model(series3.id, library.id, "/s3b1.cbz", "s3b1.cbz", None);
    BookRepository::create(&db, &book1, None).await.unwrap();
    BookRepository::create(&db, &book2, None).await.unwrap();
    BookRepository::create(&db, &book3, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // AnyOf: Series1 OR Series2 (should match book1 and book2)
    let request_body = BookListRequest {
        condition: Some(BookCondition::AnyOf {
            any_of: vec![
                BookCondition::SeriesId {
                    series_id: UuidOperator::Is { value: series1.id },
                },
                BookCondition::SeriesId {
                    series_id: UuidOperator::Is { value: series2.id },
                },
            ],
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/books/list", &request_body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books_list = response.unwrap();
    assert_eq!(books_list.data.len(), 2);
}

#[tokio::test]
async fn test_list_books_filtered_has_error() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();

    let mut book1 = create_test_book_model(series.id, library.id, "/book1.cbz", "book1.cbz", None);
    let book2 = create_test_book_model(series.id, library.id, "/book2.cbz", "book2.cbz", None);
    let mut book3 = create_test_book_model(series.id, library.id, "/book3.cbz", "book3.cbz", None);

    // Set analysis errors on book1 and book3
    book1.analysis_error = Some("Failed to parse".to_string());
    book3.analysis_error = Some("Corrupted file".to_string());

    BookRepository::create(&db, &book1, None).await.unwrap();
    BookRepository::create(&db, &book2, None).await.unwrap();
    BookRepository::create(&db, &book3, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Filter by hasError = true
    let request_body = BookListRequest {
        condition: Some(BookCondition::HasError {
            has_error: BoolOperator::IsTrue,
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/books/list", &request_body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let books_list = response.unwrap();
    assert_eq!(books_list.data.len(), 2); // book1 and book3 have errors

    // Filter by hasError = false
    let request_body = BookListRequest {
        condition: Some(BookCondition::HasError {
            has_error: BoolOperator::IsFalse,
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/books/list", &request_body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let books_list = response.unwrap();
    assert_eq!(books_list.data.len(), 1); // only book2 has no error
}

#[tokio::test]
async fn test_list_books_filtered_pagination() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Series", None)
        .await
        .unwrap();

    // Create 5 books
    for i in 1..=5 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/book{}.cbz", i),
            &format!("book{}.cbz", i),
            None,
        );
        BookRepository::create(&db, &book, None).await.unwrap();
    }

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Request page 0, page_size 2
    let request_body = BookListRequest {
        condition: None,
        page: 0,
        page_size: 2,
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/books/list", &request_body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::OK);
    let page1 = response.unwrap();
    assert_eq!(page1.data.len(), 2);
    assert_eq!(page1.total, 5);
    assert_eq!(page1.page, 0);

    // Request page 1
    let request_body = BookListRequest {
        condition: None,
        page: 1,
        page_size: 2,
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/books/list", &request_body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let page2 = response.unwrap();
    assert_eq!(page2.data.len(), 2);
    assert_eq!(page2.page, 1);
}

// ============================================================================
// ReadStatus Filtering Tests
// ============================================================================

#[tokio::test]
async fn test_list_books_filtered_by_read_status_unread() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::ReadProgressRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create 3 books
    let book1_model = create_test_book_model(
        series.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        Some("Unread Book".to_string()),
    );
    let book1 = BookRepository::create(&db, &book1_model, None)
        .await
        .unwrap();
    let book2_model = create_test_book_model(
        series.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        Some("In Progress Book".to_string()),
    );
    let book2 = BookRepository::create(&db, &book2_model, None)
        .await
        .unwrap();
    let book3_model = create_test_book_model(
        series.id,
        library.id,
        "/book3.cbz",
        "book3.cbz",
        Some("Read Book".to_string()),
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
            admin_user.is_admin,
        )
        .unwrap();

    // Set read progress:
    // - book1: No progress (unread)
    // - book2: In progress (not completed, page > 0)
    ReadProgressRepository::upsert(&db, admin_user.id, book2.id, 5, false)
        .await
        .unwrap();
    // - book3: Completed (read)
    ReadProgressRepository::upsert(&db, admin_user.id, book3.id, 10, true)
        .await
        .unwrap();

    let app = create_test_router(state).await;

    // Filter for unread books
    let request_body = BookListRequest {
        condition: Some(BookCondition::ReadStatus {
            read_status: FieldOperator::Is {
                value: "unread".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/books/list", &request_body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 1);
    assert_eq!(book_list.data[0].id, book1.id);
}

#[tokio::test]
async fn test_list_books_filtered_by_read_status_in_progress() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::ReadProgressRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book1_model = create_test_book_model(
        series.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        Some("Unread Book".to_string()),
    );
    let _book1 = BookRepository::create(&db, &book1_model, None)
        .await
        .unwrap();
    let book2_model = create_test_book_model(
        series.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        Some("In Progress Book".to_string()),
    );
    let book2 = BookRepository::create(&db, &book2_model, None)
        .await
        .unwrap();
    let book3_model = create_test_book_model(
        series.id,
        library.id,
        "/book3.cbz",
        "book3.cbz",
        Some("Read Book".to_string()),
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
            admin_user.is_admin,
        )
        .unwrap();

    // book2: In progress
    ReadProgressRepository::upsert(&db, admin_user.id, book2.id, 5, false)
        .await
        .unwrap();
    // book3: Completed
    ReadProgressRepository::upsert(&db, admin_user.id, book3.id, 10, true)
        .await
        .unwrap();

    let app = create_test_router(state).await;

    // Filter for in_progress books
    let request_body = BookListRequest {
        condition: Some(BookCondition::ReadStatus {
            read_status: FieldOperator::Is {
                value: "in_progress".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/books/list", &request_body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 1);
    assert_eq!(book_list.data[0].id, book2.id);
}

#[tokio::test]
async fn test_list_books_filtered_by_read_status_read() {
    let (db, _temp_dir) = setup_test_db().await;

    use codex::db::repositories::ReadProgressRepository;

    let library = LibraryRepository::create(&db, "Library", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let book1_model = create_test_book_model(
        series.id,
        library.id,
        "/book1.cbz",
        "book1.cbz",
        Some("Unread Book".to_string()),
    );
    let _book1 = BookRepository::create(&db, &book1_model, None)
        .await
        .unwrap();
    let book2_model = create_test_book_model(
        series.id,
        library.id,
        "/book2.cbz",
        "book2.cbz",
        Some("In Progress Book".to_string()),
    );
    let book2 = BookRepository::create(&db, &book2_model, None)
        .await
        .unwrap();
    let book3_model = create_test_book_model(
        series.id,
        library.id,
        "/book3.cbz",
        "book3.cbz",
        Some("Read Book".to_string()),
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
            admin_user.is_admin,
        )
        .unwrap();

    // book2: In progress
    ReadProgressRepository::upsert(&db, admin_user.id, book2.id, 5, false)
        .await
        .unwrap();
    // book3: Completed
    ReadProgressRepository::upsert(&db, admin_user.id, book3.id, 10, true)
        .await
        .unwrap();

    let app = create_test_router(state).await;

    // Filter for read books
    let request_body = BookListRequest {
        condition: Some(BookCondition::ReadStatus {
            read_status: FieldOperator::Is {
                value: "read".to_string(),
            },
        }),
        ..Default::default()
    };
    let request = post_json_request_with_auth("/api/v1/books/list", &request_body, &token);
    let (status, response): (StatusCode, Option<BookListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let book_list = response.unwrap();
    assert_eq!(book_list.data.len(), 1);
    assert_eq!(book_list.data[0].id, book3.id);
}
