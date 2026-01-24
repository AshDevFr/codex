use axum::Router;
use codex::api::extractors::{auth::UserAuthCache, AppState, AuthState};
use codex::api::permissions::UserRole;
use codex::api::routes::create_router;
use codex::config::{AuthConfig, Config, DatabaseConfig, EmailConfig, FilesConfig, PdfConfig};
use codex::db::entities::users;
use codex::events::EventBroadcaster;
use codex::services::email::EmailService;
use codex::services::{
    AuthTrackingService, FileCleanupService, InflightThumbnailTracker, PdfPageCache,
    ReadProgressService, SettingsService, ThumbnailService,
};
use codex::utils::jwt::JwtService;
use http_body_util::BodyExt;
use hyper::{body::Bytes, Request, StatusCode};
use sea_orm::DatabaseConnection;
use serde::de::DeserializeOwned;
use std::sync::Arc;
use tower::ServiceExt;

/// Helper to create AuthState for tests (deprecated - use create_test_app_state)
pub async fn create_test_auth_state(db: DatabaseConnection) -> Arc<AuthState> {
    let jwt_service = Arc::new(JwtService::new(
        "test_secret_key_for_integration_tests".to_string(),
        24, // 24 hour expiry
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
        task_metrics_service: None, // Tests don't need metrics service
        scheduler: None,            // Tests don't need scheduler
        read_progress_service,
        auth_tracking_service,
        pdf_page_cache,
        inflight_thumbnails: Arc::new(InflightThumbnailTracker::new()),
        user_auth_cache: Arc::new(UserAuthCache::new()),
    })
}

/// Helper to create AppState for tests
pub async fn create_test_app_state(db: DatabaseConnection) -> Arc<AppState> {
    let jwt_service = Arc::new(JwtService::new(
        "test_secret_key_for_integration_tests".to_string(),
        24, // 24 hour expiry
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
        task_metrics_service: None, // Tests don't need metrics service
        scheduler: None,            // Tests don't need scheduler
        read_progress_service,
        auth_tracking_service,
        pdf_page_cache,
        inflight_thumbnails: Arc::new(InflightThumbnailTracker::new()),
        user_auth_cache: Arc::new(UserAuthCache::new()),
    })
}

/// Helper to generate a JWT token for a test user
/// This derives the role from the user's role field (or defaults to Reader)
pub fn generate_test_token(state: &AppState, user: &users::Model) -> String {
    let role = user.role.parse().unwrap_or(UserRole::Reader);
    state
        .jwt_service
        .generate_token(user.id, user.username.clone(), role)
        .expect("Failed to generate test token")
}

/// Helper to create a test config (Komga API disabled by default)
pub fn create_test_config() -> Config {
    let mut config = Config::default();
    // Disable CORS in tests to avoid conflicts with allow_credentials
    // Tests don't need CORS since they make direct requests, not cross-origin
    config.api.cors_enabled = false;
    config.api.enable_api_docs = false;
    // Komga API is disabled by default
    config.komga_api.enabled = false;
    config
}

/// Helper to create a test config with Komga API enabled
pub fn create_test_config_with_komga() -> Config {
    let mut config = create_test_config();
    config.komga_api.enabled = true;
    config.komga_api.prefix = "komga".to_string();
    config
}

/// Helper to create the API router with test state (deprecated - use create_test_router_with_app_state)
pub async fn create_test_router(state: Arc<AuthState>) -> Router {
    // Convert AuthState to AppState for compatibility
    let auth_config = Arc::new(AuthConfig::default());
    let database_config = Arc::new(DatabaseConfig::default());
    let pdf_config = Arc::new(PdfConfig::default());
    let email_service = Arc::new(EmailService::new(EmailConfig::default()));
    let event_broadcaster = Arc::new(EventBroadcaster::new(1000));
    let settings_service = Arc::new(
        SettingsService::new(state.db.clone())
            .await
            .expect("Failed to initialize settings service for tests"),
    );
    let files_config = FilesConfig::default();
    let thumbnail_service = Arc::new(ThumbnailService::new(files_config.clone()));
    let file_cleanup_service = Arc::new(FileCleanupService::new(files_config));
    let read_progress_service = Arc::new(ReadProgressService::new(state.db.clone()));
    let auth_tracking_service = Arc::new(AuthTrackingService::new(state.db.clone()));
    let pdf_page_cache = Arc::new(PdfPageCache::new(&pdf_config.cache_dir, false)); // Disabled in tests
    let app_state = Arc::new(AppState {
        db: state.db.clone(),
        jwt_service: state.jwt_service.clone(),
        auth_config,
        database_config,
        pdf_config,
        email_service,
        event_broadcaster,
        settings_service,
        thumbnail_service,
        file_cleanup_service,
        task_metrics_service: None, // Tests don't need metrics service
        scheduler: None,            // Tests don't need scheduler
        read_progress_service,
        auth_tracking_service,
        pdf_page_cache,
        inflight_thumbnails: Arc::new(InflightThumbnailTracker::new()),
        user_auth_cache: Arc::new(UserAuthCache::new()),
    });
    let config = create_test_config();
    create_router(app_state, &config)
}

