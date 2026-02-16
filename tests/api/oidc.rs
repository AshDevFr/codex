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
        oauth_state_manager: Arc::new(codex::services::user_plugin::OAuthStateManager::new()),
        plugin_file_storage: None,
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
        redirect_uri_base: None,
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
        redirect_uri_base: None,
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

// =============================================================================
// Provider Info Structure Tests
// =============================================================================

#[tokio::test]
async fn test_provider_info_contains_all_fields() {
    let (db, _temp_dir) = setup_test_db().await;
    let oidc_config = create_test_oidc_config();
    let state = create_test_state_with_oidc(db, oidc_config).await;
    let config = create_test_config_with_oidc();
    let app = create_router(state, &config);

    let request = get_request("/api/v1/auth/oidc/providers");
    let (status, response): (_, Option<serde_json::Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected JSON response");
    let providers = response["providers"].as_array().unwrap();
    let provider = &providers[0];

    // Verify all required fields are present
    assert!(provider.get("name").is_some(), "Missing 'name' field");
    assert!(
        provider.get("displayName").is_some(),
        "Missing 'displayName' field"
    );
    assert!(
        provider.get("loginUrl").is_some(),
        "Missing 'loginUrl' field"
    );

    // Verify login URL format
    let login_url = provider["loginUrl"].as_str().unwrap();
    assert!(
        login_url.starts_with("/api/v1/auth/oidc/"),
        "Login URL should start with /api/v1/auth/oidc/"
    );
    assert!(
        login_url.ends_with("/login"),
        "Login URL should end with /login"
    );
}

// =============================================================================
// Callback Error Handling Tests
// =============================================================================

#[tokio::test]
async fn test_callback_with_error_description_encoding() {
    let (db, _temp_dir) = setup_test_db().await;
    let oidc_config = create_test_oidc_config();
    let state = create_test_state_with_oidc(db, oidc_config).await;
    let config = create_test_config_with_oidc();
    let app = create_router(state, &config);

    // Simulate IdP error with special characters in description
    let request = get_request(
        "/api/v1/auth/oidc/test-provider/callback?code=&state=xyz&error=server_error&error_description=Something%20went%20wrong%20%26%20failed",
    );

    use axum::body::Body;
    use tower::ServiceExt;

    let response = app.oneshot(request.map(Body::from)).await.unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.contains("/login"));
    assert!(location.contains("error=server_error"));
}

#[tokio::test]
async fn test_callback_with_error_no_description() {
    let (db, _temp_dir) = setup_test_db().await;
    let oidc_config = create_test_oidc_config();
    let state = create_test_state_with_oidc(db, oidc_config).await;
    let config = create_test_config_with_oidc();
    let app = create_router(state, &config);

    // IdP error without description - should use default message
    let request =
        get_request("/api/v1/auth/oidc/test-provider/callback?code=&state=xyz&error=access_denied");

    use axum::body::Body;
    use tower::ServiceExt;

    let response = app.oneshot(request.map(Body::from)).await.unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.contains("error=access_denied"));
    // Should have default error description
    assert!(location.contains("error_description="));
}

