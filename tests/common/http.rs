use axum::Router;
use codex::api::extractors::{AppState, AuthState};
use codex::api::routes::create_router;
use codex::config::{ApiConfig, AuthConfig, EmailConfig, ThumbnailConfig};
use codex::events::EventBroadcaster;
use codex::services::email::EmailService;
use codex::services::{SettingsService, ThumbnailService};
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
    let email_service = Arc::new(EmailService::new(EmailConfig::default()));
    let event_broadcaster = Arc::new(EventBroadcaster::new(1000));
    let settings_service = Arc::new(
        SettingsService::new(db.clone())
            .await
            .expect("Failed to initialize settings service for tests"),
    );
    let thumbnail_service = Arc::new(ThumbnailService::new(ThumbnailConfig::default()));

    Arc::new(AppState {
        db,
        jwt_service,
        auth_config,
        email_service,
        event_broadcaster,
        settings_service,
        thumbnail_service,
        scheduler: None, // Tests don't need scheduler
    })
}

/// Helper to create AppState for tests
pub async fn create_test_app_state(db: DatabaseConnection) -> Arc<AppState> {
    let jwt_service = Arc::new(JwtService::new(
        "test_secret_key_for_integration_tests".to_string(),
        24, // 24 hour expiry
    ));

    let auth_config = Arc::new(AuthConfig::default());
    let email_service = Arc::new(EmailService::new(EmailConfig::default()));
    let event_broadcaster = Arc::new(EventBroadcaster::new(1000));
    let settings_service = Arc::new(
        SettingsService::new(db.clone())
            .await
            .expect("Failed to initialize settings service for tests"),
    );
    let thumbnail_service = Arc::new(ThumbnailService::new(ThumbnailConfig::default()));

    Arc::new(AppState {
        db,
        jwt_service,
        auth_config,
        email_service,
        event_broadcaster,
        settings_service,
        thumbnail_service,
        scheduler: None, // Tests don't need scheduler
    })
}

/// Helper to create a test API config
pub fn create_test_api_config() -> ApiConfig {
    ApiConfig {
        base_path: "/api/v1".to_string(),
        enable_api_docs: false,
        api_docs_path: "/docs".to_string(),
        // Disable CORS in tests to avoid conflicts with allow_credentials
        // Tests don't need CORS since they make direct requests, not cross-origin
        cors_enabled: false,
        cors_origins: vec![],
        max_page_size: 100,
    }
}

/// Helper to create the API router with test state (deprecated - use create_test_router_with_app_state)
pub async fn create_test_router(state: Arc<AuthState>) -> Router {
    // Convert AuthState to AppState for compatibility
    let auth_config = Arc::new(AuthConfig::default());
    let email_service = Arc::new(EmailService::new(EmailConfig::default()));
    let event_broadcaster = Arc::new(EventBroadcaster::new(1000));
    let settings_service = Arc::new(
        SettingsService::new(state.db.clone())
            .await
            .expect("Failed to initialize settings service for tests"),
    );
    let thumbnail_service = Arc::new(ThumbnailService::new(ThumbnailConfig::default()));
    let app_state = Arc::new(AppState {
        db: state.db.clone(),
        jwt_service: state.jwt_service.clone(),
        auth_config,
        email_service,
        event_broadcaster,
        settings_service,
        thumbnail_service,
        scheduler: None, // Tests don't need scheduler
    });
    let api_config = create_test_api_config();
    create_router(app_state, &api_config)
}

/// Helper to create the API router with AppState
pub fn create_test_router_with_app_state(state: Arc<AppState>) -> Router {
    let api_config = create_test_api_config();
    create_router(state, &api_config)
}

/// Helper to set up a test app with database and router (convenience function)
pub async fn setup_test_app(db: DatabaseConnection) -> (Arc<AppState>, Router) {
    let state = create_test_app_state(db).await;
    let router = create_test_router_with_app_state(state.clone());
    (state, router)
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
