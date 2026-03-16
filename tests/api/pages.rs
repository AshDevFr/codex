#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    BookRepository, LibraryRepository, PageRepository, ReadProgressRepository, SeriesRepository,
    UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use tempfile::TempDir;

// ============================================================================
// Helper Functions
// ============================================================================

/// Create an admin user and get a token
async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
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

/// Create a test book model with configurable analyzed state
fn create_test_book_model(
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
    path: &str,
    name: &str,
    format: &str,
    analyzed: bool,
    page_count: i32,
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
        format: format.to_string(),
        page_count,
        deleted: false,
        analyzed,
        analysis_error: None,
        analysis_errors: None,
        modified_at: Utc::now(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        thumbnail_path: None,
        thumbnail_generated_at: None,
        koreader_hash: None,
        epub_positions: None,
        epub_spine_items: None,
    }
}

/// Create a test page model
fn create_test_page_model(
    book_id: uuid::Uuid,
    page_number: i32,
    file_name: &str,
    format: &str,
) -> codex::db::entities::pages::Model {
    use chrono::Utc;
    codex::db::entities::pages::Model {
        id: uuid::Uuid::new_v4(),
        book_id,
        page_number,
        file_name: file_name.to_string(),
        format: format.to_string(),
        width: 800,
        height: 1200,
        file_size: 50000,
        created_at: Utc::now(),
    }
}

// ============================================================================
// GET /api/v1/books/{book_id}/pages/{page_number} Tests
// ============================================================================

#[tokio::test]
async fn test_get_page_image_cbz_analyzed_book() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a separate temp dir for the CBZ file
    let cbz_temp_dir = TempDir::new().unwrap();
    let cbz_path = create_test_cbz(&cbz_temp_dir, 5, true);

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        cbz_temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create book with analyzed=true and page metadata
    let book = create_test_book_model(
        series.id,
        library.id,
        cbz_path.to_str().unwrap(),
        "test_comic.cbz",
        "cbz",
        true, // analyzed
        5,    // page_count (matches CBZ)
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    // Create page metadata (format should match the PNG images in the CBZ)
    let page = create_test_page_model(book.id, 1, "page001.png", "png");
    PageRepository::create(&db, &page).await.unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    // Request page 1
    let request = get_request_with_auth(&format!("/api/v1/books/{}/pages/1", book.id), &token);
    let (status, headers, _body) = make_full_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("content-type").unwrap(), "image/png");
}

#[tokio::test]
async fn test_get_page_image_cbz_non_analyzed_book_success() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a separate temp dir for the CBZ file
    let cbz_temp_dir = TempDir::new().unwrap();
    let cbz_path = create_test_cbz(&cbz_temp_dir, 5, true);

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        cbz_temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create book with analyzed=false and NO page metadata
    let book = create_test_book_model(
        series.id,
        library.id,
        cbz_path.to_str().unwrap(),
        "test_comic.cbz",
        "cbz",
        false, // NOT analyzed
        0,     // page_count unknown
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    // Do NOT create page metadata - simulating non-analyzed state

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    // Request page 1 - should succeed by extracting directly from CBZ
    let request = get_request_with_auth(&format!("/api/v1/books/{}/pages/1", book.id), &token);
    let (status, headers, _body) = make_full_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    // Content type should be detected from image data
    let content_type = headers.get("content-type").unwrap().to_str().unwrap();
    assert!(
        content_type.starts_with("image/"),
        "Expected image content type, got: {}",
        content_type
    );
}

