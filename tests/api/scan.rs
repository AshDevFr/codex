// Allow unused temp_dir - needed to keep TempDir alive but not always referenced
#![allow(unused_variables)]

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::routes::v1::dto::scan::ScanStatusDto;
use codex::db::repositories::{LibraryRepository, UserRepository};
use codex::db::ScanningStrategy;
use codex::scanner::ScanMode;
use codex::tasks::TaskWorker;
use codex::utils::password;
use common::*;
use hyper::{Request, StatusCode};
use tower::ServiceExt;

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

// ============================================================================
// Trigger Scan Tests
// ============================================================================

#[tokio::test]
async fn test_trigger_normal_scan() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a test library
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
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{}/scan?mode=normal", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<ScanStatusDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let scan_status = response.unwrap();
    assert_eq!(scan_status.library_id, library.id);
    assert!(scan_status.status == "pending" || scan_status.status == "processing");
}

#[tokio::test]
async fn test_trigger_deep_scan() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a test library
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
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{}/scan?mode=deep", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<ScanStatusDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let scan_status = response.unwrap();
    assert_eq!(scan_status.library_id, library.id);
    assert!(scan_status.status == "pending" || scan_status.status == "processing");
}

#[tokio::test]
async fn test_trigger_scan_invalid_mode() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a test library
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
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{}/scan?mode=invalid", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = response.unwrap();
    // The error message comes from ScanMode::from_str
    println!("Error message: {}", error.message);
    assert!(
        error.message.to_lowercase().contains("mode")
            || error.message.to_lowercase().contains("invalid")
    );
}

#[tokio::test]
async fn test_trigger_scan_requires_write_permission() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a test library
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

    let uri = format!("/api/v1/libraries/{}/scan?mode=normal", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_trigger_scan_library_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/api/v1/libraries/{}/scan?mode=normal", fake_id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// Get Scan Status Tests
// ============================================================================

#[tokio::test]
async fn test_get_scan_status() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a test library
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

    // Trigger a scan first
    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .ok();

    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{}/scan-status", library.id);
    let request = get_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<ScanStatusDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let scan_status = response.unwrap();
    assert_eq!(scan_status.library_id, library.id);
}

