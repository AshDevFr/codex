#[path = "../common/mod.rs"]
mod common;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use codex::api::error::ErrorResponse;
use codex::db::repositories::{LibraryRepository, SeriesRepository, UserRepository};
use codex::models::ScanningStrategy;
use codex::utils::password;
use common::*;
use hyper::StatusCode;

// ============================================================================
// OPDS Root Catalog Tests
// ============================================================================

#[tokio::test]
async fn test_opds_root_catalog_with_jwt() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("opdsuser", "opds@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.get_role(),
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds")
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response_text = String::from_utf8(body.to_vec()).unwrap();

    // Check for OPDS feed elements
    assert!(response_text.contains("<feed"));
    assert!(response_text.contains("xmlns=\"http://www.w3.org/2005/Atom\""));
    assert!(response_text.contains("<title>Codex OPDS Catalog</title>"));
    assert!(response_text.contains("<link rel=\"self\""));
    assert!(response_text.contains("<link rel=\"start\""));
}

#[tokio::test]
async fn test_opds_root_catalog_with_basic_auth() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create test user
    let password = "password123";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user("basicuser", "basic@example.com", &password_hash, true);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Encode credentials
    let credentials = format!("{}:{}", "basicuser", password);
    let encoded = STANDARD.encode(credentials.as_bytes());

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds")
        .header("Authorization", format!("Basic {}", encoded))
        .body(String::new())
        .unwrap();

    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response_text = String::from_utf8(body.to_vec()).unwrap();
    assert!(response_text.contains("<feed"));
}

#[tokio::test]
async fn test_opds_root_catalog_without_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds")
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

// ============================================================================
// OPDS Libraries Tests
// ============================================================================

#[tokio::test]
async fn test_opds_list_libraries() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create libraries
    LibraryRepository::create(&db, "Comics", "/comics", ScanningStrategy::Default)
        .await
        .unwrap();
    LibraryRepository::create(&db, "Manga", "/manga", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("libuser", "lib@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.get_role(),
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds/libraries")
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response_text = String::from_utf8(body.to_vec()).unwrap();

    // Check for library entries
    assert!(response_text.contains("<title>All Libraries</title>"));
    assert!(response_text.contains("Comics"));
    assert!(response_text.contains("Manga"));
}

// ============================================================================
// OPDS Series Tests
// ============================================================================

#[tokio::test]
async fn test_opds_library_series() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    SeriesRepository::create(&db, library.id, "Series 1", None)
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Series 2", None)
        .await
        .unwrap();

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("seriesuser", "series@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.get_role(),
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri(format!("/opds/libraries/{}", library.id))
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response_text = String::from_utf8(body.to_vec()).unwrap();

    assert!(response_text.contains("Series 1"));
    assert!(response_text.contains("Series 2"));
}

// ============================================================================
// OPDS Books Tests
// ============================================================================

#[tokio::test]
async fn test_opds_series_books_with_thumbnails() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library, series, and books
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let _book = create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "Test Book #1",
        "/test/book1.cbz",
        1,
        25,
    )
    .await;

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("bookuser", "book@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.get_role(),
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri(format!("/opds/series/{}", series.id))
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response_text = String::from_utf8(body.to_vec()).unwrap();

    // Check for book entry
    assert!(response_text.contains("Test Book #1"));

    // Check for PSE namespace
    assert!(response_text.contains("xmlns:pse"));

    // Check for PSE stream link
    assert!(response_text.contains("http://vaemendis.net/opds-pse/stream"));
    assert!(response_text.contains("pse:count=\"25\""));

    // Check for thumbnail links
    assert!(response_text.contains("http://opds-spec.org/image/thumbnail"));
    assert!(response_text.contains("/thumbnail"));
}

// ============================================================================
// OPDS Search Tests
// ============================================================================

