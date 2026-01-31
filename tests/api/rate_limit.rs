//! Integration tests for rate limiting middleware
//!
//! Tests cover:
//! - Anonymous request rate limiting
//! - Authenticated request rate limiting (higher limits)
//! - Exempt path bypass (/health, /api/v1/events)
//! - Rate limit headers on all responses
//! - 429 response format and headers
//! - Multiple clients have separate limits

#[path = "../common/mod.rs"]
mod common;

use axum::body::Body;
use axum::Router;
use codex::api::extractors::{auth::UserAuthCache, AppState};
use codex::api::routes::create_router;
use codex::config::{Config, RateLimitConfig};
use codex::db::repositories::UserRepository;
use codex::services::rate_limiter::RateLimiterService;
use codex::services::InflightThumbnailTracker;
use codex::utils::password;
use common::*;
use http_body_util::BodyExt;
use hyper::{Method, Request, StatusCode};
use serde::Deserialize;
use std::sync::Arc;
use tower::ServiceExt;

/// Rate limit exceeded response body (matches the middleware's response format)
#[derive(Debug, Deserialize)]
struct RateLimitExceededResponse {
    error: String,
    message: String,
    retry_after: u64,
}

/// Create a test config with rate limiting enabled
fn create_rate_limit_test_config() -> Config {
    let mut config = Config::default();
    config.api.cors_enabled = false;
    config.api.enable_api_docs = false;
    config.komga_api.enabled = false;

    // Configure rate limiting with small values for testing
    // Use very low refill rates to prevent tokens from being replenished
    // between requests during tests (which would cause flaky failures)
    config.rate_limit.enabled = true;
    config.rate_limit.anonymous_rps = 1; // 1 token per second - won't refill during test
    config.rate_limit.anonymous_burst = 3; // Small burst for easy testing
    config.rate_limit.authenticated_rps = 1; // 1 token per second - won't refill during test
    config.rate_limit.authenticated_burst = 5; // Small burst for easy testing
    config.rate_limit.exempt_paths = vec!["/health".to_string(), "/api/v1/events".to_string()];

    config
}

/// Create an AppState with rate limiting enabled
async fn create_rate_limited_app_state(
    db: sea_orm::DatabaseConnection,
    config: &RateLimitConfig,
) -> Arc<AppState> {
    use codex::config::{AuthConfig, DatabaseConfig, EmailConfig, FilesConfig, PdfConfig};
    use codex::events::EventBroadcaster;
    use codex::services::email::EmailService;
    use codex::services::{
        plugin::PluginManager, AuthTrackingService, FileCleanupService, PdfPageCache,
        PluginMetricsService, ReadProgressService, SettingsService, ThumbnailService,
    };
    use codex::utils::jwt::JwtService;

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
    let read_progress_service = Arc::new(ReadProgressService::new(db.clone()));
    let auth_tracking_service = Arc::new(AuthTrackingService::new(db.clone()));
    let pdf_page_cache = Arc::new(PdfPageCache::new(&pdf_config.cache_dir, false));

    // Create rate limiter service with test config
    let rate_limiter_service = Some(Arc::new(RateLimiterService::new(Arc::new(config.clone()))));
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
        rate_limiter_service,
        plugin_manager,
        plugin_metrics_service,
    })
}

/// Create a router with rate limiting enabled
fn create_rate_limited_router(state: Arc<AppState>, config: &Config) -> Router {
    create_router(state, config)
}

/// Helper to create a GET request with X-Forwarded-For header (for simulating different clients)
fn get_request_with_ip(uri: &str, ip: &str) -> Request<String> {
    Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header("X-Forwarded-For", ip)
        .body(String::new())
        .unwrap()
}

/// Helper to create a GET request with auth token and IP
fn get_request_with_auth_and_ip(uri: &str, token: &str, ip: &str) -> Request<String> {
    Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header("Authorization", format!("Bearer {}", token))
        .header("X-Forwarded-For", ip)
        .body(String::new())
        .unwrap()
}

