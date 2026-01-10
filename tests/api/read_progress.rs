#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::{MarkReadResponse, ReadProgressResponse};
use codex::api::error::ErrorResponse;
use codex::db::repositories::{
    BookRepository, LibraryRepository, ReadProgressRepository, SeriesRepository, UserRepository,
};
use codex::db::ScanningStrategy;
use codex::utils::password;
use common::*;
use hyper::StatusCode;

// Helper to create admin and token
async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> (uuid::Uuid, String) {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username, created.is_admin)
        .unwrap();
    (created.id, token)
}

// Helper to create a test book
fn create_test_book_model(
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
    path: &str,
    name: &str,
    title: Option<String>,
    page_count: i32,
) -> codex::db::entities::books::Model {
    use chrono::Utc;
    codex::db::entities::books::Model {
        id: uuid::Uuid::new_v4(),
        series_id,
        library_id,
        title,
        number: None,
        file_path: path.to_string(),
        file_name: name.to_string(),
        file_size: 1024,
        file_hash: format!("hash_{}", uuid::Uuid::new_v4()),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count,
        deleted: false,
        analyzed: false,
        modified_at: Utc::now(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        thumbnail_path: None,
        thumbnail_generated_at: None,
    }
}

// ============================================================================
// Mark Book as Read/Unread Tests
// ============================================================================

#[tokio::test]
async fn test_mark_book_as_read() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create a test book with 50 pages
    let book = create_test_book_model(
        series.id,
        library.id,
        "/test/book1.cbz",
        "book1.cbz",
        Some("Book 1".to_string()),
        50,
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Mark book as read
    let request = post_request_with_auth(&format!("/api/v1/books/{}/read", book.id), &token);
    let (status, response): (StatusCode, Option<ReadProgressResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let progress = response.unwrap();
    assert_eq!(progress.book_id, book.id);
    assert_eq!(progress.user_id, user_id);
    assert_eq!(progress.current_page, 49); // 0-indexed, so last page is 49
    assert!(progress.completed);
    assert!(progress.completed_at.is_some());
}

#[tokio::test]
async fn test_mark_book_as_unread() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create a test book
    let book = create_test_book_model(
        series.id,
        library.id,
        "/test/book1.cbz",
        "book1.cbz",
        Some("Book 1".to_string()),
        50,
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;

    // Create initial progress
    ReadProgressRepository::upsert(&db, user_id, book.id, 25, false)
        .await
        .unwrap();

    let app = create_test_router(state).await;

    // Mark book as unread
    let request = post_request_with_auth(&format!("/api/v1/books/{}/unread", book.id), &token);
    let (status, _): (StatusCode, Option<String>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify progress is deleted
    let progress = ReadProgressRepository::get_by_user_and_book(&db, user_id, book.id)
        .await
        .unwrap();
    assert!(progress.is_none());
}

#[tokio::test]
async fn test_mark_book_as_read_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let non_existent_id = uuid::Uuid::new_v4();

    // Try to mark non-existent book as read
    let request =
        post_request_with_auth(&format!("/api/v1/books/{}/read", non_existent_id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    // Error message should indicate book not found
    assert!(
        error.message.to_lowercase().contains("book")
            && error.message.to_lowercase().contains("not"),
        "Expected error to mention book not found, got: {}",
        error.message
    );
}

// ============================================================================
// Mark Series as Read/Unread Tests
// ============================================================================

#[tokio::test]
async fn test_mark_series_as_read() {
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
    let mut books = Vec::new();
    for i in 1..=3 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
            50,
        );
        let book = BookRepository::create(&db, &book, None).await.unwrap();
        books.push(book);
    }

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Mark series as read
    let request = post_request_with_auth(&format!("/api/v1/series/{}/read", series.id), &token);
    let (status, response): (StatusCode, Option<MarkReadResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let mark_response = response.unwrap();
    assert_eq!(mark_response.count, 3);
    assert!(mark_response.message.contains("3 books"));

    // Verify all books are marked as read
    for book in books {
        let progress = ReadProgressRepository::get_by_user_and_book(&db, user_id, book.id)
            .await
            .unwrap()
            .unwrap();
        assert!(progress.completed);
        assert_eq!(progress.current_page, 49); // 0-indexed
    }
}

#[tokio::test]
async fn test_mark_series_as_unread() {
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
    let mut books = Vec::new();
    for i in 1..=3 {
        let book = create_test_book_model(
            series.id,
            library.id,
            &format!("/test/book{}.cbz", i),
            &format!("book{}.cbz", i),
            Some(format!("Book {}", i)),
            50,
        );
        let book = BookRepository::create(&db, &book, None).await.unwrap();
        books.push(book);
    }

    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_admin_and_token(&db, &state).await;

    // Create progress for all books
    for book in &books {
        ReadProgressRepository::upsert(&db, user_id, book.id, 25, false)
            .await
            .unwrap();
    }

    let app = create_test_router(state).await;

    // Mark series as unread
    let request = post_request_with_auth(&format!("/api/v1/series/{}/unread", series.id), &token);
    let (status, response): (StatusCode, Option<MarkReadResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let mark_response = response.unwrap();
    assert_eq!(mark_response.count, 3);
    assert!(mark_response.message.contains("3 books"));

    // Verify all progress is deleted
    for book in books {
        let progress = ReadProgressRepository::get_by_user_and_book(&db, user_id, book.id)
            .await
            .unwrap();
        assert!(progress.is_none());
    }
}

#[tokio::test]
async fn test_mark_series_as_read_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let non_existent_id = uuid::Uuid::new_v4();

    // Try to mark non-existent series as read
    let request =
        post_request_with_auth(&format!("/api/v1/series/{}/read", non_existent_id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    // Error message should indicate series not found
    assert!(
        error.message.to_lowercase().contains("series")
            && error.message.to_lowercase().contains("not"),
        "Expected error to mention series not found, got: {}",
        error.message
    );
}

#[tokio::test]
async fn test_mark_empty_series_as_read() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series (but no books)
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Empty Series", None)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let (_user_id, token) = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    // Mark empty series as read
    let request = post_request_with_auth(&format!("/api/v1/series/{}/read", series.id), &token);
    let (status, response): (StatusCode, Option<MarkReadResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let mark_response = response.unwrap();
    assert_eq!(mark_response.count, 0);
    assert!(mark_response.message.contains("No books"));
}

// ============================================================================
// Authorization Tests
// ============================================================================

#[tokio::test]
async fn test_mark_book_as_read_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let book_id = uuid::Uuid::new_v4();

    // Try to mark book as read without auth
    let request = post_request(&format!("/api/v1/books/{}/read", book_id));
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_mark_series_as_read_unauthorized() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let series_id = uuid::Uuid::new_v4();

    // Try to mark series as read without auth
    let request = post_request(&format!("/api/v1/series/{}/read", series_id));
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
