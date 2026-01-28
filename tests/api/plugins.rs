//! Plugin API endpoint tests
//!
//! Tests for the admin plugin management endpoints:
//! - GET /api/v1/admin/plugins - List all plugins
//! - POST /api/v1/admin/plugins - Create a new plugin
//! - GET /api/v1/admin/plugins/:id - Get a plugin by ID
//! - PATCH /api/v1/admin/plugins/:id - Update a plugin
//! - DELETE /api/v1/admin/plugins/:id - Delete a plugin
//! - POST /api/v1/admin/plugins/:id/enable - Enable a plugin
//! - POST /api/v1/admin/plugins/:id/disable - Disable a plugin
//! - POST /api/v1/admin/plugins/:id/test - Test a plugin connection
//! - GET /api/v1/admin/plugins/:id/health - Get plugin health
//! - POST /api/v1/admin/plugins/:id/reset - Reset plugin failure count

#[path = "../common/mod.rs"]
mod common;

use common::db::setup_test_db;
use common::fixtures::create_test_user;
use common::http::{
    create_test_auth_state, create_test_router, delete_request_with_auth, generate_test_token,
    get_request_with_auth, make_json_request, patch_json_request_with_auth,
    post_json_request_with_auth, post_request_with_auth,
};
use hyper::StatusCode;
use serde_json::json;

use codex::api::routes::v1::dto::{
    PluginDto, PluginHealthResponse, PluginStatusResponse, PluginTestResult, PluginsListResponse,
};
use codex::db::repositories::UserRepository;
use codex::utils::password;

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

/// Create a regular user and return a JWT token
async fn create_user_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AppState,
) -> String {
    let password_hash = password::hash_password("user123").unwrap();
    let user = create_test_user("regularuser", "user@example.com", &password_hash, false);
    let created = UserRepository::create(db, &user).await.unwrap();
    generate_test_token(state, &created)
}

// =============================================================================
// Authorization Tests
// =============================================================================

#[tokio::test]
async fn test_list_plugins_requires_admin() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_user_and_token(&db, &state).await;

    let request = get_request_with_auth("/api/v1/admin/plugins", &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_list_plugins_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = common::http::get_request("/api/v1/admin/plugins");
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// =============================================================================
// List Plugins Tests
// =============================================================================

#[tokio::test]
async fn test_list_plugins_empty() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let request = get_request_with_auth("/api/v1/admin/plugins", &token);
    let (status, response): (StatusCode, Option<PluginsListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert_eq!(response.total, 0);
    assert!(response.plugins.is_empty());
}

// =============================================================================
// Create Plugin Tests
// =============================================================================

#[tokio::test]
async fn test_create_plugin_success() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let body = json!({
        "name": "test_plugin",
        "displayName": "Test Plugin",
        "description": "A test plugin",
        "command": "node",
        "args": ["/path/to/plugin.js"],
        "permissions": ["metadata:write:summary"],
        "scopes": ["series:detail"],
        "enabled": false
    });

    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (status, response): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CREATED);
    let response = response.expect("Expected response body");
    assert_eq!(response.plugin.name, "test_plugin");
    assert_eq!(response.plugin.display_name, "Test Plugin");
    assert_eq!(response.plugin.command, "node");
    assert!(!response.plugin.enabled);
}

#[tokio::test]
async fn test_create_plugin_minimal() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let body = json!({
        "name": "minimal_plugin",
        "displayName": "Minimal",
        "command": "node"  // Must be in allowed commands list
    });

    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (status, response): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CREATED);
    let response = response.expect("Expected response body");
    assert_eq!(response.plugin.name, "minimal_plugin");
    assert_eq!(response.plugin.plugin_type, "system");
    assert_eq!(response.plugin.credential_delivery, "env");
}

#[tokio::test]
async fn test_create_plugin_invalid_name() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let body = json!({
        "name": "Invalid-Name",  // Contains uppercase and dash
        "displayName": "Test",
        "command": "node"
    });

    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_plugin_invalid_permission() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let body = json!({
        "name": "test_plugin",
        "displayName": "Test",
        "command": "node",
        "permissions": ["invalid:permission"]
    });

    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_plugin_invalid_scope() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let body = json!({
        "name": "test_plugin",
        "displayName": "Test",
        "command": "node",
        "scopes": ["invalid:scope"]
    });

    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_plugin_duplicate_name() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create first plugin
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "duplicate_test",
        "displayName": "First",
        "command": "node"
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (status, _): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::CREATED);

    // Try to create second plugin with same name
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "duplicate_test",
        "displayName": "Second",
        "command": "python"
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CONFLICT);
}

// =============================================================================
// Get Plugin Tests
// =============================================================================

