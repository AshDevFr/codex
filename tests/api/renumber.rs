// Allow unused temp_dir - needed to keep TempDir alive but not always referenced
#![allow(unused_variables)]

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::read_progress::BulkTaskResponse;
use codex::api::routes::v1::handlers::task_queue::CreateTaskResponse;
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    LibraryRepository, SeriesRepository, TaskRepository, UserRepository,
};
use codex::scanner::ScanMode;
use codex::tasks::TaskWorker;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use serde_json::json;
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
        .generate_token(created.id, created.username.clone(), created.get_role())
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
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

// Helper to scan a library and process all tasks
async fn scan_and_process(db: &sea_orm::DatabaseConnection, library_id: uuid::Uuid) {
    trigger_scan_task(db, library_id, ScanMode::Normal)
        .await
        .unwrap();

    let worker = TaskWorker::new(db.clone()).with_poll_interval(Duration::from_millis(100));

    // Process scan task first
    worker.process_once().await.ok();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Process all remaining tasks (analysis, etc.)
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
// Single Series Renumber Tests
// ============================================================================

#[tokio::test]
async fn test_renumber_series_success() {
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

    // Scan and process to create series and books with numbers
    scan_and_process(&db, library.id).await;

    let series_list = SeriesRepository::list_by_library(&db, library.id)
        .await
        .unwrap();

    if series_list.is_empty() {
        return;
    }

    let series = &series_list[0];

    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/series/{}/renumber", series.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<CreateTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    // Verify we got a valid task_id back
    assert!(!result.task_id.is_nil(), "task_id should not be nil");

    // Verify the task exists in the database
    let task = TaskRepository::get_by_id(&db, result.task_id)
        .await
        .unwrap();
    assert!(task.is_some(), "Task should exist in the database");
    let task = task.unwrap();
    assert_eq!(task.task_type, "renumber_series");
    assert_eq!(task.series_id, Some(series.id));
}

#[tokio::test]
async fn test_renumber_series_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/api/v1/series/{}/renumber", fake_id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_renumber_series_requires_write_permission() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/api/v1/series/{}/renumber", fake_id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_renumber_series_requires_authentication() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/api/v1/series/{}/renumber", fake_id);
    let request = post_request(&uri);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Bulk Renumber Tests
// ============================================================================

#[tokio::test]
async fn test_bulk_renumber_series_success() {
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

    scan_and_process(&db, library.id).await;

    let series_list = SeriesRepository::list_by_library(&db, library.id)
        .await
        .unwrap();

    if series_list.is_empty() {
        return;
    }

    let series_ids: Vec<uuid::Uuid> = series_list.iter().map(|s| s.id).collect();

    let app = create_test_router_with_app_state(state);

    let body = json!({ "seriesIds": series_ids });
    let request = post_json_request_with_auth("/api/v1/series/bulk/renumber", &body, &token);

    let (status, response): (StatusCode, Option<BulkTaskResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    // Verify we got a valid task_id back
    assert!(!result.task_id.is_nil(), "task_id should not be nil");
    assert!(
        result.message.contains("Renumber task queued"),
        "Message should mention renumber: {}",
        result.message
    );

    // Verify the fan-out task exists in the database
    let task = TaskRepository::get_by_id(&db, result.task_id)
        .await
        .unwrap();
    assert!(task.is_some(), "Task should exist in the database");
    let task = task.unwrap();
    assert_eq!(task.task_type, "renumber_series_batch");
}

#[tokio::test]
async fn test_bulk_renumber_empty_request() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let body = json!({ "seriesIds": [] });
    let request = post_json_request_with_auth("/api/v1/series/bulk/renumber", &body, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    // Empty request now returns 400 BadRequest
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_bulk_renumber_nonexistent_series_enqueues_task() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_ids = vec![uuid::Uuid::new_v4(), uuid::Uuid::new_v4()];
    let body = json!({ "seriesIds": fake_ids });
    let request = post_json_request_with_auth("/api/v1/series/bulk/renumber", &body, &token);

    let (status, response): (StatusCode, Option<BulkTaskResponse>) =
        make_json_request(app, request).await;

    // Task is still enqueued (validation happens at task execution time, not enqueue time)
    assert_eq!(status, StatusCode::OK);
    let result = response.unwrap();
    assert!(!result.task_id.is_nil(), "task_id should not be nil");
}

#[tokio::test]
async fn test_bulk_renumber_requires_write_permission() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let body = json!({ "seriesIds": [uuid::Uuid::new_v4()] });
    let request = post_json_request_with_auth("/api/v1/series/bulk/renumber", &body, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}
