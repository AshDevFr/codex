#[path = "../common/mod.rs"]
mod common;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use codex::api::dto::opds2::{Opds2Feed, Opds2Link, Publication};
use codex::api::error::ErrorResponse;
use codex::db::repositories::{
    BookRepository, LibraryRepository, ReadProgressRepository, SeriesRepository, UserRepository,
};
use codex::models::ScanningStrategy;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use sea_orm::prelude::Decimal;

// ============================================================================
// OPDS 2.0 Root Catalog Tests
// ============================================================================

#[tokio::test]
async fn test_opds2_root_catalog_with_jwt() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("opds2user", "opds2@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds/v2")
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<Opds2Feed>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let feed = response.unwrap();

    // Check feed structure
    assert_eq!(feed.metadata.title, "Codex OPDS 2.0 Catalog");
    assert!(feed.metadata.subtitle.is_some());
    assert!(!feed.links.is_empty());

    // Check for self link
    let self_link = feed
        .links
        .iter()
        .find(|l| l.rel == Some("self".to_string()));
    assert!(self_link.is_some());
    assert_eq!(
        self_link.unwrap().media_type,
        Some("application/opds+json".to_string())
    );

    // Check navigation links
    assert!(feed.navigation.is_some());
    let nav = feed.navigation.unwrap();
    assert!(!nav.is_empty());

    // Check for libraries link
    let libraries_link = nav
        .iter()
        .find(|l| l.title == Some("All Libraries".to_string()));
    assert!(libraries_link.is_some());
}

#[tokio::test]
async fn test_opds2_root_catalog_with_basic_auth() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create test user
    let password = "password123";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user("basicuser2", "basic2@example.com", &password_hash, true);
    UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Encode credentials
    let credentials = format!("{}:{}", "basicuser2", password);
    let encoded = STANDARD.encode(credentials.as_bytes());

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds/v2")
        .header("Authorization", format!("Basic {}", encoded))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<Opds2Feed>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.is_some());
}

#[tokio::test]
async fn test_opds2_root_catalog_without_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds/v2")
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_opds2_content_type() {
    let (db, _temp_dir) = setup_test_db().await;

    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("ctuser", "ct@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds/v2")
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, body) = make_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    // Verify the response is valid JSON
    let response_text = String::from_utf8(body.to_vec()).unwrap();
    let parsed: Result<Opds2Feed, _> = serde_json::from_str(&response_text);
    assert!(parsed.is_ok());
}

// ============================================================================
// OPDS 2.0 Libraries Tests
// ============================================================================

#[tokio::test]
async fn test_opds2_list_libraries() {
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
    let user = create_test_user("libuser2", "lib2@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds/v2/libraries")
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<Opds2Feed>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let feed = response.unwrap();

    assert_eq!(feed.metadata.title, "All Libraries");
    assert!(feed.navigation.is_some());

    let nav = feed.navigation.unwrap();
    assert_eq!(nav.len(), 2);

    let library_names: Vec<String> = nav.iter().filter_map(|l| l.title.clone()).collect();
    assert!(library_names.contains(&"Comics".to_string()));
    assert!(library_names.contains(&"Manga".to_string()));
}

// ============================================================================
// OPDS 2.0 Series Tests
// ============================================================================

#[tokio::test]
async fn test_opds2_library_series() {
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
    let user = create_test_user("seriesuser2", "series2@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri(format!("/opds/v2/libraries/{}", library.id))
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<Opds2Feed>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let feed = response.unwrap();

    assert!(feed.metadata.title.contains("Series"));
    assert!(feed.navigation.is_some());

    let nav = feed.navigation.unwrap();
    assert_eq!(nav.len(), 2);

    let series_names: Vec<String> = nav.iter().filter_map(|l| l.title.clone()).collect();
    assert!(series_names.contains(&"Series 1".to_string()));
    assert!(series_names.contains(&"Series 2".to_string()));
}

#[tokio::test]
async fn test_opds2_library_series_pagination() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and many series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    for i in 1..=15 {
        SeriesRepository::create(&db, library.id, &format!("Series {}", i), None)
            .await
            .unwrap();
    }

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user(
        "paginationuser",
        "pagination@example.com",
        &password_hash,
        true,
    );
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state).await;

    // Request page 1 with 10 items per page
    let request = hyper::Request::builder()
        .method("GET")
        .uri(format!(
            "/opds/v2/libraries/{}?page=1&page_size=10",
            library.id
        ))
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<Opds2Feed>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let feed = response.unwrap();

    // Check pagination metadata
    assert_eq!(feed.metadata.number_of_items, Some(15));
    assert_eq!(feed.metadata.items_per_page, Some(10));
    assert_eq!(feed.metadata.current_page, Some(1));

    // Check for next page link
    let next_link = feed
        .links
        .iter()
        .find(|l| l.rel == Some("next".to_string()));
    assert!(next_link.is_some());
}

