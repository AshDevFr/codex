#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::handlers::task_queue::CreateTaskResponse;
use codex::db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, TaskRepository, UserRepository,
};
use codex::db::ScanningStrategy;
use codex::scanner::ScanMode;
use codex::tasks::TaskWorker;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use serde_json::json;
use std::time::Duration;

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

/// Create a readonly (non-admin) user and get a token
async fn create_readonly_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AppState,
) -> String {
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user("readonly", "readonly@example.com", &password_hash, false);
    let created = UserRepository::create(db, &user).await.unwrap();

    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

/// Process all pending tasks in the queue
async fn process_all_pending_tasks(db: &sea_orm::DatabaseConnection) {
    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));

    loop {
        let stats = TaskRepository::get_stats(db).await.unwrap();
        if stats.pending == 0 {
            break;
        }
        worker.process_once().await.ok();
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

// ============================================================================
// POST /api/v1/books/thumbnails/generate Tests (Book Thumbnails)
// ============================================================================

#[tokio::test]
async fn test_generate_book_thumbnails_all_success() {
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

    // Process scan task
    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify books were created
    let (books, _) = BookRepository::list_all(&db, false, 0, 100).await.unwrap();
    assert!(!books.is_empty(), "Should have books after scan");

    let app = create_test_router_with_app_state(state);

    // Trigger thumbnail generation for all books
    let request_body = json!({});
    let request =
        post_json_request_with_auth("/api/v1/books/thumbnails/generate", &request_body, &token);

    let (status, response): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let task_response = response.unwrap();
    assert!(!task_response.task_id.to_string().is_empty());
}

#[tokio::test]
async fn test_generate_book_thumbnails_with_force() {
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

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();

    let app = create_test_router_with_app_state(state);

    // Trigger with force=true
    let request_body = json!({ "force": true });
    let request =
        post_json_request_with_auth("/api/v1/books/thumbnails/generate", &request_body, &token);

    let (status, response): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.is_some());
}

#[tokio::test]
async fn test_generate_book_thumbnails_requires_write_permission() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request_body = json!({});
    let request =
        post_json_request_with_auth("/api/v1/books/thumbnails/generate", &request_body, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================================
// POST /api/v1/libraries/:library_id/books/thumbnails/generate Tests
// ============================================================================

#[tokio::test]
async fn test_generate_library_book_thumbnails_success() {
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

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let app = create_test_router_with_app_state(state);

    // Trigger thumbnail generation for library books
    let uri = format!("/api/v1/libraries/{}/books/thumbnails/generate", library.id);
    let request_body = json!({});
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, response): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let task_response = response.unwrap();
    assert!(!task_response.task_id.to_string().is_empty());
}

#[tokio::test]
async fn test_generate_library_book_thumbnails_with_force() {
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

    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();

    let app = create_test_router_with_app_state(state);

    // Trigger with force=true
    let uri = format!("/api/v1/libraries/{}/books/thumbnails/generate", library.id);
    let request_body = json!({ "force": true });
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, response): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.is_some());
}

#[tokio::test]
async fn test_generate_library_book_thumbnails_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/api/v1/libraries/{}/books/thumbnails/generate", fake_id);
    let request_body = json!({});
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_generate_library_book_thumbnails_requires_write_permission() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{}/books/thumbnails/generate", library.id);
    let request_body = json!({});
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================================
// POST /api/v1/series/thumbnails/generate Tests (Batch Series Thumbnails)
// ============================================================================

#[tokio::test]
async fn test_generate_series_thumbnails_batch_success() {
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

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let app = create_test_router_with_app_state(state);

    // Trigger batch series thumbnail generation
    let request_body = json!({});
    let request =
        post_json_request_with_auth("/api/v1/series/thumbnails/generate", &request_body, &token);

    let (status, response): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let task_response = response.unwrap();
    assert!(!task_response.task_id.to_string().is_empty());
}

#[tokio::test]
async fn test_generate_series_thumbnails_batch_with_library_id() {
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

    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let app = create_test_router_with_app_state(state);

    // Trigger with library_id scope
    let request_body = json!({ "library_id": library.id.to_string() });
    let request =
        post_json_request_with_auth("/api/v1/series/thumbnails/generate", &request_body, &token);

    let (status, response): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.is_some());
}

#[tokio::test]
async fn test_generate_series_thumbnails_batch_requires_write_permission() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request_body = json!({});
    let request =
        post_json_request_with_auth("/api/v1/series/thumbnails/generate", &request_body, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================================
// POST /api/v1/series/:series_id/thumbnail/generate Tests (Single Series)
// ============================================================================

#[tokio::test]
async fn test_generate_series_thumbnail_single_success() {
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

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Get a series
    let series_list = SeriesRepository::list_by_library(&db, library.id)
        .await
        .unwrap();

    if series_list.is_empty() {
        return; // Skip if no series
    }

    let series = &series_list[0];

    let app = create_test_router_with_app_state(state);

    // Trigger thumbnail generation for single series (singular - uses GenerateSeriesThumbnail)
    let uri = format!("/api/v1/series/{}/thumbnail/generate", series.id);
    let request_body = json!({});
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, response): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let task_response = response.unwrap();
    assert!(!task_response.task_id.to_string().is_empty());
}

#[tokio::test]
async fn test_generate_series_thumbnail_single_with_force() {
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

    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let series_list = SeriesRepository::list_by_library(&db, library.id)
        .await
        .unwrap();

    if series_list.is_empty() {
        return;
    }

    let series = &series_list[0];

    let app = create_test_router_with_app_state(state);

    // Trigger with force=true
    let uri = format!("/api/v1/series/{}/thumbnail/generate", series.id);
    let request_body = json!({ "force": true });
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, response): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.is_some());
}

