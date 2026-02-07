//! User Plugin API endpoint tests
//!
//! Tests for user plugin management endpoints:
//! - GET /api/v1/user/plugins - List user plugins (enabled + available)
//! - GET /api/v1/user/plugins/:id - Get a single user plugin
//! - POST /api/v1/user/plugins/:id/enable - Enable a plugin
//! - POST /api/v1/user/plugins/:id/disable - Disable a plugin
//! - PATCH /api/v1/user/plugins/:id/config - Update plugin config
//! - DELETE /api/v1/user/plugins/:id - Disconnect a plugin

#[path = "../common/mod.rs"]
mod common;

use common::db::setup_test_db;
use common::fixtures::create_test_user;
use common::http::{
    create_test_auth_state, create_test_router, delete_request_with_auth, generate_test_token,
    get_request, get_request_with_auth, make_json_request, patch_json_request_with_auth,
    post_request_with_auth,
};
use hyper::StatusCode;
use serde_json::json;

use codex::api::routes::v1::dto::user_plugins::{
    SyncStatusDto, SyncTriggerResponse, UserPluginDto, UserPluginsListResponse,
};
use codex::db::repositories::{PluginsRepository, UserPluginsRepository, UserRepository};
use codex::utils::password;
use std::sync::Once;

static INIT_ENCRYPTION: Once = Once::new();

/// Ensure encryption key is set for tests that need to store OAuth tokens
fn ensure_test_encryption_key() {
    INIT_ENCRYPTION.call_once(|| {
        if std::env::var("CODEX_ENCRYPTION_KEY").is_err() {
            // SAFETY: This is only called once from test code in a Once block,
            // before any concurrent access to this env var.
            unsafe {
                std::env::set_var(
                    "CODEX_ENCRYPTION_KEY",
                    "dGVzdGtleXRlc3RrZXl0ZXN0a2V5dGVzdGtleTEyMzQ=", // 32 bytes
                );
            }
        }
    });
}

// =============================================================================
// Helper functions
// =============================================================================

/// Create an admin user and return a JWT token
async fn create_admin_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AppState,
) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    generate_test_token(state, &created)
}

/// Create a regular user and return (user_id, token)
async fn create_user_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AppState,
    username: &str,
) -> (uuid::Uuid, String) {
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user(
        username,
        &format!("{}@example.com", username),
        &password_hash,
        false,
    );
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = generate_test_token(state, &created);
    (created.id, token)
}

/// Create a user-type plugin (admin operation) and return its ID
async fn create_user_type_plugin(
    db: &sea_orm::DatabaseConnection,
    name: &str,
    display_name: &str,
) -> uuid::Uuid {
    let plugin = PluginsRepository::create(
        db,
        name,
        display_name,
        Some("A test user plugin"),
        "user", // plugin_type
        "echo", // command
        vec!["hello".to_string()],
        vec![],
        None,
        vec![],
        vec![],
        vec![],
        None,
        "none",
        None,
        true, // enabled
        None,
        None,
    )
    .await
    .unwrap();
    plugin.id
}

/// Create a user-type plugin with a sync provider manifest
async fn create_sync_plugin(
    db: &sea_orm::DatabaseConnection,
    name: &str,
    display_name: &str,
) -> uuid::Uuid {
    let plugin = PluginsRepository::create(
        db,
        name,
        display_name,
        Some("A test sync plugin"),
        "user",
        "echo",
        vec!["hello".to_string()],
        vec![],
        None,
        vec![],
        vec![],
        vec![],
        None,
        "none",
        None,
        true,
        None,
        None,
    )
    .await
    .unwrap();

    // Set manifest with sync provider capability
    let manifest = json!({
        "name": name,
        "displayName": display_name,
        "version": "1.0.0",
        "protocolVersion": "1.0",
        "pluginType": "user",
        "capabilities": {
            "userSyncProvider": true
        },
        "userDescription": "Sync reading progress"
    });
    PluginsRepository::update_manifest(db, plugin.id, Some(manifest))
        .await
        .unwrap();

    plugin.id
}

/// Create a system-type plugin (admin operation) and return its ID
async fn create_system_type_plugin(db: &sea_orm::DatabaseConnection, name: &str) -> uuid::Uuid {
    let plugin = PluginsRepository::create(
        db,
        name,
        name,
        None,
        "system", // plugin_type
        "echo",
        vec![],
        vec![],
        None,
        vec![],
        vec![],
        vec![],
        None,
        "none",
        None,
        true,
        None,
        None,
    )
    .await
    .unwrap();
    plugin.id
}

// =============================================================================
// Authentication Tests
// =============================================================================

#[tokio::test]
async fn test_list_user_plugins_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;

    let request = get_request("/api/v1/user/plugins");
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_user_plugin_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request(&format!("/api/v1/user/plugins/{}", fake_id));
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// =============================================================================
// List User Plugins Tests
// =============================================================================

