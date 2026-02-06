//! OIDC Authentication API Integration Tests
//!
//! Tests for OpenID Connect (OIDC) authentication endpoints.
//! Note: Full flow tests require mocking external IdPs, so these tests
//! focus on endpoint behavior with disabled and enabled OIDC configurations.

#[path = "../common/mod.rs"]
mod common;

use codex::api::extractors::AppState;
use codex::api::extractors::auth::UserAuthCache;
use codex::api::routes::create_router;
use codex::config::{
    AuthConfig, Config, DatabaseConfig, EmailConfig, FilesConfig, OidcConfig, OidcDefaultRole,
    OidcProviderConfig, PdfConfig,
};
use codex::events::EventBroadcaster;
use codex::services::email::EmailService;
use codex::services::{
    AuthTrackingService, FileCleanupService, InflightThumbnailTracker, OidcService, PdfPageCache,
    PluginMetricsService, ReadProgressService, SettingsService, ThumbnailService,
    plugin::PluginManager,
};
use codex::utils::jwt::JwtService;
use common::*;
use hyper::StatusCode;
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::Arc;

/// Helper to create AppState with OIDC enabled
async fn create_test_state_with_oidc(
    db: DatabaseConnection,
    oidc_config: OidcConfig,
) -> Arc<AppState> {
    let jwt_service = Arc::new(JwtService::new(
        "test_secret_key_for_integration_tests".to_string(),
        24,
    ));

    let auth_config = Arc::new(AuthConfig {
        oidc: oidc_config.clone(),
        ..Default::default()
    });

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
    let plugin_manager = Arc::new(PluginManager::with_defaults(Arc::new(db.clone())));
    let plugin_metrics_service = Arc::new(PluginMetricsService::new());

    // Create OIDC service if enabled
    let oidc_service = if oidc_config.enabled {
        Some(Arc::new(OidcService::new(
            oidc_config,
            "http://localhost:8080".to_string(),
        )))
    } else {
        None
    };

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
        oidc_service,
    })
}

/// Create a test OIDC config with one provider
fn create_test_oidc_config() -> OidcConfig {
    let mut providers = HashMap::new();
    let mut role_mapping = HashMap::new();
    role_mapping.insert("admin".to_string(), vec!["codex-admins".to_string()]);
    role_mapping.insert("reader".to_string(), vec!["codex-users".to_string()]);

    providers.insert(
        "test-provider".to_string(),
        OidcProviderConfig {
            display_name: "Test Provider".to_string(),
            issuer_url: "https://auth.example.com".to_string(),
            client_id: "test-client-id".to_string(),
            client_secret: Some("test-client-secret".to_string()),
            client_secret_env: None,
            scopes: vec!["email".to_string(), "profile".to_string()],
            role_mapping,
            groups_claim: "groups".to_string(),
            username_claim: "preferred_username".to_string(),
            email_claim: "email".to_string(),
        },
    );

    OidcConfig {
        enabled: true,
        auto_create_users: true,
        default_role: OidcDefaultRole::Reader,
        providers,
    }
}

fn create_test_config_no_oidc() -> Config {
    let mut config = Config::default();
    config.api.cors_enabled = false;
    config.api.enable_api_docs = false;
    config.auth.oidc.enabled = false;
    config
}

fn create_test_config_with_oidc() -> Config {
    let mut config = Config::default();
    config.api.cors_enabled = false;
    config.api.enable_api_docs = false;
    config.auth.oidc = create_test_oidc_config();
    config
}

// =============================================================================
// Provider List Tests
// =============================================================================

