#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::scan::AnalysisResult;
use codex::api::error::ErrorResponse;
use codex::api::handlers::task_queue::CreateTaskResponse;
use codex::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, TaskRepository, UserRepository,
};
use codex::db::ScanningStrategy;
use codex::scanner::ScanMode;
use codex::tasks::TaskWorker;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use std::time::Duration;

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

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Scan to detect files and create series
    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
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

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Scan
    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
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

    let state = create_test_app_state(db.clone()).await;
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

    let state = create_test_app_state(db.clone()).await;
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

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Scan to detect files
    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
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

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Scan and analyze
    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
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

    let state = create_test_app_state(db.clone()).await;
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

    let state = create_test_app_state(db.clone()).await;
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

// ============================================================================
// Analyzer Queue Unit Tests
// ============================================================================

// ============================================================================
// Auto-Analysis Integration Tests
// ============================================================================

#[tokio::test]
async fn test_auto_analysis_after_normal_scan() {
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

    // Trigger a normal scan (auto-analysis is now handled by the scan handler)
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Create a worker to process tasks
    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));

    // First, process the scan task (this will queue analysis tasks)
    worker.process_once().await.ok();

    // Wait a moment for task queue updates
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Now process all queued analysis tasks
    loop {
        let stats = TaskRepository::get_stats(&db).await.unwrap();
        if stats.pending == 0 {
            break;
        }
        worker.process_once().await.ok();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // Verify that books were both detected AND analyzed automatically
    let unanalyzed_books = BookRepository::get_unanalyzed_in_library(&db, library.id)
        .await
        .unwrap();

    // All books should be analyzed due to auto-analysis
    assert_eq!(
        unanalyzed_books.len(),
        0,
        "Auto-analysis should have analyzed all books"
    );
}

#[tokio::test]
async fn test_auto_analysis_queues_tasks() {
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

    // Trigger a normal scan (auto-analysis queues tasks after scan)
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Create a worker to process the scan task
    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));

    // Process the scan task (this will queue analysis tasks)
    worker.process_once().await.ok();

    // Wait for task queue updates
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify analysis tasks were queued (books still unanalyzed)
    let unanalyzed_books = BookRepository::get_unanalyzed_in_library(&db, library.id)
        .await
        .unwrap();

    // Books should still be unanalyzed (tasks queued but not processed)
    assert!(
        unanalyzed_books.len() > 0,
        "Books should be unanalyzed with analysis tasks queued"
    );

    // Verify analysis tasks were actually queued
    let stats = TaskRepository::get_stats(&db).await.unwrap();
    assert_eq!(
        stats.pending,
        unanalyzed_books.len() as u64,
        "Should have analysis tasks queued for each unanalyzed book"
    );
}

#[tokio::test]
async fn test_deep_scan_analyzes_all_books() {
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

    // Trigger a DEEP scan (auto-analysis will analyze ALL books, not just unanalyzed)
    trigger_scan_task(&db, library.id, ScanMode::Deep)
        .await
        .unwrap();

    // Create a worker to process tasks
    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));

    // First, process the scan task (this will queue analysis tasks for ALL books)
    worker.process_once().await.ok();

    // Wait a moment for task queue updates
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Now process all queued analysis tasks
    loop {
        let stats = TaskRepository::get_stats(&db).await.unwrap();
        if stats.pending == 0 {
            break;
        }
        worker.process_once().await.ok();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // All books should be analyzed
    let unanalyzed_books = BookRepository::get_unanalyzed_in_library(&db, library.id)
        .await
        .unwrap();

    assert_eq!(
        unanalyzed_books.len(),
        0,
        "Deep scan should analyze all books"
    );
}

#[tokio::test]
async fn test_auto_analysis_processes_all_books() {
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

    // Trigger a normal scan (auto-analysis is handled by the scan handler)
    trigger_scan_task(&db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Create a worker to process tasks
    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));

    // First, process the scan task (this will queue analysis tasks)
    worker.process_once().await.ok();

    // Wait a moment for task queue updates
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Now process all queued analysis tasks
    loop {
        let stats = TaskRepository::get_stats(&db).await.unwrap();
        if stats.pending == 0 {
            break;
        }
        worker.process_once().await.ok();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    let unanalyzed_books = BookRepository::get_unanalyzed_in_library(&db, library.id)
        .await
        .unwrap();

    assert_eq!(
        unanalyzed_books.len(),
        0,
        "Auto-analysis should analyze all books"
    );
}
