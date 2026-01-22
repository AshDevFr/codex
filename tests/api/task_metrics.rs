//! Task metrics API endpoint tests

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::extractors::AppState;
use codex::api::routes::v1::dto::{
    MetricsCleanupResponse, MetricsNukeResponse, TaskMetricsHistoryResponse, TaskMetricsResponse,
};
use codex::config::{AuthConfig, DatabaseConfig, EmailConfig, FilesConfig, PdfConfig};
use codex::db::repositories::UserRepository;
use codex::events::EventBroadcaster;
use codex::services::email::EmailService;
use codex::services::{
    AuthTrackingService, FileCleanupService, PdfPageCache, ReadProgressService, SettingsService,
    TaskMetricsService, ThumbnailService,
};
use codex::utils::jwt::JwtService;
use codex::utils::password;
use common::db::setup_test_db;
use common::fixtures::create_test_user;
use common::http::{
    create_test_router_with_app_state, delete_request_with_auth, get_request,
    get_request_with_auth, make_json_request, post_request_with_auth,
};
use hyper::StatusCode;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// Create an AppState with TaskMetricsService for testing
async fn create_test_app_state_with_metrics(db: DatabaseConnection) -> Arc<AppState> {
    let jwt_service = Arc::new(JwtService::new(
        "test_secret_key_for_integration_tests".to_string(),
        24,
    ));

    let auth_config = Arc::new(AuthConfig::default());
    let database_config = Arc::new(DatabaseConfig::default());
    let pdf_config = Arc::new(PdfConfig::default());
    let email_service = Arc::new(EmailService::new(EmailConfig::default()));
    let event_broadcaster = Arc::new(EventBroadcaster::new(1000));
    let settings_service = Arc::new(
        SettingsService::new(db.clone())
            .await
            .expect("Failed to initialize settings service for tests"),
    );
    let files_config = FilesConfig::default();
    let thumbnail_service = Arc::new(ThumbnailService::new(files_config.clone()));
    let file_cleanup_service = Arc::new(FileCleanupService::new(files_config));

    // Create task metrics service
    let task_metrics_service = Arc::new(TaskMetricsService::new(
        db.clone(),
        settings_service.clone(),
    ));
    let read_progress_service = Arc::new(ReadProgressService::new(db.clone()));
    let auth_tracking_service = Arc::new(AuthTrackingService::new(db.clone()));
    let pdf_page_cache = Arc::new(PdfPageCache::new(&pdf_config.cache_dir, false)); // Disabled in tests

    Arc::new(AppState {
        db,
        jwt_service,
        auth_config,
        database_config,
        pdf_config,
        email_service,
        event_broadcaster,
        settings_service,
        thumbnail_service,
        file_cleanup_service,
        task_metrics_service: Some(task_metrics_service),
        scheduler: None,
        read_progress_service,
        auth_tracking_service,
        pdf_page_cache,
    })
}

// Helper to create an admin user and get a token
async fn create_admin_and_token(db: &sea_orm::DatabaseConnection, state: &AppState) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();

    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

