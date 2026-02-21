//! PDF cache API endpoint tests

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::extractors::AppState;
use codex::api::extractors::auth::UserAuthCache;
use codex::api::routes::v1::dto::{
    PdfCacheCleanupResultDto, PdfCacheStatsDto, TriggerPdfCacheCleanupResponse,
};
use codex::config::{AuthConfig, DatabaseConfig, EmailConfig, FilesConfig, PdfConfig};
use codex::db::repositories::UserRepository;
use codex::events::EventBroadcaster;
use codex::services::email::EmailService;
use codex::services::{
    AuthTrackingService, FileCleanupService, InflightThumbnailTracker, PdfPageCache,
    PluginMetricsService, ReadProgressService, SettingsService, ThumbnailService,
    plugin::PluginManager,
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
use tempfile::TempDir;

/// Create an AppState with PDF cache enabled for testing
async fn create_test_app_state_with_pdf_cache(
    db: DatabaseConnection,
    temp_dir: &TempDir,
) -> Arc<AppState> {
    let jwt_service = Arc::new(JwtService::new(
        "test_secret_key_for_integration_tests".to_string(),
        24,
    ));

    let auth_config = Arc::new(AuthConfig::default());
    let database_config = Arc::new(DatabaseConfig::default());
    let pdf_config = Arc::new(PdfConfig {
        cache_dir: temp_dir.path().to_string_lossy().to_string(),
        cache_rendered_pages: true,
        ..PdfConfig::default()
    });
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
    let read_progress_service = Arc::new(ReadProgressService::new(db.clone()));
    let auth_tracking_service = Arc::new(AuthTrackingService::new(db.clone()));

    // Create PDF page cache with cache ENABLED
    let pdf_page_cache = Arc::new(PdfPageCache::new(temp_dir.path(), true));
    let plugin_manager = Arc::new(PluginManager::with_defaults(Arc::new(db.clone())));
    let plugin_metrics_service = Arc::new(PluginMetricsService::new());

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
        task_metrics_service: None,
        scheduler: None,
        read_progress_service,
        auth_tracking_service,
        pdf_page_cache,
        inflight_thumbnails: Arc::new(InflightThumbnailTracker::new()),
        user_auth_cache: Arc::new(UserAuthCache::new()),
        rate_limiter_service: None,
        plugin_manager,
        plugin_metrics_service,
        oidc_service: None,
        oauth_state_manager: Arc::new(codex::services::user_plugin::OAuthStateManager::new()),
        plugin_file_storage: None,
        scheduler_timezone: "UTC".to_string(),
    })
}

/// Helper to create an admin user and get a token
async fn create_admin_and_token(db: &sea_orm::DatabaseConnection, state: &AppState) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();

    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

/// Helper to create a non-admin user and get a token
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

// ============================================================
// GET /api/v1/admin/pdf-cache/stats tests
// ============================================================

#[tokio::test]
async fn test_get_pdf_cache_stats_empty_cache() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = get_request_with_auth("/api/v1/admin/pdf-cache/stats", &token);
    let (status, response): (StatusCode, Option<PdfCacheStatsDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    assert_eq!(response.total_files, 0);
    assert_eq!(response.total_size_bytes, 0);
    assert_eq!(response.book_count, 0);
    assert!(response.oldest_file_age_days.is_none());
    assert!(response.cache_enabled);
}

#[tokio::test]
async fn test_get_pdf_cache_stats_with_data() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;

    // Add some cached pages
    let book_id = uuid::Uuid::new_v4();
    state
        .pdf_page_cache
        .set(book_id, 1, 150, b"page data 1")
        .await
        .unwrap();
    state
        .pdf_page_cache
        .set(book_id, 2, 150, b"page data 2")
        .await
        .unwrap();

    let app = create_test_router_with_app_state(state.clone());
    let request = get_request_with_auth("/api/v1/admin/pdf-cache/stats", &token);
    let (status, response): (StatusCode, Option<PdfCacheStatsDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    assert_eq!(response.total_files, 2);
    assert_eq!(response.book_count, 1);
    assert!(response.cache_enabled);
}

#[tokio::test]
async fn test_get_pdf_cache_stats_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = get_request("/api/v1/admin/pdf-cache/stats");
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_pdf_cache_stats_requires_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_regular_user_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = get_request_with_auth("/api/v1/admin/pdf-cache/stats", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================
// POST /api/v1/admin/pdf-cache/cleanup tests
// ============================================================

#[tokio::test]
async fn test_trigger_pdf_cache_cleanup() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = post_request_with_auth("/api/v1/admin/pdf-cache/cleanup", &token);
    let (status, response): (StatusCode, Option<TriggerPdfCacheCleanupResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    assert!(!response.task_id.is_nil());
    assert!(response.message.contains("queued"));
    // Default max_age_days should be 30
    assert_eq!(response.max_age_days, 30);
}

#[tokio::test]
async fn test_trigger_pdf_cache_cleanup_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = hyper::Request::builder()
        .method(hyper::Method::POST)
        .uri("/api/v1/admin/pdf-cache/cleanup")
        .header("content-type", "application/json")
        .body(String::new())
        .unwrap();

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_trigger_pdf_cache_cleanup_requires_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_regular_user_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = post_request_with_auth("/api/v1/admin/pdf-cache/cleanup", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================
// DELETE /api/v1/admin/pdf-cache tests
// ============================================================

#[tokio::test]
async fn test_clear_pdf_cache_empty() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = delete_request_with_auth("/api/v1/admin/pdf-cache", &token);
    let (status, response): (StatusCode, Option<PdfCacheCleanupResultDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    assert_eq!(response.files_deleted, 0);
    assert_eq!(response.bytes_reclaimed, 0);
}

#[tokio::test]
async fn test_clear_pdf_cache_with_data() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;

    // Add some cached pages
    let book_id = uuid::Uuid::new_v4();
    state
        .pdf_page_cache
        .set(book_id, 1, 150, b"page data 1")
        .await
        .unwrap();
    state
        .pdf_page_cache
        .set(book_id, 2, 150, b"page data 2")
        .await
        .unwrap();

    let app = create_test_router_with_app_state(state.clone());
    let request = delete_request_with_auth("/api/v1/admin/pdf-cache", &token);
    let (status, response): (StatusCode, Option<PdfCacheCleanupResultDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    assert_eq!(response.files_deleted, 2);
    assert!(response.bytes_reclaimed > 0);

    // Verify cache is empty
    let stats = state.pdf_page_cache.get_total_stats().await.unwrap();
    assert_eq!(stats.total_files, 0);
}

#[tokio::test]
async fn test_clear_pdf_cache_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = hyper::Request::builder()
        .method(hyper::Method::DELETE)
        .uri("/api/v1/admin/pdf-cache")
        .header("content-type", "application/json")
        .body(String::new())
        .unwrap();

    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_clear_pdf_cache_requires_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_regular_user_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = delete_request_with_auth("/api/v1/admin/pdf-cache", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}
