//! PDF cache API endpoint tests

#[path = "../common/mod.rs"]
mod common;

use codex::api::error::ErrorResponse;
use codex::api::extractors::AppState;
use codex::api::extractors::auth::UserAuthCache;
use codex::api::routes::v1::dto::{
    PdfCacheCleanupResultDto, PdfCacheStatsDto, PdfHandleCacheClearResultDto,
    PdfHandleCacheStatsDto, TriggerPdfCacheCleanupResponse,
};
use codex::config::{
    AuthConfig, DatabaseConfig, EmailConfig, FilesConfig, ObservabilityConfig, PdfConfig,
};
use codex::db::repositories::UserRepository;
use codex::events::EventBroadcaster;
use codex::parsers::pdf::{open_pdf_document, renderer};
use codex::services::email::EmailService;
use codex::services::{
    AuthTrackingService, FileCleanupService, InflightThumbnailTracker, PdfHandleCache,
    PdfPageCache, PluginMetricsService, ReadProgressService, RefreshTokenService, SettingsService,
    ThumbnailService, plugin::PluginManager,
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
    let pdf_handle_cache = Arc::new(PdfHandleCache::new(
        8,
        std::time::Duration::from_secs(60),
        true,
    ));
    let plugin_manager = Arc::new(PluginManager::with_defaults(Arc::new(db.clone())));
    let plugin_metrics_service = Arc::new(PluginMetricsService::new());
    let refresh_token_service = Arc::new(RefreshTokenService::new(db.clone(), 30));

    Arc::new(AppState {
        db,
        jwt_service,
        refresh_token_service,
        auth_config,
        database_config,
        pdf_config,
        observability_config: Arc::new(ObservabilityConfig::default()),
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
        pdf_handle_cache,
        inflight_thumbnails: Arc::new(InflightThumbnailTracker::new()),
        user_auth_cache: Arc::new(UserAuthCache::new()),
        rate_limiter_service: None,
        plugin_manager,
        plugin_metrics_service,
        oidc_service: None,
        oauth_state_manager: Arc::new(codex::services::user_plugin::OAuthStateManager::new()),
        export_storage: None,
        plugin_file_storage: None,
        scheduler_timezone: "UTC".to_string(),
        fuzzy_index: Arc::new(codex::search::FuzzyIndex::empty()),
        app_name: env!("CARGO_PKG_NAME"),
        app_version: env!("CARGO_PKG_VERSION"),
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
// GET /api/v1/admin/pdf-cache (combined stats) tests
// ============================================================

#[tokio::test]
async fn test_get_pdf_cache_stats_empty_cache() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = get_request_with_auth("/api/v1/admin/pdf-cache", &token);
    let (status, response): (StatusCode, Option<PdfCacheStatsDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    // Page cache (empty).
    assert_eq!(response.pages.total_files, 0);
    assert_eq!(response.pages.total_size_bytes, 0);
    assert_eq!(response.pages.book_count, 0);
    assert!(response.pages.oldest_file_age_days.is_none());
    assert!(response.pages.cache_enabled);
    // Handle cache (empty but enabled).
    assert!(response.handles.enabled);
    assert_eq!(response.handles.current_size, 0);
    assert!(response.handles.entries.is_empty());
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
    let request = get_request_with_auth("/api/v1/admin/pdf-cache", &token);
    let (status, response): (StatusCode, Option<PdfCacheStatsDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    assert_eq!(response.pages.total_files, 2);
    assert_eq!(response.pages.book_count, 1);
    assert!(response.pages.cache_enabled);
}

#[tokio::test]
async fn test_get_pdf_cache_stats_legacy_alias() {
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
    assert_eq!(response.pages.total_files, 0);
    assert!(response.handles.enabled);
}

#[tokio::test]
async fn test_get_pdf_cache_stats_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = get_request("/api/v1/admin/pdf-cache");
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

    let request = get_request_with_auth("/api/v1/admin/pdf-cache", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================
// POST /api/v1/admin/pdf-cache/pages/cleanup tests
// ============================================================

#[tokio::test]
async fn test_trigger_pdf_cache_cleanup() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = post_request_with_auth("/api/v1/admin/pdf-cache/pages/cleanup", &token);
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
async fn test_trigger_pdf_cache_cleanup_legacy_alias() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = post_request_with_auth("/api/v1/admin/pdf-cache/cleanup", &token);
    let (status, response): (StatusCode, Option<TriggerPdfCacheCleanupResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert!(!response.expect("response").task_id.is_nil());
}

#[tokio::test]
async fn test_trigger_pdf_cache_cleanup_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = hyper::Request::builder()
        .method(hyper::Method::POST)
        .uri("/api/v1/admin/pdf-cache/pages/cleanup")
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

    let request = post_request_with_auth("/api/v1/admin/pdf-cache/pages/cleanup", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================
// DELETE /api/v1/admin/pdf-cache/pages tests
// ============================================================

#[tokio::test]
async fn test_clear_pdf_cache_empty() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = delete_request_with_auth("/api/v1/admin/pdf-cache/pages", &token);
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
    let request = delete_request_with_auth("/api/v1/admin/pdf-cache/pages", &token);
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
async fn test_clear_pdf_cache_legacy_alias() {
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
}

#[tokio::test]
async fn test_clear_pdf_cache_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = hyper::Request::builder()
        .method(hyper::Method::DELETE)
        .uri("/api/v1/admin/pdf-cache/pages")
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

    let request = delete_request_with_auth("/api/v1/admin/pdf-cache/pages", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

// ============================================================
// Handle cache endpoint tests
// ============================================================

/// Initialise PDFium for this test process. Returns false if the runtime is
/// unavailable, so the calling test can skip cleanly.
fn ensure_pdfium_init() -> bool {
    if renderer::is_initialized() {
        return true;
    }
    renderer::init_pdfium(None).is_ok()
}

/// Populate the handle cache with one resident entry by opening a real
/// PDFium handle. Returns the book id used for the entry.
fn populate_handle_cache(
    state: &AppState,
    temp_dir: &TempDir,
) -> Option<(uuid::Uuid, std::path::PathBuf)> {
    if !ensure_pdfium_init() {
        return None;
    }
    let pdf_path = common::create_text_only_pdf(temp_dir, 1);
    let book_id = uuid::Uuid::new_v4();

    // First call: miss + open.
    {
        let path = pdf_path.clone();
        state
            .pdf_handle_cache
            .get_or_open(book_id, path.clone(), move || open_pdf_document(&path))
            .expect("first open should succeed");
    }
    // Second call: hit.
    {
        let path = pdf_path.clone();
        state
            .pdf_handle_cache
            .get_or_open(book_id, path.clone(), move || open_pdf_document(&path))
            .expect("hit should succeed");
    }
    Some((book_id, pdf_path))
}

#[tokio::test]
async fn test_get_handle_cache_stats_empty() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = get_request_with_auth("/api/v1/admin/pdf-cache/handles", &token);
    let (status, response): (StatusCode, Option<PdfHandleCacheStatsDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    assert!(response.enabled);
    assert_eq!(response.current_size, 0);
    assert!(response.entries.is_empty());
}

#[tokio::test]
async fn test_get_handle_cache_stats_with_data() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let pdf_dir = TempDir::new().expect("Failed to create temp pdf dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;

    let Some((book_id, _)) = populate_handle_cache(&state, &pdf_dir) else {
        eprintln!("Skipping: PDFium not installed");
        return;
    };

    let app = create_test_router_with_app_state(state.clone());
    let request = get_request_with_auth("/api/v1/admin/pdf-cache/handles", &token);
    let (status, response): (StatusCode, Option<PdfHandleCacheStatsDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    assert_eq!(response.current_size, 1);
    assert_eq!(response.entries.len(), 1);
    assert_eq!(response.entries[0].book_id, book_id);
    assert_eq!(response.hits, 1);
    assert_eq!(response.misses, 1);
    assert_eq!(response.opens, 1);
}

#[tokio::test]
async fn test_get_handle_cache_stats_requires_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_regular_user_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = get_request_with_auth("/api/v1/admin/pdf-cache/handles", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_clear_handle_cache_empty_is_noop() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = delete_request_with_auth("/api/v1/admin/pdf-cache/handles", &token);
    let (status, response): (StatusCode, Option<PdfHandleCacheClearResultDto>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.expect("response").handles_closed, 0);
}

#[tokio::test]
async fn test_clear_handle_cache_closes_all() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let pdf_dir_a = TempDir::new().expect("Failed to create pdf dir a");
    let pdf_dir_b = TempDir::new().expect("Failed to create pdf dir b");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;

    if populate_handle_cache(&state, &pdf_dir_a).is_none() {
        eprintln!("Skipping: PDFium not installed");
        return;
    }
    populate_handle_cache(&state, &pdf_dir_b).expect("second populate");
    assert_eq!(state.pdf_handle_cache.snapshot().current_size, 2);

    let app = create_test_router_with_app_state(state.clone());
    let request = delete_request_with_auth("/api/v1/admin/pdf-cache/handles", &token);
    let (status, response): (StatusCode, Option<PdfHandleCacheClearResultDto>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.expect("response").handles_closed, 2);
    assert_eq!(state.pdf_handle_cache.snapshot().current_size, 0);
}

#[tokio::test]
async fn test_clear_handle_cache_requires_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_regular_user_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let request = delete_request_with_auth("/api/v1/admin/pdf-cache/handles", &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_evict_book_handle_missing_book_is_noop() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let path = format!("/api/v1/admin/pdf-cache/handles/{}", uuid::Uuid::new_v4());
    let request = delete_request_with_auth(&path, &token);
    let (status, response): (StatusCode, Option<PdfHandleCacheClearResultDto>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.expect("response").handles_closed, 0);
}

#[tokio::test]
async fn test_evict_book_handle_removes_entry() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let pdf_dir = TempDir::new().expect("Failed to create pdf dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_admin_and_token(&db, &state).await;

    let Some((book_id, _)) = populate_handle_cache(&state, &pdf_dir) else {
        eprintln!("Skipping: PDFium not installed");
        return;
    };
    assert_eq!(state.pdf_handle_cache.snapshot().current_size, 1);

    let app = create_test_router_with_app_state(state.clone());
    let path = format!("/api/v1/admin/pdf-cache/handles/{}", book_id);
    let request = delete_request_with_auth(&path, &token);
    let (status, response): (StatusCode, Option<PdfHandleCacheClearResultDto>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response.expect("response").handles_closed, 1);
    assert_eq!(state.pdf_handle_cache.snapshot().current_size, 0);
}

#[tokio::test]
async fn test_evict_book_handle_requires_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let cache_dir = TempDir::new().expect("Failed to create temp dir");
    let state = create_test_app_state_with_pdf_cache(db.clone(), &cache_dir).await;
    let token = create_regular_user_and_token(&db, &state).await;
    let app = create_test_router_with_app_state(state.clone());

    let path = format!("/api/v1/admin/pdf-cache/handles/{}", uuid::Uuid::new_v4());
    let request = delete_request_with_auth(&path, &token);
    let (status, _): (StatusCode, Option<ErrorResponse>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