/// Helper to create the API router with AppState
pub fn create_test_router_with_app_state(state: Arc<AppState>) -> Router {
    let config = create_test_config();
    create_router(state, &config)
}

/// Helper to set up a test app with database and router (convenience function)
pub async fn setup_test_app(db: DatabaseConnection) -> (Arc<AppState>, Router) {
    let state = create_test_app_state(db).await;
    let router = create_test_router_with_app_state(state.clone());
    (state, router)
}

/// Helper to create the API router with Komga API enabled
pub fn create_test_router_with_komga(state: Arc<AppState>) -> Router {
    let config = create_test_config_with_komga();
    create_router(state, &config)
}

/// Helper to set up a test app with Komga API enabled
pub async fn setup_test_app_with_komga(db: DatabaseConnection) -> (Arc<AppState>, Router) {
    let state = create_test_app_state(db).await;
    let router = create_test_router_with_komga(state.clone());
    (state, router)
}

/// Helper to create a GET request with Basic Auth header
pub fn get_request_with_basic_auth(uri: &str, username: &str, password: &str) -> Request<String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let credentials = format!("{}:{}", username, password);
    let encoded = STANDARD.encode(&credentials);
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("Authorization", format!("Basic {}", encoded))
        .body(String::new())
        .unwrap()
}

/// Helper to make an HTTP request and get the response
pub async fn make_request(app: Router, request: Request<String>) -> (StatusCode, Bytes) {
    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("Failed to read response body")
        .to_bytes();

    (status, body)
}

/// Helper to make a JSON request and parse JSON response
pub async fn make_json_request<T: DeserializeOwned>(
    app: Router,
    request: Request<String>,
) -> (StatusCode, Option<T>) {
    let (status, body) = make_request(app, request).await;

    let parsed = if body.is_empty() {
        None
    } else {
        serde_json::from_slice(&body).ok()
    };

    (status, parsed)
}

/// Helper to make a request and get raw bytes response (for binary data like files)
pub async fn make_raw_request(app: Router, request: Request<String>) -> (StatusCode, Vec<u8>) {
    let (status, body) = make_request(app, request).await;
    (status, body.to_vec())
}

/// Helper to create a GET request
pub fn get_request(uri: &str) -> Request<String> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(String::new())
        .unwrap()
}

/// Helper to create a GET request with Authorization header
pub fn get_request_with_auth(uri: &str, token: &str) -> Request<String> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap()
}

/// Helper to create a GET request with X-API-Key header
pub fn get_request_with_api_key(uri: &str, api_key: &str) -> Request<String> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("X-API-Key", api_key)
        .body(String::new())
        .unwrap()
}

/// Helper to create a POST request (no auth)
pub fn post_request(uri: &str) -> Request<String> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .body(String::new())
        .unwrap()
}

/// Helper to create a POST request with JSON body
pub fn post_json_request<T: serde::Serialize>(uri: &str, body: &T) -> Request<String> {
    let json = serde_json::to_string(body).unwrap();
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("Content-Type", "application/json")
        .body(json)
        .unwrap()
}

/// Helper to create a POST request with Authorization header (no body)
pub fn post_request_with_auth(uri: &str, token: &str) -> Request<String> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap()
}

/// Helper to create a POST request with JSON body and Authorization header
pub fn post_json_request_with_auth<T: serde::Serialize>(
    uri: &str,
    body: &T,
    token: &str,
) -> Request<String> {
    let json = serde_json::to_string(body).unwrap();
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .body(json)
        .unwrap()
}

/// Helper to create a POST request with raw JSON string and Authorization header
pub fn post_request_with_auth_json(uri: &str, token: &str, json: &str) -> Request<String> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .body(json.to_string())
        .unwrap()
}

