#[path = "../common/mod.rs"]
mod common;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use codex::api::error::ErrorResponse;
use codex::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, UserRepository,
};
use codex::models::ScanningStrategy;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use sea_orm::prelude::Decimal;

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

    let state = create_test_auth_state(db);
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state);

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

    let state = create_test_auth_state(db);
    let app = create_test_router(state);

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
    let state = create_test_auth_state(db);
    let app = create_test_router(state);

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

    let state = create_test_auth_state(db);
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state);

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

    SeriesRepository::create(&db, library.id, "Series 1")
        .await
        .unwrap();
    SeriesRepository::create(&db, library.id, "Series 2")
        .await
        .unwrap();

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("seriesuser", "series@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db);
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state);

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

    let series = SeriesRepository::create(&db, library.id, "Test Series")
        .await
        .unwrap();

    let book = create_test_book_model(series.id, "Test Book #1", "/test/book1.cbz", 1, 25);
    BookRepository::create(&db, &book).await.unwrap();

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("bookuser", "book@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db);
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state);

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

    let state = create_test_auth_state(db);
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state);

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

    let series = SeriesRepository::create(&db, library.id, "Spider-Man")
        .await
        .unwrap();

    let book1 = create_test_book_model(
        series.id,
        "Amazing Spider-Man #1",
        "/test/spiderman1.cbz",
        1,
        20,
    );
    let book2 = create_test_book_model(
        series.id,
        "Spider-Man: Blue",
        "/test/spiderman_blue.cbz",
        2,
        30,
    );
    BookRepository::create(&db, &book1).await.unwrap();
    BookRepository::create(&db, &book2).await.unwrap();

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("searchuser2", "search2@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db);
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state);

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

    let state = create_test_auth_state(db);
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state);

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

    let series = SeriesRepository::create(&db, library.id, "Test Series")
        .await
        .unwrap();

    let book = create_test_book_model(series.id, "Test Book", "/test/book.cbz", 1, 42);
    let created_book = BookRepository::create(&db, &book).await.unwrap();

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("pseuser", "pse@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db);
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state);

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

fn create_test_book_model(
    series_id: uuid::Uuid,
    title: &str,
    file_path: &str,
    number: i32,
    page_count: i32,
) -> codex::db::entities::books::Model {
    use chrono::Utc;
    use uuid::Uuid;

    codex::db::entities::books::Model {
        id: Uuid::new_v4(),
        series_id,
        title: Some(title.to_string()),
        file_path: file_path.to_string(),
        file_name: file_path.split('/').last().unwrap().to_string(),
        file_size: 1024000,
        file_hash: format!("hash_{}", Uuid::new_v4()),
        format: "cbz".to_string(),
        number: Some(Decimal::from(number)),
        page_count,
        deleted: false,
        modified_at: Utc::now(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}
