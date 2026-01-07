use axum::Router;
use codex::api::extractors::{AppState, AuthState};
use codex::api::routes::create_router;
use codex::config::{ApiConfig, AuthConfig, EmailConfig};
use codex::events::EventBroadcaster;
use codex::services::email::EmailService;
use codex::utils::jwt::JwtService;
use http_body_util::BodyExt;
use hyper::{body::Bytes, Request, StatusCode};
use sea_orm::DatabaseConnection;
use serde::de::DeserializeOwned;
use std::sync::Arc;
use tower::ServiceExt;

/// Helper to create AuthState for tests (deprecated - use create_test_app_state)
pub fn create_test_auth_state(db: DatabaseConnection) -> Arc<AuthState> {
    let jwt_service = Arc::new(JwtService::new(
        "test_secret_key_for_integration_tests".to_string(),
        24, // 24 hour expiry
    ));

    let auth_config = Arc::new(AuthConfig::default());
    let email_service = Arc::new(EmailService::new(EmailConfig::default()));
    let event_broadcaster = Arc::new(EventBroadcaster::new(1000));

    Arc::new(AppState {
        db,
        jwt_service,
        auth_config,
        email_service,
        event_broadcaster,
    })
}

/// Helper to create AppState for tests
pub fn create_test_app_state(db: DatabaseConnection) -> Arc<AppState> {
    let jwt_service = Arc::new(JwtService::new(
        "test_secret_key_for_integration_tests".to_string(),
        24, // 24 hour expiry
    ));

    let auth_config = Arc::new(AuthConfig::default());
    let email_service = Arc::new(EmailService::new(EmailConfig::default()));
    let event_broadcaster = Arc::new(EventBroadcaster::new(1000));

    Arc::new(AppState {
        db,
        jwt_service,
        auth_config,
        email_service,
        event_broadcaster,
    })
}

/// Helper to create a test API config
pub fn create_test_api_config() -> ApiConfig {
    ApiConfig {
        base_path: "/api/v1".to_string(),
        enable_swagger: false,
        swagger_path: "/docs".to_string(),
        cors_enabled: true,
        cors_origins: vec!["*".to_string()],
        max_page_size: 100,
    }
}

/// Helper to create the API router with test state (deprecated - use create_test_router_with_app_state)
pub fn create_test_router(state: Arc<AuthState>) -> Router {
    // Convert AuthState to AppState for compatibility
    let auth_config = Arc::new(AuthConfig::default());
    let email_service = Arc::new(EmailService::new(EmailConfig::default()));
    let event_broadcaster = Arc::new(EventBroadcaster::new(1000));
    let app_state = Arc::new(AppState {
        db: state.db.clone(),
        jwt_service: state.jwt_service.clone(),
        auth_config,
        email_service,
        event_broadcaster,
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
pub fn setup_test_app(db: DatabaseConnection) -> (Arc<AppState>, Router) {
    let state = create_test_app_state(db);
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

/// Helper to create a DELETE request with Authorization header
pub fn delete_request_with_auth(uri: &str, token: &str) -> Request<String> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("Authorization", format!("Bearer {}", token))
        .body(String::new())
        .unwrap()
}