// ============================================================================
// OPDS 2.0 Books Tests
// ============================================================================

#[tokio::test]
async fn test_opds2_series_books() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library, series, and books
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    let _book1 = create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "Test Book #1",
        "/test/book1.cbz",
        1,
        25,
    )
    .await;
    let _book2 = create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "Test Book #2",
        "/test/book2.cbz",
        2,
        30,
    )
    .await;

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("bookuser2", "book2@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri(format!("/opds/v2/series/{}", series.id))
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<Opds2Feed>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let feed = response.unwrap();

    assert!(feed.metadata.title.contains("Books"));
    assert!(feed.publications.is_some());

    let pubs = feed.publications.unwrap();
    assert_eq!(pubs.len(), 2);

    // Check first publication
    let pub1 = pubs.iter().find(|p| p.metadata.title == "Test Book #1");
    assert!(pub1.is_some());
    let pub1 = pub1.unwrap();

    // Check for acquisition link
    let acq_link = pub1
        .links
        .iter()
        .find(|l| l.rel == Some("http://opds-spec.org/acquisition/open-access".to_string()));
    assert!(acq_link.is_some());

    // Check for image
    assert!(!pub1.images.is_empty());

    // Check for series info in belongsTo
    assert!(pub1.metadata.belongs_to.is_some());
    let belongs_to = pub1.metadata.belongs_to.as_ref().unwrap();
    assert!(belongs_to.series.is_some());
    assert_eq!(belongs_to.series.as_ref().unwrap().name, "Test Series");
}

// ============================================================================
// OPDS 2.0 Search Tests
// ============================================================================

#[tokio::test]
async fn test_opds2_search_books() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library, series, and books
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Batman Series", None)
        .await
        .unwrap();

    let _book1 = create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "Batman: Year One #1",
        "/test/batman1.cbz",
        1,
        20,
    )
    .await;
    let _book2 = create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "Batman: Year One #2",
        "/test/batman2.cbz",
        2,
        25,
    )
    .await;

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("searchuser4", "search4@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds/v2/search?query=Batman")
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<Opds2Feed>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let feed = response.unwrap();

    assert!(feed.metadata.title.contains("Search Results"));
    assert!(feed.publications.is_some());

    let pubs = feed.publications.unwrap();
    // Should find series and books
    assert!(!pubs.is_empty());
}

#[tokio::test]
async fn test_opds2_search_empty_query() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("searchuser5", "search5@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds/v2/search?query=")
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
// OPDS 2.0 Recent Additions Tests
// ============================================================================

#[tokio::test]
async fn test_opds2_recent_additions() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library, series, and books
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Recent Series", None)
        .await
        .unwrap();

    let _book = create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "Recent Book",
        "/test/recent.cbz",
        1,
        15,
    )
    .await;

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("recentuser", "recent@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri("/opds/v2/recent")
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<Opds2Feed>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let feed = response.unwrap();

    assert_eq!(feed.metadata.title, "Recent Additions");
    assert!(feed.publications.is_some());

    let pubs = feed.publications.unwrap();
    assert!(!pubs.is_empty());

    // Check that the recent book is in the feed
    let recent_pub = pubs.iter().find(|p| p.metadata.title == "Recent Book");
    assert!(recent_pub.is_some());
}

// ============================================================================
// OPDS 2.0 Not Found Tests
// ============================================================================