#[tokio::test]
async fn test_list_providers_oidc_disabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_state_with_oidc(db, OidcConfig::default()).await;
    let config = create_test_config_no_oidc();
    let app = create_router(state, &config);

    let request = get_request("/api/v1/auth/oidc/providers");
    let (status, response): (_, Option<serde_json::Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected JSON response");
    assert_eq!(response["enabled"], false);
    assert!(response["providers"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_list_providers_oidc_enabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let oidc_config = create_test_oidc_config();
    let state = create_test_state_with_oidc(db, oidc_config).await;
    let config = create_test_config_with_oidc();
    let app = create_router(state, &config);

    let request = get_request("/api/v1/auth/oidc/providers");
    let (status, response): (_, Option<serde_json::Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected JSON response");
    assert_eq!(response["enabled"], true);

    let providers = response["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0]["name"], "test-provider");
    assert_eq!(providers[0]["displayName"], "Test Provider");
    assert!(
        providers[0]["loginUrl"]
            .as_str()
            .unwrap()
            .contains("/api/v1/auth/oidc/test-provider/login")
    );
}

// =============================================================================
// Login Initiation Tests
// =============================================================================

#[tokio::test]
async fn test_login_oidc_disabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_state_with_oidc(db, OidcConfig::default()).await;
    let config = create_test_config_no_oidc();
    let app = create_router(state, &config);

    let request = post_request("/api/v1/auth/oidc/test-provider/login");
    let (status, _response): (_, Option<serde_json::Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_login_unknown_provider() {
    let (db, _temp_dir) = setup_test_db().await;
    let oidc_config = create_test_oidc_config();
    let state = create_test_state_with_oidc(db, oidc_config).await;
    let config = create_test_config_with_oidc();
    let app = create_router(state, &config);

    let request = post_request("/api/v1/auth/oidc/unknown-provider/login");
    let (status, response): (_, Option<serde_json::Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let response = response.expect("Expected JSON response");
    assert!(
        response["message"]
            .as_str()
            .unwrap()
            .contains("Unknown OIDC provider")
    );
}

// Note: Testing successful login initiation requires a real/mocked IdP for discovery
// The test below will fail because the discovery endpoint isn't accessible
// In a real test environment, you would use a mock server like wiremock

#[tokio::test]
async fn test_login_discovery_failure() {
    // This test verifies that we handle discovery failures gracefully
    let (db, _temp_dir) = setup_test_db().await;

    // Create config with non-existent issuer (discovery will fail)
    let oidc_config = create_test_oidc_config();
    let state = create_test_state_with_oidc(db, oidc_config).await;
    let config = create_test_config_with_oidc();
    let app = create_router(state, &config);

    let request = post_request("/api/v1/auth/oidc/test-provider/login");
    let (status, _response): (_, Option<serde_json::Value>) = make_json_request(app, request).await;

    // Should return 500 because discovery will fail (no real IdP)
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

// =============================================================================
// Callback Tests
// =============================================================================

#[tokio::test]
async fn test_callback_oidc_disabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_state_with_oidc(db, OidcConfig::default()).await;
    let config = create_test_config_no_oidc();
    let app = create_router(state, &config);

    let request = get_request("/api/v1/auth/oidc/test-provider/callback?code=abc123&state=xyz789");
    let (status, _response): (_, Option<serde_json::Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_callback_invalid_state() {
    let (db, _temp_dir) = setup_test_db().await;
    let oidc_config = create_test_oidc_config();
    let state = create_test_state_with_oidc(db, oidc_config).await;
    let config = create_test_config_with_oidc();
    let app = create_router(state, &config);

    // Try callback with invalid state (not from a login initiation)
    let request =
        get_request("/api/v1/auth/oidc/test-provider/callback?code=abc123&state=invalid_state");
    let (status, _response): (_, Option<serde_json::Value>) = make_json_request(app, request).await;

    // Should fail with unauthorized because state is invalid
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_callback_with_error() {
    let (db, _temp_dir) = setup_test_db().await;
    let oidc_config = create_test_oidc_config();
    let state = create_test_state_with_oidc(db, oidc_config).await;
    let config = create_test_config_with_oidc();
    let app = create_router(state, &config);

    // Simulate IdP returning an error
    let request = get_request(
        "/api/v1/auth/oidc/test-provider/callback?code=&state=xyz&error=access_denied&error_description=User%20cancelled",
    );

    // Use oneshot directly to check for redirect
    use axum::body::Body;
    use tower::ServiceExt;

    let response = app.oneshot(request.map(Body::from)).await.unwrap();

    // Should redirect to login page with error parameters
    // Axum's Redirect::to returns 303 See Other
    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.contains("/login"));
    assert!(location.contains("error=access_denied"));
}

// =============================================================================
// Multi-Provider Tests
// =============================================================================

#[tokio::test]
async fn test_list_multiple_providers() {
    let (db, _temp_dir) = setup_test_db().await;

    let mut providers = HashMap::new();

    providers.insert(
        "authentik".to_string(),
        OidcProviderConfig {
            display_name: "Authentik".to_string(),
            issuer_url: "https://authentik.example.com".to_string(),
            client_id: "client1".to_string(),
            client_secret: Some("secret1".to_string()),
            client_secret_env: None,
            scopes: vec!["email".to_string()],
            role_mapping: HashMap::new(),
            groups_claim: "groups".to_string(),
            username_claim: "preferred_username".to_string(),
            email_claim: "email".to_string(),
        },
    );

    providers.insert(
        "keycloak".to_string(),
        OidcProviderConfig {
            display_name: "Keycloak".to_string(),
            issuer_url: "https://keycloak.example.com".to_string(),
            client_id: "client2".to_string(),
            client_secret: Some("secret2".to_string()),
            client_secret_env: None,
            scopes: vec!["email".to_string(), "profile".to_string()],
            role_mapping: HashMap::new(),
            groups_claim: "groups".to_string(),
            username_claim: "preferred_username".to_string(),
            email_claim: "email".to_string(),
        },
    );

    let oidc_config = OidcConfig {
        enabled: true,
        auto_create_users: true,
        default_role: OidcDefaultRole::Reader,
        providers,
    };

    let state = create_test_state_with_oidc(db, oidc_config.clone()).await;

    let mut config = Config::default();
    config.api.cors_enabled = false;
    config.api.enable_api_docs = false;
    config.auth.oidc = oidc_config;

    let app = create_router(state, &config);

    let request = get_request("/api/v1/auth/oidc/providers");
    let (status, response): (_, Option<serde_json::Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected JSON response");
    assert_eq!(response["enabled"], true);

    let providers = response["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 2);

    // Collect provider names
    let names: Vec<&str> = providers
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"authentik"));
    assert!(names.contains(&"keycloak"));
}