#[tokio::test]
async fn test_generate_series_thumbnail_single_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/api/v1/series/{}/thumbnail/generate", fake_id);
    let request_body = json!({});
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_generate_series_thumbnail_single_requires_write_permission() {
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

    let state = create_test_app_state(db.clone()).await;
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/series/{}/thumbnail/generate", series.id);
    let request_body = json!({});
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================================
// POST /api/v1/libraries/:library_id/series/thumbnails/generate Tests
// ============================================================================

#[tokio::test]
async fn test_generate_library_series_thumbnails_success() {
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

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let app = create_test_router_with_app_state(state);

    // Trigger thumbnail generation for library series
    let uri = format!(
        "/api/v1/libraries/{}/series/thumbnails/generate",
        library.id
    );
    let request_body = json!({});
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, response): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let task_response = response.unwrap();
    assert!(!task_response.task_id.to_string().is_empty());
}

#[tokio::test]
async fn test_generate_library_series_thumbnails_with_force() {
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

    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();

    let app = create_test_router_with_app_state(state);

    // Trigger with force=true
    let uri = format!(
        "/api/v1/libraries/{}/series/thumbnails/generate",
        library.id
    );
    let request_body = json!({ "force": true });
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, response): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.is_some());
}

#[tokio::test]
async fn test_generate_library_series_thumbnails_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/api/v1/libraries/{}/series/thumbnails/generate", fake_id);
    let request_body = json!({});
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_generate_library_series_thumbnails_requires_write_permission() {
    let (db, temp_dir) = setup_test_db().await;

    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!(
        "/api/v1/libraries/{}/series/thumbnails/generate",
        library.id
    );
    let request_body = json!({});
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================================
// POST /api/v1/books/:book_id/thumbnail/generate Tests
// ============================================================================

#[tokio::test]
async fn test_generate_book_thumbnail_success() {
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

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();
    tokio::time::sleep(Duration::from_millis(100)).await;

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

    // Trigger thumbnail generation for book (singular - uses GenerateThumbnail)
    let uri = format!("/api/v1/books/{}/thumbnail/generate", book.id);
    let request_body = json!({});
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, response): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let task_response = response.unwrap();
    assert!(!task_response.task_id.to_string().is_empty());
}

#[tokio::test]
async fn test_generate_book_thumbnail_with_force() {
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

    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();
    tokio::time::sleep(Duration::from_millis(100)).await;

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

    // Trigger with force=true
    let uri = format!("/api/v1/books/{}/thumbnail/generate", book.id);
    let request_body = json!({ "force": true });
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, response): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.is_some());
}

#[tokio::test]
async fn test_generate_book_thumbnail_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/api/v1/books/{}/thumbnail/generate", fake_id);
    let request_body = json!({});
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_generate_book_thumbnail_requires_write_permission() {
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
    let _admin_token = create_admin_and_token(&db, &state).await;
    let readonly_token = create_readonly_and_token(&db, &state).await;

    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();
    tokio::time::sleep(Duration::from_millis(100)).await;

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

    // Try with readonly user - should fail
    let uri = format!("/api/v1/books/{}/thumbnail/generate", book.id);
    let request_body = json!({});
    let request = post_json_request_with_auth(&uri, &request_body, &readonly_token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================================
// Task Processing Integration Tests
// ============================================================================

#[tokio::test]
async fn test_generate_thumbnails_fan_out() {
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

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Get book count
    let (books, _) = BookRepository::list_by_library(&db, library.id, false, 0, 100)
        .await
        .unwrap();
    let book_count = books.len();

    if book_count == 0 {
        return;
    }

    let app = create_test_router_with_app_state(state);

    // Trigger thumbnail generation for library (should create GenerateThumbnails task)
    let uri = format!("/api/v1/libraries/{}/books/thumbnails/generate", library.id);
    let request_body = json!({ "force": true });
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, _): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Process the GenerateThumbnails task (should fan out to individual GenerateThumbnail tasks)
    worker.process_once().await.ok();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check that GenerateThumbnail tasks were created
    let stats = TaskRepository::get_stats(&db).await.unwrap();

    // Should have individual thumbnail tasks pending
    assert!(
        stats.pending > 0,
        "Should have pending GenerateThumbnail tasks after fan-out"
    );
}

#[tokio::test]
async fn test_generate_thumbnail_single_task() {
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

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));
    worker.process_once().await.ok();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Process all analysis tasks
    process_all_pending_tasks(&db).await;

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

    // Clear any existing tasks
    TaskRepository::nuke_all_tasks(&db).await.unwrap();

    // Trigger single book thumbnail (uses GenerateThumbnail task)
    let uri = format!("/api/v1/books/{}/thumbnail/generate", book.id);
    let request_body = json!({ "force": true });
    let request = post_json_request_with_auth(&uri, &request_body, &token);

    let (status, response): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert!(response.is_some());

    // Get the task and verify it's a GenerateThumbnail task (singular)
    let stats = TaskRepository::get_stats(&db).await.unwrap();
    assert_eq!(stats.pending, 1, "Should have exactly one pending task");

    // Verify task type
    if let Some(by_type) = stats.by_type.get("generate_thumbnail") {
        assert_eq!(
            by_type.pending, 1,
            "Should have one pending generate_thumbnail task"
        );
    } else {
        panic!("Should have generate_thumbnail task type in stats");
    }
}