/// Helper to make a request and get the full response (including headers)
async fn make_request_with_response(
    app: Router,
    request: Request<String>,
) -> axum::http::Response<Body> {
    app.oneshot(request)
        .await
        .expect("Failed to execute request")
}

// ============================================================================
// Anonymous Rate Limiting Tests
// ============================================================================

#[tokio::test]
async fn test_rate_limit_anonymous_requests() {
    let (db, _temp_dir) = setup_test_db().await;
    let config = create_rate_limit_test_config();
    let state = create_rate_limited_app_state(db, &config.rate_limit).await;
    let app = create_rate_limited_router(state, &config);

    // Anonymous burst is 3 - first 3 requests should succeed
    for i in 0..3 {
        let request = get_request_with_ip("/api/v1/libraries", "192.168.1.100");
        let (status, _body) = make_request(app.clone(), request).await;
        // 401 is expected since we're not authenticated, but we're testing rate limiting
        // The request should not be 429 yet
        assert_ne!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "Request {} should not be rate limited",
            i
        );
    }

    // The 4th request should be rate limited (429)
    let request = get_request_with_ip("/api/v1/libraries", "192.168.1.100");
    let (status, body) = make_request(app.clone(), request).await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "4th request should be rate limited"
    );

    // Verify response body
    let response: RateLimitExceededResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(response.error, "rate_limit_exceeded");
    assert!(response.message.contains("Too many requests"));
    assert!(response.retry_after > 0);
}

#[tokio::test]
async fn test_rate_limit_blocks_after_burst_exhausted() {
    let (db, _temp_dir) = setup_test_db().await;
    let config = create_rate_limit_test_config();
    let state = create_rate_limited_app_state(db, &config.rate_limit).await;
    let app = create_rate_limited_router(state, &config);

    let client_ip = "10.0.0.50";

    // Exhaust the burst limit (3 requests)
    for _ in 0..3 {
        let request = get_request_with_ip("/api/v1/libraries", client_ip);
        make_request(app.clone(), request).await;
    }

    // Additional requests should all be blocked
    for _ in 0..5 {
        let request = get_request_with_ip("/api/v1/libraries", client_ip);
        let (status, _body) = make_request(app.clone(), request).await;
        assert_eq!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "Request after burst exhaustion should be rate limited"
        );
    }
}

// ============================================================================
// Authenticated Rate Limiting Tests
// ============================================================================

#[tokio::test]
async fn test_rate_limit_authenticated_higher_limit() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user
    let password = "test_password_123";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user(
        "ratelimituser",
        "ratelimit@example.com",
        &password_hash,
        false,
    );
    UserRepository::create(&db, &user).await.unwrap();

    let config = create_rate_limit_test_config();
    let state = create_rate_limited_app_state(db, &config.rate_limit).await;

    // Generate a token for the test user
    let token = generate_test_token(&state, &user);

    let app = create_rate_limited_router(state, &config);

    // Authenticated burst is 5 - first 5 requests should succeed
    for i in 0..5 {
        let request = get_request_with_auth_and_ip("/api/v1/libraries", &token, "192.168.1.200");
        let (status, _body) = make_request(app.clone(), request).await;
        assert_ne!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "Authenticated request {} should not be rate limited",
            i
        );
    }

    // The 6th request should be rate limited
    let request = get_request_with_auth_and_ip("/api/v1/libraries", &token, "192.168.1.200");
    let (status, _body) = make_request(app.clone(), request).await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "6th authenticated request should be rate limited"
    );
}

