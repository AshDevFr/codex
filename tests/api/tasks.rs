#[path = "../common/mod.rs"]
mod common;

use codex::api::dto::task::{TaskDto, TaskProgressDto};
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
// Task Management Tests
// ============================================================================

#[tokio::test]
async fn test_list_tasks_empty() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state);

    let request = get_request_with_auth("/api/v1/tasks", &token);
    let (status, response): (StatusCode, Option<Vec<TaskDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let tasks = response.unwrap();
    assert_eq!(tasks.len(), 0);
}

#[tokio::test]
async fn test_list_tasks_without_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db);
    let app = create_test_router(state);

    let request = get_request("/api/v1/tasks");
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_list_tasks_with_readonly_user() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone());
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router(state);

    let request = get_request_with_auth("/api/v1/tasks", &token);
    let (status, response): (StatusCode, Option<Vec<TaskDto>>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let tasks = response.unwrap();
    assert_eq!(tasks.len(), 0);
}

#[tokio::test]
async fn test_get_task_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state);

    // Use a random UUID that doesn't exist
    let fake_task_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/tasks/{}", fake_task_id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert_eq!(error.error, "NotFound");
}

#[tokio::test]
async fn test_get_task_invalid_id() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state);

    let request = get_request_with_auth("/api/v1/tasks/invalid-uuid", &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = response.unwrap();
    assert_eq!(error.error, "BadRequest");
}

#[tokio::test]
async fn test_cancel_task_not_found() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state);

    // Use a random UUID that doesn't exist
    let fake_task_id = uuid::Uuid::new_v4();
    let request = post_request_with_auth(&format!("/api/v1/tasks/{}/cancel", fake_task_id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    let error = response.unwrap();
    assert_eq!(error.error, "NotFound");
}

#[tokio::test]
async fn test_cancel_task_without_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db);
    let app = create_test_router(state);

    let fake_task_id = uuid::Uuid::new_v4();
    let request = post_json_request(
        &format!("/api/v1/tasks/{}/cancel", fake_task_id),
        &serde_json::json!({}),
    );
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_cancel_task_with_readonly_user() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone());
    let token = create_readonly_and_token(&db, &state).await;
    let app = create_test_router(state);

    let fake_task_id = uuid::Uuid::new_v4();
    let request = post_request_with_auth(&format!("/api/v1/tasks/{}/cancel", fake_task_id), &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    // Readonly users don't have libraries:write permission
    assert_eq!(status, StatusCode::FORBIDDEN);
    let error = response.unwrap();
    assert_eq!(error.error, "Forbidden");
}

#[tokio::test]
async fn test_cancel_task_invalid_id() {
    let (db, _temp_dir) = setup_test_db().await;

    let state = create_test_auth_state(db.clone());
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router(state);

    let request = post_request_with_auth("/api/v1/tasks/invalid-uuid/cancel", &token);
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = response.unwrap();
    assert_eq!(error.error, "BadRequest");
}

// Note: Testing actual task operations (like triggering a scan and seeing it in the task list)
// would require more complex integration test setup. The scan endpoints already have tests
// in tests/api/scan.rs that verify the scanning functionality. The task endpoints are
// essentially views into the same scan manager state, so the core functionality is already
// tested through the scan endpoint tests.