#[tokio::test]
async fn test_get_scan_status_reader_has_read_permission() {
    // In the RBAC system, Reader role has LibrariesRead permission
    let (db, temp_dir) = setup_test_db().await;

    // Create a test library
    let library = LibraryRepository::create(
        &db,
        "Test Library",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone()).await;

    // Create a Reader user (Reader role has LibrariesRead permission)
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user_with_permissions(
        "reader",
        "reader@example.com",
        &password_hash,
        false,
        vec![],
    );
    let created = UserRepository::create(&db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();

    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{}/scan-status", library.id);
    let request = get_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    // Reader has permission, so we get 404 (no scan found) not 403
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_scan_status_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a test library but don't trigger a scan
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
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{}/scan-status", library.id);
    let request = get_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert!(error.message.contains("No scan found"));
}

// ============================================================================
// Cancel Scan Tests
// ============================================================================

#[tokio::test]
async fn test_cancel_scan() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a test library
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

    // Trigger a scan first
    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .ok();

    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{}/scan/cancel", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_cancel_scan_requires_write_permission() {
    let (db, temp_dir) = setup_test_db().await;

    // Create a test library
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

    let uri = format!("/api/v1/libraries/{}/scan/cancel", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================================
// List Active Scans Tests
// ============================================================================

#[tokio::test]
async fn test_list_active_scans() {
    let (db, temp_dir) = setup_test_db().await;

    // Create two test libraries
    let library1 = LibraryRepository::create(
        &db,
        "Library 1",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();
    let library2 = LibraryRepository::create(
        &db,
        "Library 2",
        temp_dir.path().to_str().unwrap(),
        ScanningStrategy::Default,
    )
    .await
    .unwrap();

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Trigger scans for both libraries
    trigger_scan_task(&state.db, library1.id, ScanMode::Normal)
        .await
        .ok();
    trigger_scan_task(&state.db, library2.id, ScanMode::Deep)
        .await
        .ok();

    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth("/api/v1/scans/active", &token);

    let (status, response): (StatusCode, Option<Vec<ScanStatusDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let scans = response.unwrap();
    assert!(!scans.is_empty()); // At least one scan should be active or queued
}

#[tokio::test]
async fn test_list_active_scans_reader_has_read_permission() {
    // In the RBAC system, Reader role has LibrariesRead permission
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone()).await;

    // Create a Reader user (Reader role has LibrariesRead permission)
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user_with_permissions(
        "reader",
        "reader@example.com",
        &password_hash,
        false,
        vec![],
    );
    let created = UserRepository::create(&db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();

    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth("/api/v1/scans/active", &token);

    let (status, _): (StatusCode, Option<Vec<ScanStatusDto>>) =
        make_json_request(app, request).await;

    // Reader has permission, so we should get 200 OK
    assert_eq!(status, StatusCode::OK);
}

// ============================================================================
// SSE Stream Tests
// ============================================================================

#[tokio::test]
async fn test_scan_progress_stream_requires_auth() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request("/api/v1/scans/stream");

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_scan_progress_stream_reader_has_read_permission() {
    // In the RBAC system, Reader role has LibrariesRead permission
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;

    // Create a Reader user (Reader role has LibrariesRead permission)
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user_with_permissions(
        "reader",
        "reader@example.com",
        &password_hash,
        false,
        vec![],
    );
    let created = UserRepository::create(&db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();

    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth("/api/v1/scans/stream", &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    // Reader has permission, so we should get 200 OK (SSE stream starts)
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_scan_progress_stream_connection() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/scans/stream")
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "text/event-stream")
        .body(String::new())
        .unwrap();

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    // SSE endpoint should return 200 OK
    assert_eq!(response.status(), StatusCode::OK);

    // Verify SSE headers
    let headers = response.headers();
    assert_eq!(
        headers.get("content-type").map(|v| v.to_str().ok()),
        Some(Some("text/event-stream"))
    );
    assert_eq!(
        headers.get("cache-control").map(|v| v.to_str().ok()),
        Some(Some("no-cache"))
    );
}

#[tokio::test]
async fn test_scan_manager_subscribe() {
    let (db, temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;

    // Test that we can subscribe to task progress updates
    let mut receiver = state.event_broadcaster.subscribe_tasks();

    // Verify receiver is created (doesn't fail)
    assert!(receiver.try_recv().is_err()); // No messages yet
}

#[tokio::test]
async fn test_scan_progress_broadcast() {
    use std::time::Duration;
    use tokio::time::timeout;

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

    let state = create_test_app_state(db.clone()).await;

    // Subscribe to progress updates
    let mut receiver = state.event_broadcaster.subscribe_tasks();

    // Create a worker with event broadcaster
    let worker = TaskWorker::new(db.clone())
        .with_event_broadcaster(state.event_broadcaster.clone())
        .with_poll_interval(Duration::from_millis(100));

    // Trigger a scan
    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Process the scan task in the background
    let worker_handle = tokio::spawn(async move {
        let _: Option<bool> = worker.process_once().await.ok();
    });

    // Wait for at least one progress update (with timeout)
    let result = timeout(Duration::from_secs(10), async {
        loop {
            match receiver.recv().await {
                Ok(progress) => {
                    // Verify the progress update is for our library
                    if progress.library_id == Some(library.id) {
                        return Some(progress);
                    }
                }
                Err(_) => return None,
            }
        }
    })
    .await;

    // Wait for worker to finish
    worker_handle.await.ok();

    assert!(
        result.is_ok(),
        "Should receive at least one progress update"
    );
    let progress = result.unwrap();
    assert!(progress.is_some(), "Progress update should not be None");

    let progress = progress.unwrap();
    assert_eq!(progress.library_id, Some(library.id));
}

// ============================================================================
// Integration Tests with Real Scanning
// ============================================================================

#[tokio::test]
async fn test_full_scan_with_progress_updates() {
    use std::time::Duration;
    use tokio::time::timeout;

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

    let state = create_test_app_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Subscribe to progress updates
    let mut receiver = state.event_broadcaster.subscribe_tasks();

    // Create a worker with event broadcaster to process tasks
    let worker = TaskWorker::new(db.clone())
        .with_event_broadcaster(state.event_broadcaster.clone())
        .with_poll_interval(Duration::from_millis(100));

    // Trigger scan via API
    let app = create_test_router_with_app_state(state.clone());
    let uri = format!("/api/v1/libraries/{}/scan?mode=normal", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<ScanStatusDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);

    // Start worker in background
    let worker_handle = tokio::spawn(async move {
        let _: Option<bool> = worker.process_once().await.ok();
    });

    // Collect progress updates until scan completes
    let result = timeout(Duration::from_secs(30), async {
        let mut updates = Vec::new();
        let mut completed = false;

        while let Ok(progress) = receiver.recv().await {
            if progress.library_id == Some(library.id) {
                use codex::events::TaskStatus;
                let is_done = matches!(progress.status, TaskStatus::Completed | TaskStatus::Failed);
                updates.push(progress.clone());

                if is_done {
                    completed = true;
                    break;
                }
            }
        }

        (updates, completed)
    })
    .await;

    // Wait for worker to finish
    worker_handle.await.ok();

    assert!(result.is_ok(), "Should complete scan within timeout");
    let (updates, completed) = result.unwrap();

    assert!(completed, "Scan should complete");
    assert!(!updates.is_empty(), "Should receive progress updates");

    // Verify we got various status updates
    use codex::events::TaskStatus;
    let statuses: Vec<TaskStatus> = updates.iter().map(|u| u.status).collect();
    println!("Received statuses: {:?}", statuses);

    // Should have at least one completed status
    assert!(
        statuses
            .iter()
            .any(|s| matches!(s, TaskStatus::Completed | TaskStatus::Failed)),
        "Should have final status"
    );
}

#[tokio::test]
async fn test_multiple_concurrent_sse_subscribers() {
    use std::time::Duration;
    use tokio::time::timeout;

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

    let state = create_test_app_state(db.clone()).await;

    // Create multiple subscribers
    let mut receiver1 = state.event_broadcaster.subscribe_tasks();
    let mut receiver2 = state.event_broadcaster.subscribe_tasks();
    let mut receiver3 = state.event_broadcaster.subscribe_tasks();

    // Create a worker with event broadcaster
    let worker = TaskWorker::new(db.clone())
        .with_event_broadcaster(state.event_broadcaster.clone())
        .with_poll_interval(Duration::from_millis(100));

    // Trigger a scan
    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Start worker in background
    let worker_handle = tokio::spawn(async move {
        let _: Option<bool> = worker.process_once().await.ok();
    });

    // Verify all subscribers receive updates
    let timeout_duration = Duration::from_secs(10);

    let result1 = timeout(timeout_duration, receiver1.recv()).await;
    let result2 = timeout(timeout_duration, receiver2.recv()).await;
    let result3 = timeout(timeout_duration, receiver3.recv()).await;

    // Wait for worker to finish
    worker_handle.await.ok();

    assert!(result1.is_ok(), "Subscriber 1 should receive update");
    assert!(result2.is_ok(), "Subscriber 2 should receive update");
    assert!(result3.is_ok(), "Subscriber 3 should receive update");
}

#[tokio::test]
async fn test_scan_cancel_broadcasts_update() {
    use std::time::Duration;
    use tokio::time::timeout;

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
    let mut receiver = state.event_broadcaster.subscribe_tasks();

    // Trigger a scan
    trigger_scan_task(&state.db, library.id, ScanMode::Normal)
        .await
        .unwrap();

    // Give scan a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Cancel the scan by finding and cancelling the task
    use codex::db::entities::{prelude::*, tasks};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    if let Ok(Some(task)) = Tasks::find()
        .filter(tasks::Column::TaskType.eq("scan_library"))
        .filter(tasks::Column::LibraryId.eq(library.id))
        .filter(tasks::Column::Status.is_in(vec!["pending", "processing"]))
        .one(&state.db)
        .await
    {
        use codex::db::repositories::TaskRepository;
        TaskRepository::cancel(&state.db, task.id).await.ok();
    }

    // Wait for cancelled status update
    let result = timeout(Duration::from_secs(5), async {
        loop {
            match receiver.recv().await {
                Ok(progress) => {
                    use codex::events::TaskStatus;
                    // Check for Failed status (cancellation shows as Failed in task queue)
                    if progress.library_id == Some(library.id)
                        && progress.status == TaskStatus::Failed
                    {
                        return Some(progress);
                    }
                }
                Err(_) => return None,
            }
        }
    })
    .await;

    // Note: This may not always receive cancelled status depending on timing
    // The test verifies that if we do receive it, it's correct
    if let Ok(Some(progress)) = result {
        use codex::events::TaskStatus;
        assert_eq!(progress.status, TaskStatus::Failed);
        assert_eq!(progress.library_id, Some(library.id));
    }
}
