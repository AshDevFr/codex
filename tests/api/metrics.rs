#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::metrics::MetricsDto;
use codex::api::error::ErrorResponse;
use codex::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, UserRepository,
};
use codex::db::ScanningStrategy;
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
        .generate_token(created.id, created.username, created.is_admin)
        .unwrap()
}

// Helper to create a readonly user and get a token
async fn create_readonly_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> String {
    use codex::api::permissions::READONLY_PERMISSIONS;

    let password_hash = password::hash_password("user123").unwrap();
    let permissions_vec: Vec<_> = READONLY_PERMISSIONS.iter().cloned().collect();
    let permissions_strings: Vec<String> = permissions_vec
        .iter()
        .map(|p| {
            serde_json::to_string(p)
                .unwrap()
                .trim_matches('"')
                .to_string()
        })
        .collect();
    let user = create_test_user_with_permissions(
        "readonly",
        "readonly@example.com",
        &password_hash,
        false,
        permissions_strings,
    );
    let created = UserRepository::create(db, &user).await.unwrap();

    state
        .jwt_service
        .generate_token(created.id, created.username, created.is_admin)
        .unwrap()
}

// ============================================================================
// Metrics Tests
// ============================================================================

#[tokio::test]
async fn test_get_metrics_with_auth() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create test data
    let library1 = LibraryRepository::create(&db, "Comics", "/path1", ScanningStrategy::Default)
        .await
        .unwrap();
    let library2 = LibraryRepository::create(&db, "Manga", "/path2", ScanningStrategy::Default)
        .await
        .unwrap();

    // Create series in library1
    let series1 = SeriesRepository::create(&db, library1.id, "Series 1")
        .await
        .unwrap();
    let series2 = SeriesRepository::create(&db, library1.id, "Series 2")
        .await
        .unwrap();

    // Create books
    let book1 = create_test_book(
        series1.id,
        "/path1/series1/book1.cbz",
        "book1.cbz",
        "hash1",
        "cbz",
        10,
    );
    let mut book1 = book1;
    book1.file_size = 1000000;
    BookRepository::create(&db, &book1).await.unwrap();

    let book2 = create_test_book(
        series2.id,
        "/path1/series2/book2.cbz",
        "book2.cbz",
        "hash2",
        "cbz",
        20,
    );
    let mut book2 = book2;
    book2.file_size = 2000000;
    BookRepository::create(&db, &book2).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/metrics", &token);
    let (status, response): (StatusCode, Option<MetricsDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metrics = response.unwrap();

    // Verify counts
    assert_eq!(metrics.library_count, 2);
    assert_eq!(metrics.series_count, 2);
    assert_eq!(metrics.book_count, 2);
    assert_eq!(metrics.total_book_size, 3000000); // 1MB + 2MB
    assert_eq!(metrics.user_count, 1); // The admin user we created
    assert!(metrics.database_size >= 0); // Just check it's a valid value
    assert_eq!(metrics.page_count, 0); // No pages created in this test

    // Verify library breakdown
    assert_eq!(metrics.libraries.len(), 2);

    // Find the Comics library metrics
    let comics_metrics = metrics
        .libraries
        .iter()
        .find(|l| l.name == "Comics")
        .expect("Comics library should be in metrics");
    assert_eq!(comics_metrics.series_count, 2);
    assert_eq!(comics_metrics.book_count, 2);
    assert_eq!(comics_metrics.total_size, 3000000);

    // Find the Manga library metrics
    let manga_metrics = metrics
        .libraries
        .iter()
        .find(|l| l.name == "Manga")
        .expect("Manga library should be in metrics");
    assert_eq!(manga_metrics.series_count, 0);
    assert_eq!(manga_metrics.book_count, 0);
    assert_eq!(manga_metrics.total_size, 0);
}

#[tokio::test]
async fn test_get_metrics_without_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = get_request("/api/v1/metrics");
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_get_metrics_with_readonly_user() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a library
    LibraryRepository::create(&db, "Test Library", "/path", ScanningStrategy::Default)
        .await
        .unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/metrics", &token);
    let (status, response): (StatusCode, Option<MetricsDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metrics = response.unwrap();
    assert_eq!(metrics.library_count, 1);
}