#[tokio::test]
async fn test_rate_limit_authenticated_user_tracked_by_user_id() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user
    let password = "test_password_123";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user("multiipuser", "multiip@example.com", &password_hash, false);
    UserRepository::create(&db, &user).await.unwrap();

    let config = create_rate_limit_test_config();
    let state = create_rate_limited_app_state(db, &config.rate_limit).await;
    let token = generate_test_token(&state, &user);
    let app = create_rate_limited_router(state, &config);

    // Make requests from different IPs but same user - should share the limit
    // Authenticated burst is 5
    let ips = ["10.0.0.1", "10.0.0.2", "10.0.0.3", "10.0.0.4", "10.0.0.5"];

    for (i, ip) in ips.iter().enumerate() {
        let request = get_request_with_auth_and_ip("/api/v1/libraries", &token, ip);
        let (status, _body) = make_request(app.clone(), request).await;
        assert_ne!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "Request {} from IP {} should not be rate limited",
            i,
            ip
        );
    }

    // 6th request (from any IP) should be rate limited
    let request = get_request_with_auth_and_ip("/api/v1/libraries", &token, "10.0.0.99");
    let (status, _body) = make_request(app.clone(), request).await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "6th request should be rate limited regardless of IP"
    );
}

// ============================================================================
// Exempt Paths Tests
// ============================================================================

#[tokio::test]
async fn test_rate_limit_exempt_paths_health() {
    let (db, _temp_dir) = setup_test_db().await;
    let config = create_rate_limit_test_config();
    let state = create_rate_limited_app_state(db, &config.rate_limit).await;
    let app = create_rate_limited_router(state, &config);

    // /health is exempt - should never be rate limited
    // Make many more requests than the burst limit
    for i in 0..20 {
        let request = get_request_with_ip("/health", "192.168.1.50");
        let (status, _body) = make_request(app.clone(), request).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "/health request {} should not be rate limited",
            i
        );
    }
}

#[tokio::test]
async fn test_rate_limit_exempt_paths_events() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user for authentication
    let password = "test_password_123";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user("eventsuser", "events@example.com", &password_hash, false);
    UserRepository::create(&db, &user).await.unwrap();

    let config = create_rate_limit_test_config();
    let state = create_rate_limited_app_state(db, &config.rate_limit).await;
    let token = generate_test_token(&state, &user);
    let app = create_rate_limited_router(state, &config);

    // /api/v1/events is exempt - should never be rate limited
    // Note: The actual SSE endpoint might require auth, but we're testing rate limit bypass
    for i in 0..10 {
        let request = get_request_with_auth_and_ip("/api/v1/events", &token, "192.168.1.60");
        let (status, _body) = make_request(app.clone(), request).await;
        // We expect either OK (if it's the SSE endpoint) or some other status
        // but NOT 429 (rate limited)
        assert_ne!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "/api/v1/events request {} should not be rate limited",
            i
        );
    }
}

#[tokio::test]
async fn test_rate_limit_exempt_paths_prefix_match() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user for authentication
    let password = "test_password_123";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user("prefixuser", "prefix@example.com", &password_hash, false);
    UserRepository::create(&db, &user).await.unwrap();

    let config = create_rate_limit_test_config();
    let state = create_rate_limited_app_state(db, &config.rate_limit).await;
    let token = generate_test_token(&state, &user);
    let app = create_rate_limited_router(state, &config);

    // /api/v1/events/* should also be exempt (prefix matching)
    for i in 0..10 {
        let request =
            get_request_with_auth_and_ip("/api/v1/events/some/subpath", &token, "192.168.1.70");
        let (status, _body) = make_request(app.clone(), request).await;
        assert_ne!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "/api/v1/events/some/subpath request {} should not be rate limited",
            i
        );
    }
}

// ============================================================================
// Rate Limit Headers Tests
// ============================================================================