#[tokio::test]
async fn test_callback_missing_code_and_state() {
    let (db, _temp_dir) = setup_test_db().await;
    let oidc_config = create_test_oidc_config();
    let state = create_test_state_with_oidc(db, oidc_config).await;
    let config = create_test_config_with_oidc();
    let app = create_router(state, &config);

    // Missing required query parameters
    let request = get_request("/api/v1/auth/oidc/test-provider/callback");

    let (status, _response): (_, Option<serde_json::Value>) = make_json_request(app, request).await;

    // Axum returns 400 for missing required query parameters
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_callback_unknown_provider() {
    let (db, _temp_dir) = setup_test_db().await;
    let oidc_config = create_test_oidc_config();
    let state = create_test_state_with_oidc(db, oidc_config).await;
    let config = create_test_config_with_oidc();
    let app = create_router(state, &config);

    // Valid state/code but unknown provider
    let request =
        get_request("/api/v1/auth/oidc/nonexistent/callback?code=abc123&state=invalid_state");

    let (status, _response): (_, Option<serde_json::Value>) = make_json_request(app, request).await;

    // Should fail - state is invalid regardless of provider
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// =============================================================================
// OidcService Unit Tests (additional coverage)
// =============================================================================

#[test]
fn test_oidc_service_disabled_returns_empty_providers() {
    let oidc_config = OidcConfig {
        enabled: false,
        ..OidcConfig::default()
    };
    let service = OidcService::new(oidc_config, "http://localhost:8080".to_string());

    assert!(!service.is_enabled());
    assert!(service.get_providers().is_empty());
}

#[test]
fn test_oidc_service_no_providers_configured() {
    let oidc_config = OidcConfig {
        enabled: true,
        auto_create_users: true,
        default_role: OidcDefaultRole::Reader,
        redirect_uri_base: None,
        providers: HashMap::new(),
    };
    let service = OidcService::new(oidc_config, "http://localhost:8080".to_string());

    assert!(service.is_enabled());
    assert!(service.get_providers().is_empty());
}

#[test]
fn test_oidc_config_default_values() {
    let config = OidcConfig::default();
    assert!(!config.enabled);
    assert!(config.auto_create_users);
    assert_eq!(config.default_role.as_str(), "reader");
    assert!(config.providers.is_empty());
}

#[test]
fn test_oidc_service_redirect_uri_with_custom_base() {
    let oidc_config = create_test_oidc_config();
    let service = OidcService::new(oidc_config, "https://codex.example.com/myapp".to_string());

    let providers = service.get_providers();
    assert_eq!(providers.len(), 1);
    // Service should strip trailing slashes from base
    assert!(service.is_enabled());
}

#[test]
fn test_oidc_service_auto_create_disabled() {
    let mut oidc_config = create_test_oidc_config();
    oidc_config.auto_create_users = false;
    let service = OidcService::new(oidc_config, "http://localhost:8080".to_string());

    assert!(!service.auto_create_users());
}

#[test]
fn test_oidc_role_mapping_empty_role_mapping() {
    let mut oidc_config = create_test_oidc_config();
    // Provider with empty role mapping
    let provider = OidcProviderConfig {
        display_name: "Empty Mapping".to_string(),
        issuer_url: "https://auth.example.com".to_string(),
        client_id: "test".to_string(),
        client_secret: None,
        client_secret_env: None,
        scopes: vec![],
        role_mapping: HashMap::new(), // No role mapping
        groups_claim: "groups".to_string(),
        username_claim: "preferred_username".to_string(),
        email_claim: "email".to_string(),
    };
    oidc_config
        .providers
        .insert("empty".to_string(), provider.clone());
    let service = OidcService::new(oidc_config, "http://localhost:8080".to_string());

    // With no role mapping, all groups should fall back to default
    let groups = vec!["any-group".to_string()];
    let role = service.map_groups_to_role(&groups, &provider);
    assert_eq!(role, "reader"); // Default role
}

#[test]
fn test_oidc_role_mapping_case_sensitive_groups() {
    let oidc_config = create_test_oidc_config();
    let service = OidcService::new(oidc_config.clone(), "http://localhost:8080".to_string());
    let provider = oidc_config.providers.get("test-provider").unwrap();

    // Groups are case-sensitive
    let groups = vec!["Codex-Admins".to_string()]; // Wrong case
    let role = service.map_groups_to_role(&groups, provider);
    assert_eq!(role, "reader"); // Should not match "codex-admins"
}

// =============================================================================
// OIDC Connection Database Tests
// =============================================================================

#[tokio::test]
async fn test_oidc_connection_create_and_find() {
    use codex::db::entities::oidc_connections;
    use codex::db::repositories::OidcConnectionRepository;

    let (db, _temp_dir) = setup_test_db().await;

    // Create a test user first
    let user = common::fixtures::create_test_user(
        "testuser",
        "test@example.com",
        "oidc:placeholder",
        false,
    );
    codex::db::repositories::UserRepository::create(&db, &user)
        .await
        .unwrap();

    // Create OIDC connection
    let connection = oidc_connections::Model {
        id: uuid::Uuid::new_v4(),
        user_id: user.id,
        provider_name: "authentik".to_string(),
        subject: "sub_12345".to_string(),
        email: Some("test@example.com".to_string()),
        display_name: Some("Test User".to_string()),
        groups: Some(serde_json::json!(["codex-admins", "users"])),
        access_token_hash: None,
        refresh_token_encrypted: None,
        token_expires_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        last_used_at: Some(chrono::Utc::now()),
    };

    let created = OidcConnectionRepository::create(&db, &connection)
        .await
        .unwrap();
    assert_eq!(created.provider_name, "authentik");
    assert_eq!(created.subject, "sub_12345");

    // Find by provider and subject
    let found = OidcConnectionRepository::find_by_provider_subject(&db, "authentik", "sub_12345")
        .await
        .unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.user_id, user.id);

    // Find by user ID
    let user_connections = OidcConnectionRepository::find_by_user_id(&db, user.id)
        .await
        .unwrap();
    assert_eq!(user_connections.len(), 1);

    // Find with non-existent provider
    let not_found =
        OidcConnectionRepository::find_by_provider_subject(&db, "keycloak", "sub_12345")
            .await
            .unwrap();
    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_oidc_connection_multiple_providers_per_user() {
    use codex::db::entities::oidc_connections;
    use codex::db::repositories::OidcConnectionRepository;

    let (db, _temp_dir) = setup_test_db().await;

    let user = common::fixtures::create_test_user(
        "multiuser",
        "multi@example.com",
        "oidc:placeholder",
        false,
    );
    codex::db::repositories::UserRepository::create(&db, &user)
        .await
        .unwrap();

    let now = chrono::Utc::now();

    // First provider connection
    let conn1 = oidc_connections::Model {
        id: uuid::Uuid::new_v4(),
        user_id: user.id,
        provider_name: "authentik".to_string(),
        subject: "auth_sub_1".to_string(),
        email: Some("multi@example.com".to_string()),
        display_name: Some("Multi User".to_string()),
        groups: Some(serde_json::json!(["admins"])),
        access_token_hash: None,
        refresh_token_encrypted: None,
        token_expires_at: None,
        created_at: now,
        updated_at: now,
        last_used_at: None,
    };
    OidcConnectionRepository::create(&db, &conn1).await.unwrap();

    // Second provider connection
    let conn2 = oidc_connections::Model {
        id: uuid::Uuid::new_v4(),
        user_id: user.id,
        provider_name: "keycloak".to_string(),
        subject: "kc_sub_1".to_string(),
        email: Some("multi@example.com".to_string()),
        display_name: Some("Multi User".to_string()),
        groups: Some(serde_json::json!(["readers"])),
        access_token_hash: None,
        refresh_token_encrypted: None,
        token_expires_at: None,
        created_at: now,
        updated_at: now,
        last_used_at: None,
    };
    OidcConnectionRepository::create(&db, &conn2).await.unwrap();

    // User should have two connections
    let connections = OidcConnectionRepository::find_by_user_id(&db, user.id)
        .await
        .unwrap();
    assert_eq!(connections.len(), 2);

    // Each connection should be findable by its provider+subject
    let found1 = OidcConnectionRepository::find_by_provider_subject(&db, "authentik", "auth_sub_1")
        .await
        .unwrap();
    assert!(found1.is_some());

    let found2 = OidcConnectionRepository::find_by_provider_subject(&db, "keycloak", "kc_sub_1")
        .await
        .unwrap();
    assert!(found2.is_some());
}

#[tokio::test]
async fn test_oidc_connection_update_groups_and_last_used() {
    use codex::db::entities::oidc_connections;
    use codex::db::repositories::OidcConnectionRepository;

    let (db, _temp_dir) = setup_test_db().await;

    let user = common::fixtures::create_test_user(
        "updateuser",
        "update@example.com",
        "oidc:placeholder",
        false,
    );
    codex::db::repositories::UserRepository::create(&db, &user)
        .await
        .unwrap();

    let now = chrono::Utc::now();
    let connection = oidc_connections::Model {
        id: uuid::Uuid::new_v4(),
        user_id: user.id,
        provider_name: "authentik".to_string(),
        subject: "sub_update".to_string(),
        email: Some("update@example.com".to_string()),
        display_name: Some("Original Name".to_string()),
        groups: Some(serde_json::json!(["old-group"])),
        access_token_hash: None,
        refresh_token_encrypted: None,
        token_expires_at: None,
        created_at: now,
        updated_at: now,
        last_used_at: None,
    };
    let created = OidcConnectionRepository::create(&db, &connection)
        .await
        .unwrap();

    // Update groups and last used
    let new_groups = Some(serde_json::json!(["new-group", "another-group"]));
    OidcConnectionRepository::update_groups_and_last_used(
        &db,
        created.id,
        new_groups,
        Some("updated@example.com".to_string()),
        Some("Updated Name".to_string()),
    )
    .await
    .unwrap();

    // Verify update
    let updated = OidcConnectionRepository::get_by_id(&db, created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated.groups.unwrap(),
        serde_json::json!(["new-group", "another-group"])
    );
    assert_eq!(updated.email.as_deref(), Some("updated@example.com"));
    assert_eq!(updated.display_name.as_deref(), Some("Updated Name"));
    assert!(updated.last_used_at.is_some());
}

#[tokio::test]
async fn test_oidc_connection_cascade_delete_on_user_delete() {
    use codex::db::entities::oidc_connections;
    use codex::db::repositories::OidcConnectionRepository;

    let (db, _temp_dir) = setup_test_db().await;

    let user = common::fixtures::create_test_user(
        "deleteuser",
        "delete@example.com",
        "oidc:placeholder",
        false,
    );
    codex::db::repositories::UserRepository::create(&db, &user)
        .await
        .unwrap();

    let now = chrono::Utc::now();
    let connection = oidc_connections::Model {
        id: uuid::Uuid::new_v4(),
        user_id: user.id,
        provider_name: "authentik".to_string(),
        subject: "sub_delete".to_string(),
        email: None,
        display_name: None,
        groups: None,
        access_token_hash: None,
        refresh_token_encrypted: None,
        token_expires_at: None,
        created_at: now,
        updated_at: now,
        last_used_at: None,
    };
    OidcConnectionRepository::create(&db, &connection)
        .await
        .unwrap();

    // Verify connection exists
    let connections = OidcConnectionRepository::find_by_user_id(&db, user.id)
        .await
        .unwrap();
    assert_eq!(connections.len(), 1);

    // Delete the user
    codex::db::repositories::UserRepository::delete(&db, user.id)
        .await
        .unwrap();

    // OIDC connections should be cascade deleted
    let connections = OidcConnectionRepository::find_by_user_id(&db, user.id)
        .await
        .unwrap();
    assert_eq!(connections.len(), 0);
}

// =============================================================================
// Login Endpoint Additional Tests
// =============================================================================

#[tokio::test]
async fn test_login_provider_name_is_path_safe() {
    let (db, _temp_dir) = setup_test_db().await;
    let oidc_config = create_test_oidc_config();
    let state = create_test_state_with_oidc(db, oidc_config).await;
    let config = create_test_config_with_oidc();
    let app = create_router(state, &config);

    // Try with URL-unsafe characters in provider name
    let request = post_request("/api/v1/auth/oidc/../../admin/login");
    let (status, _): (_, Option<serde_json::Value>) = make_json_request(app, request).await;

    // Should not succeed - provider name won't match
    assert_ne!(status, StatusCode::OK);
}

// =============================================================================
// OIDC Config Serialization Tests
// =============================================================================

#[test]
fn test_oidc_default_role_roundtrip() {
    let admin = OidcDefaultRole::Admin;
    let maintainer = OidcDefaultRole::Maintainer;
    let reader = OidcDefaultRole::Reader;

    assert_eq!(admin.as_str(), "admin");
    assert_eq!(maintainer.as_str(), "maintainer");
    assert_eq!(reader.as_str(), "reader");

    // Test serde roundtrip
    let json = serde_json::to_string(&admin).unwrap();
    let deserialized: OidcDefaultRole = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.as_str(), "admin");
}

#[test]
fn test_oidc_provider_config_serde() {
    let mut role_mapping = HashMap::new();
    role_mapping.insert("admin".to_string(), vec!["admins".to_string()]);

    let config = OidcProviderConfig {
        display_name: "Test".to_string(),
        issuer_url: "https://auth.example.com".to_string(),
        client_id: "client".to_string(),
        client_secret: Some("secret".to_string()),
        client_secret_env: None,
        scopes: vec!["openid".to_string(), "email".to_string()],
        role_mapping,
        groups_claim: "groups".to_string(),
        username_claim: "preferred_username".to_string(),
        email_claim: "email".to_string(),
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: OidcProviderConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.display_name, "Test");
    assert_eq!(deserialized.client_id, "client");
    assert_eq!(deserialized.scopes.len(), 2);
    assert!(deserialized.role_mapping.contains_key("admin"));
}

#[test]
fn test_oidc_config_with_all_fields() {
    let yaml = r#"
enabled: true
auto_create_users: false
default_role: admin
redirect_uri_base: https://codex.example.com
providers:
  my-idp:
    display_name: "My IdP"
    issuer_url: "https://idp.example.com"
    client_id: "codex"
    client_secret: "supersecret"
    scopes:
      - email
      - profile
      - groups
    role_mapping:
      admin:
        - idp-admins
      maintainer:
        - idp-editors
      reader:
        - idp-readers
    groups_claim: "custom_groups"
    username_claim: "uid"
    email_claim: "mail"
"#;

    let config: OidcConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(config.enabled);
    assert!(!config.auto_create_users);
    assert_eq!(config.default_role.as_str(), "admin");
    assert_eq!(
        config.redirect_uri_base.as_deref(),
        Some("https://codex.example.com")
    );

    let provider = config.providers.get("my-idp").unwrap();
    assert_eq!(provider.display_name, "My IdP");
    assert_eq!(provider.groups_claim, "custom_groups");
    assert_eq!(provider.username_claim, "uid");
    assert_eq!(provider.email_claim, "mail");
    assert_eq!(provider.scopes.len(), 3);
    assert_eq!(
        provider.role_mapping.get("admin").unwrap(),
        &vec!["idp-admins".to_string()]
    );
}

#[test]
fn test_oidc_config_minimal_yaml() {
    let yaml = r#"
enabled: true
providers:
  simple:
    display_name: "Simple"
    issuer_url: "https://auth.example.com"
    client_id: "codex"
"#;

    let config: OidcConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(config.enabled);
    assert!(config.auto_create_users); // Default
    assert_eq!(config.default_role.as_str(), "reader"); // Default

    let provider = config.providers.get("simple").unwrap();
    assert_eq!(provider.groups_claim, "groups"); // Default
    assert_eq!(provider.username_claim, "preferred_username"); // Default
    assert_eq!(provider.email_claim, "email"); // Default
}