#[tokio::test]
async fn test_opds_search_descriptor() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("searchuser", "search@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.get_role(),
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds/search.xml")
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response_text = String::from_utf8(body.to_vec()).unwrap();

    // Check OpenSearch descriptor
    assert!(response_text.contains("<OpenSearchDescription"));
    assert!(response_text.contains("<ShortName>Codex</ShortName>"));
    assert!(response_text.contains("template=\"/opds/search?q={searchTerms}\""));
}

#[tokio::test]
async fn test_opds_search_books() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library, series, and books
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Spider-Man", None)
        .await
        .unwrap();

    let _book1 = create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "Amazing Spider-Man #1",
        "/test/spiderman1.cbz",
        1,
        20,
    )
    .await;
    let _book2 = create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "Spider-Man: Blue",
        "/test/spiderman_blue.cbz",
        2,
        30,
    )
    .await;

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("searchuser2", "search2@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.get_role(),
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds/search?q=Spider")
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response_text = String::from_utf8(body.to_vec()).unwrap();

    // Check for search results
    assert!(response_text.contains("Search Results"));
    assert!(response_text.contains("Spider-Man"));
}

#[tokio::test]
async fn test_opds_search_empty_query() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("searchuser3", "search3@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.get_role(),
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds/search?q=")
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = response.unwrap();
    assert!(error.message.contains("empty"));
}

// ============================================================================
// OPDS PSE (Page Streaming Extension) Tests
// ============================================================================

#[tokio::test]
async fn test_opds_pse_page_feed() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let created_book = create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "Test Book",
        "/test/book.cbz",
        1,
        42,
    )
    .await;

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("pseuser", "pse@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.get_role(),
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri(format!("/opds/books/{}/pages", created_book.id))
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response_text = String::from_utf8(body.to_vec()).unwrap();

    // Check for PSE page feed
    assert!(response_text.contains("<title>Test Book - Pages</title>"));
    assert!(response_text.contains("http://vaemendis.net/opds-pse/page"));

    // Check that all 42 pages are listed
    assert!(response_text.contains("<title>Page 1</title>"));
    assert!(response_text.contains("<title>Page 42</title>"));
}

// ============================================================================
// Helper Functions
// ============================================================================

// Note: title and number are now in book_metadata table, not books table
fn create_test_book_model(
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
    _title: &str, // No longer used - title is in book_metadata
    file_path: &str,
    _number: i32, // No longer used - number is in book_metadata
    page_count: i32,
) -> codex::db::entities::books::Model {
    use chrono::Utc;
    use uuid::Uuid;

    codex::db::entities::books::Model {
        id: Uuid::new_v4(),
        series_id,
        library_id,
        file_path: file_path.to_string(),
        file_name: file_path.split('/').next_back().unwrap().to_string(),
        file_size: 1024000,
        file_hash: format!("hash_{}", Uuid::new_v4()),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count,
        deleted: false,
        analyzed: true, // For OPDS tests, assume books are analyzed
        analysis_error: None,
        analysis_errors: None,
        modified_at: Utc::now(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        thumbnail_path: None,
        thumbnail_generated_at: None,
    }
}

// Helper to create book with metadata (title and number now in book_metadata table)
async fn create_test_book_with_metadata(
    db: &sea_orm::DatabaseConnection,
    series_id: uuid::Uuid,
    library_id: uuid::Uuid,
    title: &str,
    file_path: &str,
    number: i32,
    page_count: i32,
) -> codex::db::entities::books::Model {
    use codex::db::repositories::{BookMetadataRepository, BookRepository};

    let book = create_test_book_model(series_id, library_id, title, file_path, number, page_count);
    let created = BookRepository::create(db, &book, None).await.unwrap();

    // Create metadata with title and number
    BookMetadataRepository::create_with_title_and_number(
        db,
        created.id,
        Some(title.to_string()),
        Some(sea_orm::prelude::Decimal::from(number)),
    )
    .await
    .unwrap();

    created
}