#[tokio::test]
async fn test_opds2_library_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("notfounduser", "notfound@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = hyper::Request::builder()
        .method("GET")
        .uri(format!("/opds/v2/libraries/{}", fake_id))
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert!(error.message.contains("not found"));
}

#[tokio::test]
async fn test_opds2_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user(
        "notfounduser2",
        "notfound2@example.com",
        &password_hash,
        true,
    );
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = hyper::Request::builder()
        .method("GET")
        .uri(format!("/opds/v2/series/{}", fake_id))
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert!(error.message.contains("not found"));
}

// ============================================================================
// OPDS 2.0 Reading Progress Tests
// ============================================================================

#[tokio::test]
async fn test_opds2_series_books_with_reading_progress() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library, series, and books
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Progress Series", None)
        .await
        .unwrap();

    let created_book1 = create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "Book With Progress",
        "/test/progress1.cbz",
        1,
        50,
    )
    .await;
    let _book2 = create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "Book Without Progress",
        "/test/progress2.cbz",
        2,
        30,
    )
    .await;

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user("progressuser", "progress@example.com", &password_hash, true);
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // Add reading progress for the first book (page 25 of 50)
    ReadProgressRepository::upsert(&db, created_user.id, created_book1.id, 25, false)
        .await
        .unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri(format!("/opds/v2/series/{}", series.id))
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<Opds2Feed>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let feed = response.unwrap();

    let pubs = feed.publications.unwrap();
    assert_eq!(pubs.len(), 2);

    // Check first book has reading progress
    let pub1 = pubs
        .iter()
        .find(|p| p.metadata.title == "Book With Progress");
    assert!(pub1.is_some());
    let pub1 = pub1.unwrap();
    assert!(pub1.reading_progress.is_some());
    let progress = pub1.reading_progress.as_ref().unwrap();
    assert_eq!(progress.current_page, 25);
    assert_eq!(progress.total_pages, 50);
    assert_eq!(progress.progress_percent, 50.0);
    assert!(!progress.is_completed);

    // Check second book has no reading progress
    let pub2 = pubs
        .iter()
        .find(|p| p.metadata.title == "Book Without Progress");
    assert!(pub2.is_some());
    let pub2 = pub2.unwrap();
    assert!(pub2.reading_progress.is_none());
}

#[tokio::test]
async fn test_opds2_completed_book_progress() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library, series, and book
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Completed Series", None)
        .await
        .unwrap();

    let created_book = create_test_book_with_metadata(
        &db,
        series.id,
        library.id,
        "Completed Book",
        "/test/completed.cbz",
        1,
        40,
    )
    .await;

    // Create test user
    let password_hash = password::hash_password("password").unwrap();
    let user = create_test_user(
        "completeduser",
        "completed@example.com",
        &password_hash,
        true,
    );
    let created_user = UserRepository::create(&db, &user).await.unwrap();

    // Mark book as completed
    ReadProgressRepository::mark_as_read(&db, created_user.id, created_book.id, 40)
        .await
        .unwrap();

    let state = create_test_auth_state(db).await;
    let token = state
        .jwt_service
        .generate_token(
            created_user.id,
            created_user.username.clone(),
            created_user.is_admin,
        )
        .unwrap();

    let app = create_test_router(state).await;

    let request = hyper::Request::builder()
        .method("GET")
        .uri(format!("/opds/v2/series/{}", series.id))
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap();

    let (status, response): (StatusCode, Option<Opds2Feed>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let feed = response.unwrap();

    let pubs = feed.publications.unwrap();
    let pub1 = pubs
        .iter()
        .find(|p| p.metadata.title == "Completed Book")
        .unwrap();

    assert!(pub1.reading_progress.is_some());
    let progress = pub1.reading_progress.as_ref().unwrap();
    assert!(progress.is_completed);
    assert!(progress.last_read_at.is_some());
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
        file_name: file_path.split('/').last().unwrap().to_string(),
        file_size: 1024000,
        file_hash: format!("hash_{}", Uuid::new_v4()),
        partial_hash: String::new(),
        format: "cbz".to_string(),
        page_count,
        deleted: false,
        analyzed: true,
        analysis_error: None,
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