#[tokio::test]
async fn test_get_page_image_cbz_non_analyzed_book_page_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a separate temp dir for the CBZ file (5 pages)
    let cbz_temp_dir = TempDir::new().unwrap();
    let cbz_path = create_test_cbz(&cbz_temp_dir, 5, true);

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        cbz_temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create book with analyzed=false
    let book = create_test_book_model(
        series.id,
        library.id,
        cbz_path.to_str().unwrap(),
        "test_comic.cbz",
        "cbz",
        false, // NOT analyzed
        0,     // page_count unknown
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    // Request page 999 - should fail with helpful message (CBZ only has 5 pages)
    let request = get_request_with_auth(&format!("/api/v1/books/{}/pages/999", book.id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert!(
        error.message.contains("not been analyzed yet"),
        "Expected 'not been analyzed yet' message, got: {}",
        error.message
    );
}

#[tokio::test]
async fn test_get_page_image_analyzed_book_page_out_of_range() {
    let (db, _temp_dir) = setup_test_db().await;

    // We don't need actual CBZ files - just need to test the page count validation
    let library =
        LibraryRepository::create(&db, "Test Library", "/fake/path", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create book with analyzed=true and known page count
    let book = create_test_book_model(
        series.id,
        library.id,
        "/fake/path/test_comic.cbz",
        "test_comic.cbz",
        "cbz",
        true, // analyzed
        3,    // page_count is 3
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    // Request page 10 - should fail because book is analyzed and we know it only has 3 pages
    let request = get_request_with_auth(&format!("/api/v1/books/{}/pages/10", book.id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert!(
        error.message.contains("book has 3 pages"),
        "Expected page count message, got: {}",
        error.message
    );
}

#[tokio::test]
async fn test_get_page_image_invalid_page_number() {
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
        "/path/to/book.cbz",
        "book.cbz",
        "cbz",
        true,
        10,
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    // Request page 0 - should fail
    let request = get_request_with_auth(&format!("/api/v1/books/{}/pages/0", book.id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = response.unwrap();
    assert!(error.message.contains("Page number must be >= 1"));
}

#[tokio::test]
async fn test_get_page_image_book_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_book_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/books/{}/pages/1", fake_book_id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert!(error.message.contains("Book not found"));
}

#[tokio::test]
async fn test_get_page_image_without_auth() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state);

    let fake_book_id = uuid::Uuid::new_v4();
    let request = get_request(&format!("/api/v1/books/{}/pages/1", fake_book_id));
    let (status, _body) = make_raw_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Content Type Detection Tests
// ============================================================================

#[tokio::test]
async fn test_get_page_image_detects_content_type_without_metadata() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a separate temp dir for the CBZ file (contains PNG images)
    let cbz_temp_dir = TempDir::new().unwrap();
    let cbz_path = create_test_cbz(&cbz_temp_dir, 5, true);

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        cbz_temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create book without page metadata (non-analyzed)
    let book = create_test_book_model(
        series.id,
        library.id,
        cbz_path.to_str().unwrap(),
        "test_comic.cbz",
        "cbz",
        false,
        0,
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth(&format!("/api/v1/books/{}/pages/1", book.id), &token);
    let (status, headers, _body) = make_full_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Content type should be detected from magic bytes (create_test_cbz creates PNG images)
    let content_type = headers.get("content-type").unwrap().to_str().unwrap();
    assert!(
        content_type == "image/jpeg" || content_type == "image/png",
        "Expected image/jpeg or image/png, got: {}",
        content_type
    );
}

// ============================================================================
// GET /api/v1/books/{book_id}/pages (List Pages) Tests
// ============================================================================

/// Page DTO for deserializing list_book_pages response
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct PageDto {
    id: uuid::Uuid,
    book_id: uuid::Uuid,
    page_number: i32,
    file_name: String,
    file_format: String,
    file_size: i64,
    width: Option<i32>,
    height: Option<i32>,
}

#[tokio::test]
async fn test_list_book_pages_returns_pages_for_analyzed_book() {
    let (db, _temp_dir) = setup_test_db().await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/fake/path", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create an analyzed book
    let book = create_test_book_model(
        series.id,
        library.id,
        "/fake/path/test_comic.cbz",
        "test_comic.cbz",
        "cbz",
        true, // analyzed
        3,    // page_count
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    // Create page metadata for the analyzed book
    for i in 1..=3 {
        let page = create_test_page_model(book.id, i, &format!("page{:03}.png", i), "png");
        PageRepository::create(&db, &page).await.unwrap();
    }

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth(&format!("/api/v1/books/{}/pages", book.id), &token);
    let (status, response): (StatusCode, Option<Vec<PageDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let pages = response.expect("Expected pages response");
    assert_eq!(pages.len(), 3);

    // Verify page data
    assert_eq!(pages[0].page_number, 1);
    assert_eq!(pages[0].book_id, book.id);
    assert_eq!(pages[0].file_format, "png");
    // Width/height come from create_test_page_model (800x1200)
    assert_eq!(pages[0].width, Some(800));
    assert_eq!(pages[0].height, Some(1200));

    // Verify pages are sorted by page_number
    assert!(pages[0].page_number < pages[1].page_number);
    assert!(pages[1].page_number < pages[2].page_number);
}

#[tokio::test]
async fn test_list_book_pages_returns_empty_for_unanalyzed_book() {
    let (db, _temp_dir) = setup_test_db().await;

    let library =
        LibraryRepository::create(&db, "Test Library", "/fake/path", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Create an unanalyzed book
    let book = create_test_book_model(
        series.id,
        library.id,
        "/fake/path/test_comic.cbz",
        "test_comic.cbz",
        "cbz",
        false, // NOT analyzed
        0,     // page_count unknown
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth(&format!("/api/v1/books/{}/pages", book.id), &token);
    let (status, response): (StatusCode, Option<Vec<PageDto>>) =
        make_json_request(app, request).await;

    // Should return 200 with empty array for unanalyzed books
    assert_eq!(status, StatusCode::OK);
    let pages = response.expect("Expected pages response");
    assert!(pages.is_empty(), "Expected empty array for unanalyzed book");
}

#[tokio::test]
async fn test_list_book_pages_returns_not_found_for_nonexistent_book() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_book_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/books/{}/pages", fake_book_id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert!(error.message.contains("Book not found"));
}

#[tokio::test]
async fn test_list_book_pages_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state);

    let fake_book_id = uuid::Uuid::new_v4();
    let request = get_request(&format!("/api/v1/books/{}/pages", fake_book_id));
    let (status, _body) = make_raw_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Reading Progress Tracking Tests
// ============================================================================

#[tokio::test]
async fn test_v1_page_fetch_does_not_record_progress() {
    let (db, _temp_dir) = setup_test_db().await;

    let cbz_temp_dir = TempDir::new().unwrap();
    let cbz_path = create_test_cbz(&cbz_temp_dir, 5, true);

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        cbz_temp_dir.path().to_str().unwrap(),
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
        cbz_path.to_str().unwrap(),
        "test_comic.cbz",
        "cbz",
        true,
        5,
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.get_role(),
        )
        .unwrap();

    // Fetch multiple pages via v1 API (simulating frontend preloading)
    for page in 1..=3 {
        let app = create_test_router_with_app_state(state.clone());
        let request =
            get_request_with_auth(&format!("/api/v1/books/{}/pages/{}", book.id, page), &token);
        let (status, _, _) = make_full_request(app, request).await;
        assert_eq!(status, StatusCode::OK);
    }

    // Flush the read progress service to ensure any buffered updates are written
    state.read_progress_service.flush().await.unwrap();

    // Verify NO progress was recorded — v1 page handler should not track progress
    let progress = ReadProgressRepository::get_by_user_and_book(&db, created_user.id, book.id)
        .await
        .unwrap();
    assert!(
        progress.is_none(),
        "Expected no progress to be recorded when fetching pages via v1 API, but found: {:?}",
        progress
    );
}

#[tokio::test]
async fn test_opds_page_fetch_records_progress() {
    let (db, _temp_dir) = setup_test_db().await;

    let cbz_temp_dir = TempDir::new().unwrap();
    let cbz_path = create_test_cbz(&cbz_temp_dir, 5, true);

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        cbz_temp_dir.path().to_str().unwrap(),
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
        cbz_path.to_str().unwrap(),
        "test_comic.cbz",
        "cbz",
        true,
        5,
    );
    let book = BookRepository::create(&db, &book, None).await.unwrap();

    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_app_state(db.clone()).await;

    // Fetch page via OPDS endpoint using Basic Auth (how OPDS clients authenticate)
    let app = create_test_router_with_app_state(state.clone());
    let request = get_request_with_basic_auth(
        &format!("/opds/books/{}/pages/3", book.id),
        "admin",
        "admin123",
    );
    let (status, _, _) = make_full_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Flush the read progress service to write buffered updates to DB
    state.read_progress_service.flush().await.unwrap();

    // Verify progress WAS recorded by the OPDS page handler
    let progress = ReadProgressRepository::get_by_user_and_book(&db, created_user.id, book.id)
        .await
        .unwrap();
    assert!(
        progress.is_some(),
        "Expected progress to be recorded when fetching pages via OPDS endpoint"
    );
    let progress = progress.unwrap();
    assert_eq!(
        progress.current_page, 3,
        "Expected progress to be at page 3, got {}",
        progress.current_page
    );
}