#[tokio::test]
async fn test_get_plugin_success() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a plugin
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "get_test_plugin",
        "displayName": "Get Test",
        "command": "node"
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (_, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    let created = created.unwrap().plugin;

    // Get the plugin
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(&format!("/api/v1/admin/plugins/{}", created.id), &token);
    let (status, response): (StatusCode, Option<PluginDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert_eq!(response.id, created.id);
    assert_eq!(response.name, "get_test_plugin");
}

#[tokio::test]
async fn test_get_plugin_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(&format!("/api/v1/admin/plugins/{}", fake_id), &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// =============================================================================
// Update Plugin Tests
// =============================================================================

#[tokio::test]
async fn test_update_plugin_success() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a plugin
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "update_test_plugin",
        "displayName": "Original Name",
        "command": "node"
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (_, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    let created = created.unwrap().plugin;

    // Update the plugin
    let app = create_test_router(state.clone()).await;
    let update_body = json!({
        "displayName": "Updated Name"
    });
    let request = patch_json_request_with_auth(
        &format!("/api/v1/admin/plugins/{}", created.id),
        &update_body,
        &token,
    );
    let (status, response): (StatusCode, Option<PluginDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert_eq!(response.display_name, "Updated Name");
    assert_eq!(response.name, "update_test_plugin"); // Name shouldn't change
}

#[tokio::test]
async fn test_update_plugin_permissions() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a plugin with initial permissions
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "perm_test_plugin",
        "displayName": "Perm Test",
        "command": "node",
        "permissions": ["metadata:read"]
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (_, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    let created = created.unwrap().plugin;

    // Update permissions
    let app = create_test_router(state.clone()).await;
    let update_body = json!({
        "permissions": ["metadata:write:summary", "metadata:write:genres"]
    });
    let request = patch_json_request_with_auth(
        &format!("/api/v1/admin/plugins/{}", created.id),
        &update_body,
        &token,
    );
    let (status, response): (StatusCode, Option<PluginDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert_eq!(response.permissions.len(), 2);
    assert!(response
        .permissions
        .contains(&"metadata:write:summary".to_string()));
    assert!(response
        .permissions
        .contains(&"metadata:write:genres".to_string()));
}

// =============================================================================
// Delete Plugin Tests
// =============================================================================

#[tokio::test]
async fn test_delete_plugin_success() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a plugin
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "delete_test_plugin",
        "displayName": "Delete Test",
        "command": "node"
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (_, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    let created = created.unwrap().plugin;

    // Delete the plugin
    let app = create_test_router(state.clone()).await;
    let request =
        delete_request_with_auth(&format!("/api/v1/admin/plugins/{}", created.id), &token);
    let (status, _): (StatusCode, Option<()>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify it's gone
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(&format!("/api/v1/admin/plugins/{}", created.id), &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_plugin_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request = delete_request_with_auth(&format!("/api/v1/admin/plugins/{}", fake_id), &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// =============================================================================
// Enable/Disable Plugin Tests
// =============================================================================

#[tokio::test]
async fn test_enable_plugin() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a disabled plugin
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "enable_test_plugin",
        "displayName": "Enable Test",
        "command": "node",
        "enabled": false
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (_, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    let created = created.unwrap().plugin;
    assert!(!created.enabled);

    // Enable the plugin
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/admin/plugins/{}/enable", created.id),
        &token,
    );
    let (status, response): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert!(response.plugin.enabled);
    assert!(response.message.contains("enabled"));
}

#[tokio::test]
async fn test_disable_plugin() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create an enabled plugin
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "disable_test_plugin",
        "displayName": "Disable Test",
        "command": "node",
        "enabled": true
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (_, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    let created = created.unwrap().plugin;
    assert!(created.enabled);

    // Disable the plugin
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/admin/plugins/{}/disable", created.id),
        &token,
    );
    let (status, response): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert!(!response.plugin.enabled);
    assert!(response.message.contains("disabled"));
}

// =============================================================================
// Plugin Health Tests
// =============================================================================

#[tokio::test]
async fn test_get_plugin_health() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a plugin
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "health_test_plugin",
        "displayName": "Health Test",
        "command": "node"
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (_, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    let created = created.unwrap().plugin;

    // Get health
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        &format!("/api/v1/admin/plugins/{}/health", created.id),
        &token,
    );
    let (status, response): (StatusCode, Option<PluginHealthResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert_eq!(response.health.plugin_id, created.id);
    assert_eq!(response.health.name, "health_test_plugin");
    assert_eq!(response.health.failure_count, 0);
}

// =============================================================================
// Reset Failure Count Tests
// =============================================================================

#[tokio::test]
async fn test_reset_plugin_failures() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a plugin
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "reset_test_plugin",
        "displayName": "Reset Test",
        "command": "node"
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (_, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    let created = created.unwrap().plugin;

    // Reset failures (even though there aren't any, the endpoint should work)
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/admin/plugins/{}/reset", created.id),
        &token,
    );
    let (status, response): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert_eq!(response.plugin.failure_count, 0);
    assert!(response.message.contains("reset"));
}

// =============================================================================
// Test Plugin Connection Tests
// =============================================================================

#[tokio::test]
async fn test_test_plugin_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let fake_id = uuid::Uuid::new_v4();
    let request =
        post_request_with_auth(&format!("/api/v1/admin/plugins/{}/test", fake_id), &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_test_plugin_invalid_command() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a plugin with a valid command but nonexistent script
    // This tests runtime failure rather than validation failure
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "invalid_cmd_plugin",
        "displayName": "Invalid Command",
        "command": "node",  // Valid command
        "args": ["/nonexistent/script/that/does/not/exist.js"]  // Nonexistent script
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (status, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    let created = created.unwrap().plugin;

    // Test the plugin - should fail gracefully because the script doesn't exist
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/admin/plugins/{}/test", created.id),
        &token,
    );
    let (status, response): (StatusCode, Option<PluginTestResult>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert!(!response.success);
    // The error could be about module not found, file not found, or spawn failure
    assert!(
        response.message.to_lowercase().contains("fail")
            || response.message.to_lowercase().contains("error")
            || response.message.to_lowercase().contains("not found")
            || response.message.contains("MODULE_NOT_FOUND")
            || response.message.contains("ENOENT"),
        "Expected error message but got: {}",
        response.message
    );
}

// =============================================================================
// List with Plugins Tests
// =============================================================================

#[tokio::test]
async fn test_list_plugins_with_data() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create multiple plugins
    for i in 1..=3 {
        let app = create_test_router(state.clone()).await;
        let body = json!({
            "name": format!("list_test_plugin_{}", i),
            "displayName": format!("List Test {}", i),
            "command": "node"
        });
        let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
        let (status, _): (StatusCode, Option<PluginStatusResponse>) =
            make_json_request(app, request).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    // List all plugins
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/admin/plugins", &token);
    let (status, response): (StatusCode, Option<PluginsListResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert_eq!(response.total, 3);
    assert_eq!(response.plugins.len(), 3);
}

// =============================================================================
// Plugin Actions API Tests (Phase 4)
// =============================================================================

use codex::api::routes::v1::dto::{ExecutePluginResponse, PluginActionsResponse};

#[tokio::test]
async fn test_get_plugin_actions_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let request = common::http::get_request("/api/v1/plugins/actions?scope=series:detail");
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_plugin_actions_invalid_scope() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let request = get_request_with_auth("/api/v1/plugins/actions?scope=invalid:scope", &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_plugin_actions_empty() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let request = get_request_with_auth("/api/v1/plugins/actions?scope=series:detail", &token);
    let (status, response): (StatusCode, Option<PluginActionsResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert!(response.actions.is_empty());
    assert_eq!(response.scope, "series:detail");
}

#[tokio::test]
async fn test_execute_plugin_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let fake_id = uuid::Uuid::new_v4();
    let body = json!({
        "action": "ping"
    });
    let request = post_json_request_with_auth(
        &format!("/api/v1/plugins/{}/execute", fake_id),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_execute_plugin_invalid_method() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a plugin first
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "exec_test_plugin",
        "displayName": "Exec Test",
        "command": "node"
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (_, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    let created = created.unwrap().plugin;

    // Try to execute with invalid action
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "action": "invalid_action"
    });
    let request = post_json_request_with_auth(
        &format!("/api/v1/plugins/{}/execute", created.id),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    // Invalid action should result in 422 (unprocessable entity) due to deserialization failure
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_execute_plugin_disabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a disabled plugin
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "disabled_plugin",
        "displayName": "Disabled Plugin",
        "command": "node",
        "enabled": false
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (_, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    let created = created.unwrap().plugin;

    // Try to execute
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "action": "ping"
    });
    let request = post_json_request_with_auth(
        &format!("/api/v1/plugins/{}/execute", created.id),
        &body,
        &token,
    );
    let (status, response): (StatusCode, Option<ExecutePluginResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert!(!response.success);
    assert!(response.error.as_ref().unwrap().contains("disabled"));
}

#[tokio::test]
async fn test_preview_series_metadata_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let fake_series_id = uuid::Uuid::new_v4();
    let fake_plugin_id = uuid::Uuid::new_v4();
    let body = json!({
        "pluginId": fake_plugin_id.to_string(),
        "externalId": "12345"
    });
    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata/preview", fake_series_id),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_apply_series_metadata_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let fake_series_id = uuid::Uuid::new_v4();
    let fake_plugin_id = uuid::Uuid::new_v4();
    let body = json!({
        "pluginId": fake_plugin_id.to_string(),
        "externalId": "12345"
    });
    let request = post_json_request_with_auth(
        &format!("/api/v1/series/{}/metadata/apply", fake_series_id),
        &body,
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_preview_series_metadata_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let fake_series_id = uuid::Uuid::new_v4();
    let body = json!({
        "pluginId": uuid::Uuid::new_v4().to_string(),
        "externalId": "12345"
    });
    let request = common::http::post_json_request(
        &format!("/api/v1/series/{}/metadata/preview", fake_series_id),
        &body,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_apply_series_metadata_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let fake_series_id = uuid::Uuid::new_v4();
    let body = json!({
        "pluginId": uuid::Uuid::new_v4().to_string(),
        "externalId": "12345"
    });
    let request = common::http::post_json_request(
        &format!("/api/v1/series/{}/metadata/apply", fake_series_id),
        &body,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