#[tokio::test]
async fn test_rate_limit_headers_on_success() {
    let (db, _temp_dir) = setup_test_db().await;
    let config = create_rate_limit_test_config();
    let state = create_rate_limited_app_state(db, &config.rate_limit).await;
    let app = create_rate_limited_router(state, &config);

    let request = get_request_with_ip("/api/v1/libraries", "192.168.1.110");
    let response = make_request_with_response(app, request).await;

    let headers = response.headers();

    // Check X-RateLimit-Limit header
    assert!(
        headers.contains_key("X-RateLimit-Limit"),
        "Response should contain X-RateLimit-Limit header"
    );
    let limit = headers
        .get("X-RateLimit-Limit")
        .unwrap()
        .to_str()
        .unwrap()
        .parse::<u32>()
        .unwrap();
    assert_eq!(limit, 3, "Limit should be 3 (anonymous burst)");

    // Check X-RateLimit-Remaining header
    assert!(
        headers.contains_key("X-RateLimit-Remaining"),
        "Response should contain X-RateLimit-Remaining header"
    );
    let remaining = headers
        .get("X-RateLimit-Remaining")
        .unwrap()
        .to_str()
        .unwrap()
        .parse::<u32>()
        .unwrap();
    // After 1 request, remaining should be 2 or less
    assert!(
        remaining <= 2,
        "Remaining should be <= 2 after one request, got {}",
        remaining
    );

    // Check X-RateLimit-Reset header
    assert!(
        headers.contains_key("X-RateLimit-Reset"),
        "Response should contain X-RateLimit-Reset header"
    );
}

#[tokio::test]
async fn test_rate_limit_headers_remaining_decreases() {
    let (db, _temp_dir) = setup_test_db().await;
    let config = create_rate_limit_test_config();
    let state = create_rate_limited_app_state(db, &config.rate_limit).await;
    let app = create_rate_limited_router(state, &config);

    let client_ip = "192.168.1.120";
    let mut previous_remaining: Option<u32> = None;

    for i in 0..3 {
        let request = get_request_with_ip("/api/v1/libraries", client_ip);
        let response = make_request_with_response(app.clone(), request).await;

        let remaining = response
            .headers()
            .get("X-RateLimit-Remaining")
            .unwrap()
            .to_str()
            .unwrap()
            .parse::<u32>()
            .unwrap();

        if let Some(prev) = previous_remaining {
            assert!(
                remaining < prev,
                "Remaining should decrease: request {} had {}, request {} has {}",
                i - 1,
                prev,
                i,
                remaining
            );
        }
        previous_remaining = Some(remaining);
    }
}

// ============================================================================
// 429 Response Format Tests
// ============================================================================

#[tokio::test]
async fn test_rate_limit_429_response_format() {
    let (db, _temp_dir) = setup_test_db().await;
    let config = create_rate_limit_test_config();
    let state = create_rate_limited_app_state(db, &config.rate_limit).await;
    let app = create_rate_limited_router(state, &config);

    let client_ip = "192.168.1.130";

    // Exhaust burst limit
    for _ in 0..3 {
        let request = get_request_with_ip("/api/v1/libraries", client_ip);
        make_request(app.clone(), request).await;
    }

    // Get 429 response
    let request = get_request_with_ip("/api/v1/libraries", client_ip);
    let response = make_request_with_response(app, request).await;

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    // Check headers
    let headers = response.headers();

    assert!(
        headers.contains_key("X-RateLimit-Limit"),
        "429 response should have X-RateLimit-Limit"
    );
    assert!(
        headers.contains_key("X-RateLimit-Remaining"),
        "429 response should have X-RateLimit-Remaining"
    );
    assert!(
        headers.contains_key("X-RateLimit-Reset"),
        "429 response should have X-RateLimit-Reset"
    );
    assert!(
        headers.contains_key("Retry-After"),
        "429 response should have Retry-After"
    );

    // Remaining should be 0
    let remaining = headers
        .get("X-RateLimit-Remaining")
        .unwrap()
        .to_str()
        .unwrap()
        .parse::<u32>()
        .unwrap();
    assert_eq!(remaining, 0, "Remaining should be 0 on 429");

    // Retry-After should be > 0
    let retry_after = headers
        .get("Retry-After")
        .unwrap()
        .to_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    assert!(retry_after > 0, "Retry-After should be > 0");

    // Check Content-Type is JSON
    let content_type = headers.get("Content-Type").unwrap().to_str().unwrap();
    assert!(
        content_type.contains("application/json"),
        "Content-Type should be application/json"
    );

    // Check body format
    let body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    let response_body: RateLimitExceededResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(response_body.error, "rate_limit_exceeded");
    assert!(response_body.message.contains("Too many requests"));
    assert!(response_body.message.contains(&retry_after.to_string()));
    assert_eq!(response_body.retry_after, retry_after);
}