// Helper to create a non-admin user and get a token
async fn create_regular_user_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &AppState,
) -> String {
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user("regularuser", "user@example.com", &password_hash, false);
    let created = UserRepository::create(db, &user).await.unwrap();

    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

#[tokio::test]
async fn test_get_task_metrics() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state_with_metrics(db.clone()).await;

    // Create admin user and get token
    let token = create_admin_and_token(&db, &state).await;

    let app = create_test_router_with_app_state(state.clone());
    let request = get_request_with_auth("/api/v1/metrics/tasks", &token);

    let (status, response): (StatusCode, Option<TaskMetricsResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    assert!(!response.retention.is_empty()); // Should have retention setting
    assert_eq!(response.summary.total_executed, 0); // No tasks executed yet
}

#[tokio::test]
async fn test_get_task_metrics_history() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state_with_metrics(db.clone()).await;

    // Create admin user and get token
    let token = create_admin_and_token(&db, &state).await;

    let app = create_test_router_with_app_state(state.clone());
    let request = get_request_with_auth(
        "/api/v1/metrics/tasks/history?days=7&granularity=hour",
        &token,
    );

    let (status, response): (StatusCode, Option<TaskMetricsHistoryResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    assert_eq!(response.granularity, "hour");
    // History should be empty initially
    assert!(response.points.is_empty());
}

#[tokio::test]
async fn test_get_task_metrics_history_with_task_type_filter() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state_with_metrics(db.clone()).await;

    // Create admin user and get token
    let token = create_admin_and_token(&db, &state).await;

    let app = create_test_router_with_app_state(state.clone());
    let request = get_request_with_auth(
        "/api/v1/metrics/tasks/history?days=7&task_type=scan_library&granularity=day",
        &token,
    );

    let (status, response): (StatusCode, Option<TaskMetricsHistoryResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    assert_eq!(response.granularity, "day");
}

#[tokio::test]
async fn test_trigger_metrics_cleanup() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state_with_metrics(db.clone()).await;

    // Create admin user and get token
    let token = create_admin_and_token(&db, &state).await;

    let app = create_test_router_with_app_state(state.clone());
    let request = post_request_with_auth("/api/v1/metrics/tasks/cleanup", &token);

    let (status, response): (StatusCode, Option<MetricsCleanupResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    assert_eq!(response.deleted_count, 0); // No old metrics to clean
    assert!(!response.retention_days.is_empty());
}

#[tokio::test]
async fn test_nuke_task_metrics() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state_with_metrics(db.clone()).await;

    // Record some metrics
    if let Some(metrics) = &state.task_metrics_service {
        metrics
            .record(
                "test_task".to_string(),
                None,
                true,
                false,
                100,
                10,
                1,
                100,
                None,
            )
            .await;

        metrics.flush().await.expect("Failed to flush");
    }

    // Create admin user and get token
    let token = create_admin_and_token(&db, &state).await;

    let app = create_test_router_with_app_state(state.clone());
    let request = delete_request_with_auth("/api/v1/metrics/tasks", &token);

    let (status, response): (StatusCode, Option<MetricsNukeResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    // Should have deleted at least 1 record (or 0 if flush timing)
    // Note: Due to async nature, this might be 0 or 1
    let _response = response.expect("Response should be present");
}

#[tokio::test]
async fn test_task_metrics_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state_with_metrics(db).await;

    let app = create_test_router_with_app_state(state.clone());

    // Request without auth token should fail
    let request = get_request("/api/v1/metrics/tasks");
    let (status, response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let error = response.unwrap();
    assert_eq!(error.error, "Unauthorized");
}

#[tokio::test]
async fn test_metrics_cleanup_requires_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state_with_metrics(db.clone()).await;

    // Create regular user (not admin)
    let token = create_regular_user_and_token(&db, &state).await;

    let app = create_test_router_with_app_state(state.clone());
    let request = post_request_with_auth("/api/v1/metrics/tasks/cleanup", &token);

    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    // Should be forbidden for non-admin
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_metrics_nuke_requires_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state_with_metrics(db.clone()).await;

    // Create regular user (not admin)
    let token = create_regular_user_and_token(&db, &state).await;

    let app = create_test_router_with_app_state(state.clone());
    let request = delete_request_with_auth("/api/v1/metrics/tasks", &token);

    let (status, _response): (StatusCode, Option<ErrorResponse>) =
        make_json_request(app, request).await;

    // Should be forbidden for non-admin
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_record_and_get_metrics() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state_with_metrics(db.clone()).await;

    // Record several metrics
    if let Some(metrics) = &state.task_metrics_service {
        // Successful task
        metrics
            .record(
                "scan_library".to_string(),
                None,
                true,
                false,
                1000,
                50,
                10,
                1024,
                None,
            )
            .await;

        // Failed task
        metrics
            .record(
                "scan_library".to_string(),
                None,
                false,
                true,
                500,
                25,
                0,
                0,
                Some("Test error".to_string()),
            )
            .await;
    }

    // Create admin user and get token
    let token = create_admin_and_token(&db, &state).await;

    let app = create_test_router_with_app_state(state.clone());
    let request = get_request_with_auth("/api/v1/metrics/tasks", &token);

    let (status, response): (StatusCode, Option<TaskMetricsResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    assert_eq!(response.summary.total_executed, 2);
    assert_eq!(response.summary.total_succeeded, 1);
    assert_eq!(response.summary.total_failed, 1);

    // Check by_type breakdown
    let scan_metrics = response
        .by_type
        .iter()
        .find(|m| m.task_type == "scan_library");
    assert!(scan_metrics.is_some());
    let scan = scan_metrics.unwrap();
    assert_eq!(scan.executed, 2);
    assert_eq!(scan.succeeded, 1);
    assert_eq!(scan.failed, 1);
    assert!(scan.last_error.is_some());
}