#[tokio::test]
async fn test_get_metrics_empty_database() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/metrics", &token);
    let (status, response): (StatusCode, Option<MetricsDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let metrics = response.unwrap();

    // All counts should be 0 or 1 (for the admin user)
    assert_eq!(metrics.library_count, 0);
    assert_eq!(metrics.series_count, 0);
    assert_eq!(metrics.book_count, 0);
    assert_eq!(metrics.total_book_size, 0);
    assert_eq!(metrics.user_count, 1); // Just the admin
    assert_eq!(metrics.page_count, 0);
    assert_eq!(metrics.libraries.len(), 0);
}

#[tokio::test]
async fn test_get_metrics_with_file_sizes() {
    // This test specifically verifies the fix for the bug where
    // the SQL query failed with "ambiguous column name: id" when
    // calculating total book sizes across joined tables
    let (db, _temp_dir) = setup_test_db().await;

    // Create test data
    let library =
        LibraryRepository::create(&db, "Test Library", "/path", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series")
        .await
        .unwrap();

    // Create multiple books with different file sizes
    let book1 = create_test_book(
        series.id,
        "/path/series/book1.cbz",
        "book1.cbz",
        "hash1",
        "cbz",
        10,
    );
    let mut book1 = book1;
    book1.file_size = 5000000; // 5MB
    BookRepository::create(&db, &book1).await.unwrap();

    let book2 = create_test_book(
        series.id,
        "/path/series/book2.cbz",
        "book2.cbz",
        "hash2",
        "cbz",
        15,
    );
    let mut book2 = book2;
    book2.file_size = 10000000; // 10MB
    BookRepository::create(&db, &book2).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/metrics", &token);
    let (status, response): (StatusCode, Option<MetricsDto>) =
        make_json_request(app, request).await;

    // The main assertion: this should return 200 OK, not 500 Internal Server Error
    assert_eq!(status, StatusCode::OK);
    let metrics = response.unwrap();

    // Verify the totals are calculated correctly
    assert_eq!(metrics.book_count, 2);
    assert_eq!(metrics.total_book_size, 15000000); // 5MB + 10MB

    // Verify the library-specific metrics work correctly with the JOIN
    assert_eq!(metrics.libraries.len(), 1);
    let library_metrics = &metrics.libraries[0];
    assert_eq!(library_metrics.book_count, 2);
    assert_eq!(library_metrics.total_size, 15000000);
}

#[tokio::test]
async fn test_get_metrics_postgres() {
    // This test runs against PostgreSQL to verify database-specific behavior
    // PostgreSQL is stricter about ambiguous column names than SQLite
    let Some(db) = setup_test_db_postgres().await else {
        // Skip test if PostgreSQL is not available
        return;
    };

    // Create test data
    let library =
        LibraryRepository::create(&db, "Test Library", "/path", ScanningStrategy::Default)
            .await
            .unwrap();

    let series = SeriesRepository::create(&db, library.id, "Test Series")
        .await
        .unwrap();

    // Create books with file sizes
    let book1 = create_test_book(
        series.id,
        "/path/series/book1.cbz",
        "book1.cbz",
        "hash1",
        "cbz",
        10,
    );
    let mut book1 = book1;
    book1.file_size = 5000000; // 5MB
    BookRepository::create(&db, &book1).await.unwrap();

    let book2 = create_test_book(
        series.id,
        "/path/series/book2.cbz",
        "book2.cbz",
        "hash2",
        "cbz",
        15,
    );
    let mut book2 = book2;
    book2.file_size = 10000000; // 10MB
    BookRepository::create(&db, &book2).await.unwrap();

    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state).await;

    let request = get_request_with_auth("/api/v1/metrics", &token);
    let (status, response): (StatusCode, Option<MetricsDto>) =
        make_json_request(app, request).await;

    // The critical assertion: PostgreSQL would fail with "ambiguous column name: id"
    // if we were using raw SQL instead of properly qualified column references
    assert_eq!(status, StatusCode::OK);
    let metrics = response.unwrap();

    // Verify the aggregations work correctly on PostgreSQL
    assert_eq!(metrics.book_count, 2);
    assert_eq!(metrics.total_book_size, 15000000);

    // Verify the JOIN-based library metrics work on PostgreSQL
    assert_eq!(metrics.libraries.len(), 1);
    let library_metrics = &metrics.libraries[0];
    assert_eq!(library_metrics.book_count, 2);
    assert_eq!(library_metrics.total_size, 15000000);
}