// ============================================================================
// Multiple Clients Tests
// ============================================================================

#[tokio::test]
async fn test_rate_limit_multiple_clients_separate_limits() {
    let (db, _temp_dir) = setup_test_db().await;
    let config = create_rate_limit_test_config();
    let state = create_rate_limited_app_state(db, &config.rate_limit).await;
    let app = create_rate_limited_router(state, &config);

    // Client 1 exhausts their limit
    for _ in 0..3 {
        let request = get_request_with_ip("/api/v1/libraries", "10.0.0.1");
        make_request(app.clone(), request).await;
    }

    // Client 1 should be blocked
    let request = get_request_with_ip("/api/v1/libraries", "10.0.0.1");
    let (status, _body) = make_request(app.clone(), request).await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "Client 1 should be rate limited"
    );

    // Client 2 should still have their full limit
    for i in 0..3 {
        let request = get_request_with_ip("/api/v1/libraries", "10.0.0.2");
        let (status, _body) = make_request(app.clone(), request).await;
        assert_ne!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "Client 2 request {} should not be rate limited",
            i
        );
    }

    // Client 2's 4th request should be rate limited
    let request = get_request_with_ip("/api/v1/libraries", "10.0.0.2");
    let (status, _body) = make_request(app.clone(), request).await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "Client 2's 4th request should be rate limited"
    );
}

#[tokio::test]
async fn test_rate_limit_anonymous_and_authenticated_separate() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user
    let password = "test_password_123";
    let password_hash = password::hash_password(password).unwrap();
    let user = create_test_user(
        "separateuser",
        "separate@example.com",
        &password_hash,
        false,
    );
    UserRepository::create(&db, &user).await.unwrap();

    let config = create_rate_limit_test_config();
    let state = create_rate_limited_app_state(db, &config.rate_limit).await;
    let token = generate_test_token(&state, &user);
    let app = create_rate_limited_router(state, &config);

    let shared_ip = "192.168.1.150";

    // Anonymous user exhausts their limit (3 requests)
    for _ in 0..3 {
        let request = get_request_with_ip("/api/v1/libraries", shared_ip);
        make_request(app.clone(), request).await;
    }

    // Anonymous is now blocked
    let request = get_request_with_ip("/api/v1/libraries", shared_ip);
    let (status, _body) = make_request(app.clone(), request).await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "Anonymous should be rate limited"
    );

    // Authenticated user (same IP) should still have their limit
    // They are tracked by user ID, not IP
    for i in 0..5 {
        let request = get_request_with_auth_and_ip("/api/v1/libraries", &token, shared_ip);
        let (status, _body) = make_request(app.clone(), request).await;
        assert_ne!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "Authenticated request {} should not be rate limited",
            i
        );
    }
}

// ============================================================================
// Rate Limiting Disabled Tests
// ============================================================================

#[tokio::test]
async fn test_rate_limit_disabled_allows_unlimited_requests() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create config with rate limiting disabled
    let mut config = Config::default();
    config.api.cors_enabled = false;
    config.api.enable_api_docs = false;
    config.komga_api.enabled = false;
    config.rate_limit.enabled = false;

    // Create state WITHOUT rate limiter
    let state = create_test_app_state(db).await;
    let app = create_test_router_with_app_state(state);

    // Should be able to make many requests without being limited
    for i in 0..20 {
        let request = get_request_with_ip("/api/v1/libraries", "192.168.1.160");
        let (status, _body) = make_request(app.clone(), request).await;
        assert_ne!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "Request {} should not be rate limited when rate limiting is disabled",
            i
        );
    }
}
