#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::scan::{ScanStatusDto, TriggerScanQuery};
use codex::api::error::ErrorResponse;
use codex::db::repositories::{LibraryRepository, UserRepository};
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

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{}/scan?mode=normal", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<ScanStatusDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let scan_status = response.unwrap();
    assert_eq!(scan_status.library_id, library.id);
    assert!(scan_status.status == "queued" || scan_status.status == "running");
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

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{}/scan?mode=deep", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<ScanStatusDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let scan_status = response.unwrap();
    assert_eq!(scan_status.library_id, library.id);
    assert!(scan_status.status == "queued" || scan_status.status == "running");
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

    let state = create_test_app_state(db.clone());
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

    let state = create_test_app_state(db.clone());
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{}/scan?mode=normal", library.id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_trigger_scan_library_not_found() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state);

    let fake_id = uuid::Uuid::new_v4();
    let uri = format!("/api/v1/libraries/{}/scan?mode=normal", fake_id);
    let request = post_request_with_auth(&uri, &token);

    let (status, response): (StatusCode, Option<ErrorResponse>) =
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

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;

    // Trigger a scan first
    state
        .scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
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
async fn test_get_scan_status_requires_read_permission() {
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

    let state = create_test_app_state(db.clone());

    // Create a user with no permissions
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user_with_permissions(
        "noperms",
        "noperms@example.com",
        &password_hash,
        false,
        vec![],
    );
    let created = UserRepository::create(&db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username, created.is_admin)
        .unwrap();

    let app = create_test_router_with_app_state(state);

    let uri = format!("/api/v1/libraries/{}/scan-status", library.id);
    let request = get_request_with_auth(&uri, &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
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

    let state = create_test_app_state(db.clone());
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

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;

    // Trigger a scan first
    state
        .scan_manager
        .trigger_scan(library.id, ScanMode::Normal)
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

    let state = create_test_app_state(db.clone());
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

    let state = create_test_app_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;

    // Trigger scans for both libraries
    state
        .scan_manager
        .trigger_scan(library1.id, ScanMode::Normal)
        .await
        .ok();
    state
        .scan_manager
        .trigger_scan(library2.id, ScanMode::Deep)
        .await
        .ok();

    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth("/api/v1/scans/active", &token);

    let (status, response): (StatusCode, Option<Vec<ScanStatusDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let scans = response.unwrap();
    assert!(scans.len() >= 1); // At least one scan should be active or queued
}

#[tokio::test]
async fn test_list_active_scans_requires_read_permission() {
    let (db, temp_dir) = setup_test_db().await;

    let state = create_test_app_state(db.clone());

    // Create a user with no permissions
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user_with_permissions(
        "noperms",
        "noperms@example.com",
        &password_hash,
        false,
        vec![],
    );
    let created = UserRepository::create(&db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username, created.is_admin)
        .unwrap();

    let app = create_test_router_with_app_state(state);

    let request = get_request_with_auth("/api/v1/scans/active", &token);

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}