#[tokio::test]
async fn test_list_user_plugins_empty() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let request = get_request_with_auth("/api/v1/user/plugins", &token);
    let (status, response): (StatusCode, Option<UserPluginsListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert!(response.enabled.is_empty());
    assert!(response.available.is_empty());
}

#[tokio::test]
async fn test_list_user_plugins_shows_available() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    // Create a user-type plugin
    create_user_type_plugin(&db, "test-sync", "Test Sync Plugin").await;

    // System plugins should NOT appear in available
    create_system_type_plugin(&db, "system-plugin").await;

    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/user/plugins", &token);
    let (status, response): (StatusCode, Option<UserPluginsListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert!(response.enabled.is_empty());
    assert_eq!(response.available.len(), 1);
    assert_eq!(response.available[0].name, "test-sync");
    assert_eq!(response.available[0].display_name, "Test Sync Plugin");
}

#[tokio::test]
async fn test_list_user_plugins_shows_enabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync Plugin").await;

    // Enable the plugin
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // List should show it in enabled, not available
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/user/plugins", &token);
    let (status, response): (StatusCode, Option<UserPluginsListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert_eq!(response.enabled.len(), 1);
    assert_eq!(response.enabled[0].plugin_name, "test-sync");
    assert!(response.available.is_empty());
}

// =============================================================================
// Enable Plugin Tests
// =============================================================================

#[tokio::test]
async fn test_enable_plugin_success() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync").await;

    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, response): (StatusCode, Option<UserPluginDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let dto = response.expect("Expected response body");
    assert_eq!(dto.plugin_id, plugin_id);
    assert_eq!(dto.plugin_name, "test-sync");
    assert!(dto.enabled);
    assert!(!dto.connected); // No OAuth yet
}

#[tokio::test]
async fn test_enable_plugin_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let fake_id = uuid::Uuid::new_v4();
    let app = create_test_router(state.clone()).await;
    let request =
        post_request_with_auth(&format!("/api/v1/user/plugins/{}/enable", fake_id), &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_enable_system_plugin_rejected() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let system_id = create_system_type_plugin(&db, "system-plugin").await;

    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", system_id),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_enable_plugin_duplicate_rejected() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync").await;

    // Enable first time
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Enable second time - should conflict
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

// =============================================================================
// Get User Plugin Tests
// =============================================================================

#[tokio::test]
async fn test_get_user_plugin_success() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync").await;

    // Enable first
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Get the plugin
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(&format!("/api/v1/user/plugins/{}", plugin_id), &token);
    let (status, response): (StatusCode, Option<UserPluginDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let dto = response.expect("Expected response body");
    assert_eq!(dto.plugin_id, plugin_id);
    assert_eq!(dto.plugin_name, "test-sync");
    assert_eq!(dto.plugin_display_name, "Test Sync");
    assert!(dto.enabled);
    assert!(!dto.connected);
    assert_eq!(dto.health_status, "unknown");
}

#[tokio::test]
async fn test_get_user_plugin_not_enabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync").await;

    // Try to get without enabling
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(&format!("/api/v1/user/plugins/{}", plugin_id), &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_user_plugin_isolation() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token_a) = create_user_and_token(&db, &state, "usera").await;
    let (_, token_b) = create_user_and_token(&db, &state, "userb").await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync").await;

    // User A enables plugin
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token_a,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // User B cannot see User A's plugin instance
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(&format!("/api/v1/user/plugins/{}", plugin_id), &token_b);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// =============================================================================
// Disable Plugin Tests
// =============================================================================

#[tokio::test]
async fn test_disable_plugin_success() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync").await;

    // Enable
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Disable
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/disable", plugin_id),
        &token,
    );
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("Expected response body");
    assert_eq!(body["success"], true);
}

#[tokio::test]
async fn test_disable_plugin_not_enabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync").await;

    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/disable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// =============================================================================
// Update Config Tests
// =============================================================================

#[tokio::test]
async fn test_update_config_success() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync").await;

    // Enable first
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Update config
    let config_body = json!({ "config": { "autoSync": true, "syncInterval": 3600 } });
    let app = create_test_router(state.clone()).await;
    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/plugins/{}/config", plugin_id),
        &config_body,
        &token,
    );
    let (status, response): (StatusCode, Option<UserPluginDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let dto = response.expect("Expected response body");
    assert_eq!(dto.config["autoSync"], true);
    assert_eq!(dto.config["syncInterval"], 3600);
}

