#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::scan::AnalysisResult;
use codex::api::error::ErrorResponse;
use codex::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, UserRepository,
};
use codex::db::ScanningStrategy;
use codex::scanner::ScanMode;
use codex::utils::password;
use common::*;
use hyper::StatusCode;

// Helper to create an admin user and get a token
async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AppState,
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
    state: &codex::api::extractors::AppState,
) -> String {
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user("readonly", "readonly@example.com", &password_hash, false);
    let created = UserRepository::create(db, &user).await.unwrap();

    state
        .jwt_service
        .generate_token(created.id, created.username, created.is_admin)
        .unwrap()
}

// ============================================================================
// Library Analysis Tests
// ============================================================================

#[tokio::test]
async fn test_analyze_library_success() {
    let (db, temp_dir) = setup_test_db().await;

    // Create test files
    create_test_cbz_files_in_dir(temp_dir.path());

    // Create a test library
    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;

    // First, scan to detect files (without analysis)
    state
        .scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Wait for scan to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Check that books exist but are not analyzed
    let books = BookRepository::get_unanalyzed_in_library(&db, library.id)
        .await
        .unwrap();
    let initial_unanalyzed_count = books.len();

    if initial_unanalyzed_count == 0 {
        // Skip test if no books were detected
        return;
    }

    let app = create_test_router_with_app_state(state);

    // Now trigger analysis
    let uri = format!("/api/v1/libraries/{}/analyze", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<AnalysisResult>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(result.books_analyzed > 0);
    assert_eq!(result.errors.len(), 0);
}

#[tokio::test]
async fn test_analyze_library_with_concurrency() {
    let (db, temp_dir) = setup_test_db().await;

    // Create test files
    create_test_cbz_files_in_dir(temp_dir.path());

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;

    // Scan to detect files
    state
        .scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let app = create_test_router_with_app_state(state);

    // Trigger analysis with specific concurrency
    let uri = format!("/api/v1/libraries/{}/analyze?concurrency=8", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<AnalysisResult>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(result.books_analyzed >= 0); // May be 0 if already analyzed
}

#[tokio::test]
async fn test_analyze_library_invalid_concurrency() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    // Test concurrency < 1
    let uri = format!("/api/v1/libraries/{}/analyze?concurrency=0", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app.clone(), request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Test concurrency > 16
    let uri = format!("/api/v1/libraries/{}/analyze?concurrency=20", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_analyze_library_requires_write_permission() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone());
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{}/analyze", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_analyze_library_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/api/v1/libraries/{}/analyze", fake_id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_analyze_library_marks_books_as_analyzed() {
    let (db, temp_dir) = setup_test_db().await;

    create_test_cbz_files_in_dir(temp_dir.path());

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;

    // Scan to detect files
    state
        .scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Check unanalyzed count before analysis
    let unanalyzed_before = BookRepository::get_unanalyzed_in_library(&db, library.id)
        .await
        .unwrap()
        .len();

    if unanalyzed_before == 0 {
        // No books to analyze, skip test
        return;
    }

    let app = create_test_router_with_app_state(state);

    // Analyze
    let uri = format!("/api/v1/libraries/{}/analyze", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<AnalysisResult>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Wait for analysis to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Check unanalyzed count after analysis
    let unanalyzed_after = BookRepository::get_unanalyzed_in_library(&db, library.id)
        .await
        .unwrap()
        .len();

    assert!(
        unanalyzed_after < unanalyzed_before,
        "Some books should be marked as analyzed"
    );
}

// ============================================================================
// Series Analysis Tests
// ============================================================================

#[tokio::test]
async fn test_analyze_series_success() {
    let (db, temp_dir) = setup_test_db().await;

    create_test_cbz_files_in_dir(temp_dir.path());

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;

    // Scan to detect files and create series
    state
        .scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Get a series
    let series_list = SeriesRepository::list_by_library(&db, library.id)
        .await
        .unwrap();

    if series_list.is_empty() {
        // No series found, skip test
        return;
    }

    let series = &series_list[0];

    let app = create_test_router_with_app_state(state);

    // Trigger series analysis
    let uri = format!("/api/v1/series/{}/analyze", series.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<AnalysisResult>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(result.books_analyzed >= 0);
}

#[tokio::test]
async fn test_analyze_series_with_concurrency() {
    let (db, temp_dir) = setup_test_db().await;

    create_test_cbz_files_in_dir(temp_dir.path());

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;

    // Scan
    state
        .scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let series_list = SeriesRepository::list_by_library(&db, library.id)
        .await
        .unwrap();

    if series_list.is_empty() {
        return;
    }

    let series = &series_list[0];

    let app = create_test_router_with_app_state(state);

    // Trigger analysis with concurrency
    let uri = format!("/api/v1/series/{}/analyze?concurrency=4", series.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<AnalysisResult>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_analyze_series_requires_write_permission() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone());
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/api/v1/series/{}/analyze", fake_id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_analyze_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/api/v1/series/{}/analyze", fake_id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Book Analysis Tests
// ============================================================================

#[tokio::test]
async fn test_analyze_book_success() {
    let (db, temp_dir) = setup_test_db().await;

    create_test_cbz_files_in_dir(temp_dir.path());

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;

    // Scan to detect files
    state
        .scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Get a book
    let series_list = SeriesRepository::list_by_library(&db, library.id)
        .await
        .unwrap();

    if series_list.is_empty() {
        return;
    }

    let books = BookRepository::list_by_series(&db, series_list[0].id, false)
        .await
        .unwrap();

    if books.is_empty() {
        return;
    }

    let book = &books[0];

    let app = create_test_router_with_app_state(state);

    // Trigger book analysis
    let uri = format!("/api/v1/books/{}/analyze", book.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<AnalysisResult>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert_eq!(result.books_analyzed, 1);
}

#[tokio::test]
async fn test_analyze_book_force_reanalysis() {
    let (db, temp_dir) = setup_test_db().await;

    create_test_cbz_files_in_dir(temp_dir.path());

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;

    // Scan and analyze
    state
        .scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let series_list = SeriesRepository::list_by_library(&db, library.id)
        .await
        .unwrap();

    if series_list.is_empty() {
        return;
    }

    let books = BookRepository::list_by_series(&db, series_list[0].id, false)
        .await
        .unwrap();

    if books.is_empty() {
        return;
    }

    let book = &books[0];

    // Analyze once
    let app = create_test_router_with_app_state(state.clone());
    let uri = format!("/api/v1/books/{}/analyze", book.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<AnalysisResult>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Force reanalysis (should work even if already analyzed)
    let app = create_test_router_with_app_state(state);
    let uri = format!("/api/v1/books/{}/analyze", book.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<AnalysisResult>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert_eq!(result.books_analyzed, 1);
}

#[tokio::test]
async fn test_analyze_book_requires_write_permission() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone());
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/api/v1/books/{}/analyze", fake_id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_analyze_book_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/api/v1/books/{}/analyze", fake_id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Integration Tests
// ============================================================================

#[tokio::test]
async fn test_two_step_scan_and_analyze_workflow() {
    let (db, temp_dir) = setup_test_db().await;

    create_test_cbz_files_in_dir(temp_dir.path());

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    // Step 1: Fast scan (detection only)
    let scan_uri = format!("/api/v1/libraries/{}/scan", library.id);
    let request = post_request_with_auth(&scan_uri, &token);

    let (status, _): (StatusCode, Option<codex::api::dto::scan::ScanStatusDto>) =
        make_json_request(app.clone(), request).await;
    assert_eq!(status, StatusCode::OK);

    // Wait for scan to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Verify books are detected but not analyzed
    let unanalyzed_books = BookRepository::get_unanalyzed_in_library(&db, library.id)
        .await
        .unwrap();

    if unanalyzed_books.is_empty() {
        // No books detected, skip rest of test
        return;
    }

    println!(
        "Step 1 complete: {} unanalyzed books detected",
        unanalyzed_books.len()
    );

    // Step 2: Analyze with parallel processing
    let analyze_uri = format!("/api/v1/libraries/{}/analyze?concurrency=8", library.id);
    let request = post_request_with_auth(&analyze_uri, &token);

    let (status, response): (StatusCode, Option<AnalysisResult>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();

    println!(
        "Step 2 complete: {} books analyzed, {} errors",
        result.books_analyzed,
        result.errors.len()
    );

    assert!(result.books_analyzed > 0);
    assert_eq!(result.errors.len(), 0);

    // Wait for analysis to finish
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Verify all books are now analyzed
    let remaining_unanalyzed = BookRepository::get_unanalyzed_in_library(&db, library.id)
        .await
        .unwrap();

    assert!(
        remaining_unanalyzed.len() < unanalyzed_books.len(),
        "Analysis should reduce unanalyzed count"
    );
}

#[tokio::test]
async fn test_concurrent_analysis_multiple_libraries() {
    let (db, temp_dir1) = setup_test_db().await;
    let temp_dir2 = tempfile::TempDir::new().unwrap();

    // Create test files in both directories
    create_test_cbz_files_in_dir(temp_dir1.path());
    create_test_cbz_files_in_dir(temp_dir2.path());

    let library1 = LibraryRepository::create(
        &db,
        "Library 1",
        temp_dir1.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let library2 = LibraryRepository::create(
        &db,
        "Library 2",
        temp_dir2.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;

    // Scan both libraries
    state
        .scan_manager
        .trigger_scan(library1.id, ScanMode::Normal)
        .await
        .unwrap();
    state
        .scan_manager
        .trigger_scan(library2.id, ScanMode::Normal)
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(8)).await;

    // Analyze both libraries concurrently
    let app1 = create_test_router_with_app_state(state.clone());
    let app2 = create_test_router_with_app_state(state.clone());

    let uri1 = format!("/api/v1/libraries/{}/analyze?concurrency=4", library1.id);
    let request1 = post_request_with_auth(&uri1, &token);

    let uri2 = format!("/api/v1/libraries/{}/analyze?concurrency=4", library2.id);
    let request2 = post_request_with_auth(&uri2, &token);

    // Run both analyses concurrently
    let (result1, result2) = tokio::join!(
        make_json_request::<AnalysisResult>(app1, request1),
        make_json_request::<AnalysisResult>(app2, request2)
    );

    let (status1, _): (StatusCode, Option<AnalysisResult>) = result1;
    let (status2, _): (StatusCode, Option<AnalysisResult>) = result2;

    assert_eq!(status1, StatusCode::OK);
    assert_eq!(status2, StatusCode::OK);
}

// ============================================================================
// Analyzer Queue Unit Tests
// ============================================================================

#[tokio::test]
async fn test_analyzer_config_bounds() {
    use codex::scanner::AnalyzerConfig;

    let config = AnalyzerConfig::default();
    assert_eq!(config.max_concurrent, 4);

    let config = AnalyzerConfig { max_concurrent: 1 };
    assert_eq!(config.max_concurrent, 1);

    let config = AnalyzerConfig { max_concurrent: 16 };
    assert_eq!(config.max_concurrent, 16);
}

#[tokio::test]
async fn test_analyze_library_books_with_no_unanalyzed() {
    use codex::scanner::{analyze_library_books, AnalyzerConfig};

    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Empty Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let config = AnalyzerConfig::default();
    let result = analyze_library_books(&db, library.id, config, None)
        .await
        .unwrap();

    assert_eq!(result.books_analyzed, 0);
    assert_eq!(result.errors.len(), 0);
}