/// Helper to create a PUT request with JSON body (no auth)
pub fn put_json_request<T: serde::Serialize>(uri: &str, body: &T) -> Request<String> {
    let json = serde_json::to_string(body).unwrap();
    Request::builder()
        .method("PUT")
        .uri(uri)
        .header("Content-Type", "application/json")
        .body(json)
        .unwrap()
}

/// Helper to create a PUT request with JSON body and Authorization header
pub fn put_json_request_with_auth<T: serde::Serialize>(
    uri: &str,
    body: &T,
    token: &str,
) -> Request<String> {
    let json = serde_json::to_string(body).unwrap();
    Request::builder()
        .method("PUT")
        .uri(uri)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .body(json)
        .unwrap()
}

/// Helper to create a PATCH request with JSON body (no auth)
pub fn patch_json_request<T: serde::Serialize>(uri: &str, body: &T) -> Request<String> {
    let json = serde_json::to_string(body).unwrap();
    Request::builder()
        .method("PATCH")
        .uri(uri)
        .header("Content-Type", "application/json")
        .body(json)
        .unwrap()
}

/// Helper to create a PATCH request with JSON body and Authorization header
pub fn patch_json_request_with_auth<T: serde::Serialize>(
    uri: &str,
    body: &T,
    token: &str,
) -> Request<String> {
    let json = serde_json::to_string(body).unwrap();
    Request::builder()
        .method("PATCH")
        .uri(uri)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .body(json)
        .unwrap()
}

/// Helper to create a PATCH request with raw JSON string and Authorization header
pub fn patch_request_with_auth_json(uri: &str, token: &str, json: &str) -> Request<String> {
    Request::builder()
        .method("PATCH")
        .uri(uri)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .body(json.to_string())
        .unwrap()
}

/// Helper to create a PUT request with JSON body, Authorization header, and custom IP headers
pub fn put_json_request_with_auth_and_ip<T: serde::Serialize>(
    uri: &str,
    body: &T,
    token: &str,
    ip_address: &str,
) -> Request<String> {
    let json = serde_json::to_string(body).unwrap();
    Request::builder()
        .method("PUT")
        .uri(uri)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .header("X-Forwarded-For", ip_address)
        .body(json)
        .unwrap()
}

/// Helper to create a POST request with JSON body, Authorization header, and custom IP headers
pub fn post_json_request_with_auth_and_ip<T: serde::Serialize>(
    uri: &str,
    body: &T,
    token: &str,
    ip_address: &str,
) -> Request<String> {
    let json = serde_json::to_string(body).unwrap();
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .header("X-Forwarded-For", ip_address)
        .body(json)
        .unwrap()
}

/// Helper to create a DELETE request with Authorization header
pub fn delete_request_with_auth(uri: &str, token: &str) -> Request<String> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap()
}

/// Helper to create a DELETE request without Authorization header
pub fn delete_request(uri: &str) -> Request<String> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .body(String::new())
        .unwrap()
}

/// Helper to create a PUT request with JSON body
pub fn put_request(uri: &str) -> Request<String> {
    Request::builder()
        .method("PUT")
        .uri(uri)
        .header("Content-Type", "application/json")
        .body(String::new())
        .unwrap()
}

/// Helper to create a PUT request with JSON body and Authorization header
pub fn put_request_with_auth(uri: &str, body: &str, token: &str) -> Request<String> {
    Request::builder()
        .method("PUT")
        .uri(uri)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .body(body.to_string())
        .unwrap()
}

/// Helper to create a POST multipart request with file upload and Authorization header
pub fn post_multipart_request_with_auth(
    uri: &str,
    field_name: &str,
    file_data: &[u8],
    filename: &str,
    token: &str,
) -> Request<String> {
    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";

    // Build multipart body
    let mut body = String::new();
    body.push_str(&format!("--{}\r\n", boundary));
    body.push_str(&format!(
        "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
        field_name, filename
    ));
    body.push_str("Content-Type: application/octet-stream\r\n\r\n");
    // For binary data, we'll base64 encode it in the test and decode in handler
    // Actually, for our tests we'll use text-based approach
    body.push_str(&String::from_utf8_lossy(file_data));
    body.push_str(&format!("\r\n--{}--\r\n", boundary));

    Request::builder()
        .method("POST")
        .uri(uri)
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .header("Authorization", format!("Bearer {}", token))
        .body(body)
        .unwrap()
}