#[tokio::test]
async fn test_update_config_not_enabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync").await;

    let config_body = json!({ "config": { "autoSync": true } });
    let app = create_test_router(state.clone()).await;
    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/plugins/{}/config", plugin_id),
        &config_body,
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_config_invalid_not_object() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync").await;

    // Enable first
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Try to set config as an array (invalid)
    let config_body = json!({ "config": [1, 2, 3] });
    let app = create_test_router(state.clone()).await;
    let request = patch_json_request_with_auth(
        &format!("/api/v1/user/plugins/{}/config", plugin_id),
        &config_body,
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// =============================================================================
// Disconnect Plugin Tests
// =============================================================================

#[tokio::test]
async fn test_disconnect_plugin_success() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync").await;

    // Enable
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Disconnect
    let app = create_test_router(state.clone()).await;
    let request = delete_request_with_auth(&format!("/api/v1/user/plugins/{}", plugin_id), &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("Expected response body");
    assert_eq!(body["success"], true);

    // Verify plugin no longer accessible
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(&format!("/api/v1/user/plugins/{}", plugin_id), &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_disconnect_plugin_not_enabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync").await;

    let app = create_test_router(state.clone()).await;
    let request = delete_request_with_auth(&format!("/api/v1/user/plugins/{}", plugin_id), &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// =============================================================================
// User Isolation Tests
// =============================================================================

#[tokio::test]
async fn test_user_plugin_isolation_between_users() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token_a) = create_user_and_token(&db, &state, "usera").await;
    let (_, token_b) = create_user_and_token(&db, &state, "userb").await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync").await;

    // User A enables the plugin
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token_a,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // User B's list should show plugin as available (not enabled)
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/user/plugins", &token_b);
    let (status, response): (StatusCode, Option<UserPluginsListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert!(response.enabled.is_empty());
    assert_eq!(response.available.len(), 1);
    assert_eq!(response.available[0].plugin_id, plugin_id);

    // User B can also enable independently
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token_b,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Now both users have it enabled
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/user/plugins", &token_a);
    let (_, response_a): (StatusCode, Option<UserPluginsListResponse>) =
        make_json_request(app, request).await;
    assert_eq!(response_a.unwrap().enabled.len(), 1);

    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/user/plugins", &token_b);
    let (_, response_b): (StatusCode, Option<UserPluginsListResponse>) =
        make_json_request(app, request).await;
    assert_eq!(response_b.unwrap().enabled.len(), 1);
}

// =============================================================================
// Admin can also use user plugin endpoints (admin is a user too)
// =============================================================================

#[tokio::test]
async fn test_admin_can_enable_user_plugins() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let admin_token = create_admin_and_token(&db, &state).await;

    let plugin_id = create_user_type_plugin(&db, "test-sync", "Test Sync").await;

    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &admin_token,
    );
    let (status, response): (StatusCode, Option<UserPluginDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let dto = response.expect("Expected response body");
    assert_eq!(dto.plugin_id, plugin_id);
    assert!(dto.enabled);
}

// =============================================================================
// Sync Trigger Tests
// =============================================================================

#[tokio::test]
async fn test_trigger_sync_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = common::http::post_request(&format!("/api/v1/user/plugins/{}/sync", fake_id));
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_trigger_sync_not_enabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_sync_plugin(&db, "sync-anilist", "AniList Sync").await;

    // Try to sync without enabling
    let app = create_test_router(state.clone()).await;
    let request =
        post_request_with_auth(&format!("/api/v1/user/plugins/{}/sync", plugin_id), &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_trigger_sync_not_sync_provider() {
    ensure_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "testuser").await;

    // Create a user plugin WITHOUT sync capability
    let plugin_id = create_user_type_plugin(&db, "no-sync", "No Sync Plugin").await;

    // Enable it
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Fake authentication by setting oauth tokens directly
    let instance = UserPluginsRepository::get_by_user_and_plugin(&db, user_id, plugin_id)
        .await
        .unwrap()
        .unwrap();
    UserPluginsRepository::update_oauth_tokens(
        &db,
        instance.id,
        "fake_token",
        Some("fake_refresh"),
        None,
        None,
    )
    .await
    .unwrap();

    // Try to sync - should fail because plugin is not a sync provider
    let app = create_test_router(state.clone()).await;
    let request =
        post_request_with_auth(&format!("/api/v1/user/plugins/{}/sync", plugin_id), &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_trigger_sync_not_connected() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_sync_plugin(&db, "sync-anilist", "AniList Sync").await;

    // Enable but don't connect (no OAuth)
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Try to sync without being connected
    let app = create_test_router(state.clone()).await;
    let request =
        post_request_with_auth(&format!("/api/v1/user/plugins/{}/sync", plugin_id), &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_trigger_sync_success() {
    ensure_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_sync_plugin(&db, "sync-anilist", "AniList Sync").await;

    // Enable the plugin
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Simulate being connected by setting oauth tokens
    let instance = UserPluginsRepository::get_by_user_and_plugin(&db, user_id, plugin_id)
        .await
        .unwrap()
        .unwrap();
    UserPluginsRepository::update_oauth_tokens(
        &db,
        instance.id,
        "fake_access_token",
        Some("fake_refresh_token"),
        None,
        None,
    )
    .await
    .unwrap();

    // Trigger sync
    let app = create_test_router(state.clone()).await;
    let request =
        post_request_with_auth(&format!("/api/v1/user/plugins/{}/sync", plugin_id), &token);
    let (status, response): (StatusCode, Option<SyncTriggerResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let resp = response.expect("Expected response body");
    assert!(!resp.task_id.is_nil());
    assert!(resp.message.contains("AniList Sync"));
}

#[tokio::test]
async fn test_trigger_sync_disabled_plugin() {
    ensure_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_sync_plugin(&db, "sync-anilist", "AniList Sync").await;

    // Enable plugin
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Set tokens
    let instance = UserPluginsRepository::get_by_user_and_plugin(&db, user_id, plugin_id)
        .await
        .unwrap()
        .unwrap();
    UserPluginsRepository::update_oauth_tokens(
        &db,
        instance.id,
        "fake_token",
        Some("fake_refresh"),
        None,
        None,
    )
    .await
    .unwrap();

    // Disable the plugin
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/disable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Try to sync with disabled plugin
    let app = create_test_router(state.clone()).await;
    let request =
        post_request_with_auth(&format!("/api/v1/user/plugins/{}/sync", plugin_id), &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// =============================================================================
// Sync Status Tests
// =============================================================================

#[tokio::test]
async fn test_sync_status_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request(&format!("/api/v1/user/plugins/{}/sync/status", fake_id));
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_sync_status_not_enabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_sync_plugin(&db, "sync-anilist", "AniList Sync").await;

    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        &format!("/api/v1/user/plugins/{}/sync/status", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_sync_status_success() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_sync_plugin(&db, "sync-anilist", "AniList Sync").await;

    // Enable
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Get status
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        &format!("/api/v1/user/plugins/{}/sync/status", plugin_id),
        &token,
    );
    let (status, response): (StatusCode, Option<SyncStatusDto>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let dto = response.expect("Expected response body");
    assert_eq!(dto.plugin_id, plugin_id);
    assert_eq!(dto.plugin_name, "AniList Sync");
    assert!(!dto.connected); // No OAuth yet
    assert!(dto.last_sync_at.is_none());
    assert_eq!(dto.health_status, "unknown");
    assert_eq!(dto.failure_count, 0);
    assert!(dto.enabled);
}

#[tokio::test]
async fn test_sync_status_isolation() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token_a) = create_user_and_token(&db, &state, "usera").await;
    let (_, token_b) = create_user_and_token(&db, &state, "userb").await;

    let plugin_id = create_sync_plugin(&db, "sync-anilist", "AniList Sync").await;

    // User A enables
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token_a,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // User B cannot see User A's sync status
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        &format!("/api/v1/user/plugins/{}/sync/status", plugin_id),
        &token_b,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_sync_status_default_has_no_live_fields() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_sync_plugin(&db, "sync-anilist", "AniList Sync").await;

    // Enable
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Get status without ?live=true
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        &format!("/api/v1/user/plugins/{}/sync/status", plugin_id),
        &token,
    );
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("Expected response body");

    // DB fields present
    assert!(body.get("pluginId").is_some());
    assert!(body.get("pluginName").is_some());

    // Live fields absent (not null, truly absent from JSON)
    assert!(body.get("externalCount").is_none());
    assert!(body.get("pendingPush").is_none());
    assert!(body.get("pendingPull").is_none());
    assert!(body.get("conflicts").is_none());
    assert!(body.get("liveError").is_none());
}

#[tokio::test]
async fn test_sync_status_live_degrades_gracefully() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id = create_sync_plugin(&db, "sync-anilist", "AniList Sync").await;

    // Enable
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<UserPluginDto>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Get status with ?live=true (no real plugin process available)
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        &format!("/api/v1/user/plugins/{}/sync/status?live=true", plugin_id),
        &token,
    );
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    // Should still succeed with 200 (graceful degradation)
    assert_eq!(status, StatusCode::OK);
    let body = response.expect("Expected response body");

    // DB fields still present
    assert_eq!(body["pluginName"], "AniList Sync");
    assert_eq!(body["enabled"], true);

    // Live data fields absent due to error
    assert!(body.get("externalCount").is_none());
    assert!(body.get("pendingPush").is_none());
    assert!(body.get("pendingPull").is_none());
    assert!(body.get("conflicts").is_none());

    // live_error should be present explaining why
    assert!(body.get("liveError").is_some());
    let error_msg = body["liveError"].as_str().unwrap();
    assert!(!error_msg.is_empty());
}
