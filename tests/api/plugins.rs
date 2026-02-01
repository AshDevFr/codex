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
        "name": "test-plugin",
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
    assert_eq!(response.plugin.name, "test-plugin");
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
        "name": "minimal-plugin",
        "displayName": "Minimal",
        "command": "node"  // Must be in allowed commands list
    });

    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (status, response): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CREATED);
    let response = response.expect("Expected response body");
    assert_eq!(response.plugin.name, "minimal-plugin");
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
        "name": "test-plugin",
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
        "name": "test-plugin",
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
        "name": "duplicate-test",
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
        "name": "duplicate-test",
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
        "name": "get-test-plugin",
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
    assert_eq!(response.name, "get-test-plugin");
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
        "name": "update-test-plugin",
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
    assert_eq!(response.name, "update-test-plugin"); // Name shouldn't change
}

#[tokio::test]
async fn test_update_plugin_permissions() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a plugin with initial permissions
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "perm-test-plugin",
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

#[tokio::test]
async fn test_update_plugin_clear_search_template() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a plugin with a search template
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "search-template-test",
        "displayName": "Search Template Test",
        "command": "node",
        "searchQueryTemplate": "{{clean metadata.title}}"
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (status, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    let created = created.unwrap().plugin;
    assert_eq!(
        created.search_query_template,
        Some("{{clean metadata.title}}".to_string())
    );

    // Clear the search template by setting it to null
    let app = create_test_router(state.clone()).await;
    let update_body = json!({
        "searchQueryTemplate": null
    });
    let request = patch_json_request_with_auth(
        &format!("/api/v1/admin/plugins/{}", created.id),
        &update_body,
        &token,
    );
    let (status, response): (StatusCode, Option<PluginDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert_eq!(response.search_query_template, None);
}

#[tokio::test]
async fn test_update_plugin_clear_search_template_with_empty_string() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a plugin with a search template
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "search-template-empty-test",
        "displayName": "Search Template Empty Test",
        "command": "node",
        "searchQueryTemplate": "{{metadata.title}}"
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (status, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    let created = created.unwrap().plugin;
    assert_eq!(
        created.search_query_template,
        Some("{{metadata.title}}".to_string())
    );

    // Clear the search template by setting it to empty string
    let app = create_test_router(state.clone()).await;
    let update_body = json!({
        "searchQueryTemplate": ""
    });
    let request = patch_json_request_with_auth(
        &format!("/api/v1/admin/plugins/{}", created.id),
        &update_body,
        &token,
    );
    let (status, response): (StatusCode, Option<PluginDto>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert_eq!(response.search_query_template, None);
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
        "name": "delete-test-plugin",
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
        "name": "enable-test-plugin",
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
        "name": "disable-test-plugin",
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
        "name": "health-test-plugin",
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
    assert_eq!(response.health.name, "health-test-plugin");
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
        "name": "reset-test-plugin",
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
        "name": "invalid-cmd-plugin",
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
            "name": format!("list-test-plugin-{}", i),
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
        "name": "exec-test-plugin",
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
        "name": "disabled-plugin",
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

// =============================================================================
// Permission-Based Access Tests (Phase 8)
// =============================================================================

/// Create a maintainer user and return a JWT token.
/// Maintainers have SeriesWrite permission but not PluginsManage.
async fn create_maintainer_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AppState,
) -> String {
    use codex::api::permissions::UserRole;
    use codex::db::entities::users;
    use sea_orm::ActiveModelTrait;

    let password_hash = password::hash_password("maintainer123").unwrap();
    let maintainer = users::ActiveModel {
        id: sea_orm::ActiveValue::Set(uuid::Uuid::new_v4()),
        username: sea_orm::ActiveValue::Set("maintainer".to_string()),
        email: sea_orm::ActiveValue::Set("maintainer@example.com".to_string()),
        password_hash: sea_orm::ActiveValue::Set(password_hash),
        role: sea_orm::ActiveValue::Set(UserRole::Maintainer.to_string()),
        is_active: sea_orm::ActiveValue::Set(true),
        email_verified: sea_orm::ActiveValue::Set(true),
        permissions: sea_orm::ActiveValue::Set(serde_json::json!([])),
        created_at: sea_orm::ActiveValue::Set(chrono::Utc::now()),
        updated_at: sea_orm::ActiveValue::Set(chrono::Utc::now()),
        last_login_at: sea_orm::ActiveValue::Set(None),
    };
    let created = maintainer.insert(db).await.unwrap();
    generate_test_token(state, &created)
}

#[tokio::test]
async fn test_plugin_crud_requires_plugins_manage_permission() {
    // A maintainer (who has SeriesWrite but NOT PluginsManage) should NOT be able
    // to create, update, or delete plugins
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let maintainer_token = create_maintainer_and_token(&db, &state).await;

    // Try to list plugins - should fail (requires PluginsManage)
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/admin/plugins", &maintainer_token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "Maintainer should not list plugins"
    );

    // Try to create a plugin - should fail
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "test-plugin",
        "displayName": "Test Plugin",
        "command": "node"
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &maintainer_token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "Maintainer should not create plugins"
    );
}

#[tokio::test]
async fn test_reader_cannot_access_plugin_actions() {
    // A reader (no SeriesWrite) should not see plugin actions
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let reader_token = create_user_and_token(&db, &state).await;

    // Get plugin actions - reader can view the actions endpoint (LibrariesRead)
    // but won't see any plugins because they lack SeriesWrite
    let app = create_test_router(state.clone()).await;
    let request =
        get_request_with_auth("/api/v1/plugins/actions?scope=series:detail", &reader_token);
    let (status, response): (
        StatusCode,
        Option<codex::api::routes::v1::dto::PluginActionsResponse>,
    ) = make_json_request(app, request).await;

    // Reader has LibrariesRead, so they can access the endpoint
    assert_eq!(status, StatusCode::OK);
    // But since they don't have SeriesWrite, the actions list should be empty
    // (no plugins will pass the permission filter)
    let response = response.expect("Expected response body");
    assert!(
        response.actions.is_empty(),
        "Reader should not see any plugin actions (no SeriesWrite permission)"
    );
}

#[tokio::test]
async fn test_maintainer_can_use_plugin_actions() {
    // A maintainer (has SeriesWrite) should be able to access plugin actions
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let maintainer_token = create_maintainer_and_token(&db, &state).await;

    // Get plugin actions - maintainer can view
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        "/api/v1/plugins/actions?scope=series:detail",
        &maintainer_token,
    );
    let (status, response): (
        StatusCode,
        Option<codex::api::routes::v1::dto::PluginActionsResponse>,
    ) = make_json_request(app, request).await;

    // Maintainer has LibrariesRead, so they can access the endpoint
    assert_eq!(status, StatusCode::OK);
    // The actions list will be empty because there are no plugins configured,
    // but the endpoint is accessible (unlike for readers when plugins exist)
    let response = response.expect("Expected response body");
    assert_eq!(response.scope, "series:detail");
}

// =============================================================================
// Search Title Endpoint Tests
// =============================================================================

use codex::api::routes::v1::dto::SearchTitleResponse;
use codex::db::repositories::{LibraryRepository, SeriesRepository};
use codex::db::ScanningStrategy;

#[tokio::test]
async fn test_get_search_title_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let fake_series_id = uuid::Uuid::new_v4();
    let fake_plugin_id = uuid::Uuid::new_v4();
    let request = common::http::get_request(&format!(
        "/api/v1/series/{}/metadata/search-title?pluginId={}",
        fake_series_id, fake_plugin_id
    ));
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_search_title_series_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    let fake_series_id = uuid::Uuid::new_v4();
    let fake_plugin_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(
        &format!(
            "/api/v1/series/{}/metadata/search-title?pluginId={}",
            fake_series_id, fake_plugin_id
        ),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_search_title_plugin_not_found() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Test Series", None)
        .await
        .unwrap();

    // Try to get search title with non-existent plugin
    let app = create_test_router(state.clone()).await;
    let fake_plugin_id = uuid::Uuid::new_v4();
    let request = get_request_with_auth(
        &format!(
            "/api/v1/series/{}/metadata/search-title?pluginId={}",
            series.id, fake_plugin_id
        ),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_search_title_no_preprocessing() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "My Test Series", None)
        .await
        .unwrap();

    // Create a plugin without preprocessing rules
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "no-preprocess-plugin",
        "displayName": "No Preprocess Plugin",
        "command": "node",
        "permissions": ["metadata:read"]
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (_, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    let plugin = created.unwrap().plugin;

    // Get search title
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        &format!(
            "/api/v1/series/{}/metadata/search-title?pluginId={}",
            series.id, plugin.id
        ),
        &token,
    );
    let (status, response): (StatusCode, Option<SearchTitleResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert_eq!(response.original_title, "My Test Series");
    assert_eq!(response.search_title, "My Test Series");
    assert_eq!(response.rules_applied, 0);
}

#[tokio::test]
async fn test_get_search_title_with_preprocessing_rules() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a library and series with (Digital) suffix
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "One Piece (Digital)", None)
        .await
        .unwrap();

    // Create a plugin with preprocessing rules to remove (Digital) suffix
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "preprocess-plugin",
        "displayName": "Preprocess Plugin",
        "command": "node",
        "permissions": ["metadata:read"],
        "searchPreprocessingRules": [
            {
                "pattern": "\\s*\\(Digital\\)$",
                "replacement": "",
                "enabled": true
            }
        ]
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (status, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    let plugin = created.unwrap().plugin;

    // Get search title
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        &format!(
            "/api/v1/series/{}/metadata/search-title?pluginId={}",
            series.id, plugin.id
        ),
        &token,
    );
    let (status, response): (StatusCode, Option<SearchTitleResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert_eq!(response.original_title, "One Piece (Digital)");
    assert_eq!(response.search_title, "One Piece");
    assert_eq!(response.rules_applied, 1);
}

#[tokio::test]
async fn test_get_search_title_with_search_query_template() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Naruto", None)
        .await
        .unwrap();

    // Create a plugin with a search query template
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "template-plugin",
        "displayName": "Template Plugin",
        "command": "node",
        "permissions": ["metadata:read"],
        "searchQueryTemplate": "{{metadata.title}} manga"
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (status, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    let plugin = created.unwrap().plugin;

    // Get search title
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        &format!(
            "/api/v1/series/{}/metadata/search-title?pluginId={}",
            series.id, plugin.id
        ),
        &token,
    );
    let (status, response): (StatusCode, Option<SearchTitleResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert_eq!(response.original_title, "Naruto");
    // Template should render: "Naruto manga"
    assert_eq!(response.search_title, "Naruto manga");
}

// =============================================================================
// Unified Series Context Integration Tests (Phase 4)
// =============================================================================

use codex::db::repositories::{GenreRepository, SeriesMetadataRepository, TagRepository};
use codex::services::metadata::preprocessing::context::SeriesContextBuilder;

#[tokio::test]
async fn test_series_context_builder_full_flow() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create a library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "One Piece", None)
        .await
        .unwrap();

    // Update metadata with full details using the replace method
    // (metadata is automatically created when series is created)
    SeriesMetadataRepository::replace(
        &db,
        series.id,
        Some("One Piece".to_string()),                // title_sort
        Some("A pirate adventure story".to_string()), // summary
        Some("Shueisha".to_string()),                 // publisher
        Some(1997),                                   // year
        Some("rtl".to_string()),                      // reading_direction
    )
    .await
    .unwrap();

    // Add genres
    GenreRepository::add_genre_to_series(&db, series.id, "Action")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series.id, "Adventure")
        .await
        .unwrap();
    GenreRepository::add_genre_to_series(&db, series.id, "Comedy")
        .await
        .unwrap();

    // Add tags
    TagRepository::add_tag_to_series(&db, series.id, "pirates")
        .await
        .unwrap();
    TagRepository::add_tag_to_series(&db, series.id, "treasure")
        .await
        .unwrap();

    // Build the series context using the new builder
    let context = SeriesContextBuilder::new(series.id)
        .build(&db)
        .await
        .unwrap();

    // Verify context fields
    assert_eq!(context.series_id, Some(series.id));
    assert_eq!(context.metadata.title, Some("One Piece".to_string()));
    assert_eq!(context.metadata.publisher, Some("Shueisha".to_string()));
    assert_eq!(context.metadata.year, Some(1997));
    assert_eq!(context.metadata.reading_direction, Some("rtl".to_string()));
    // Verify genres and tags are populated
    assert_eq!(context.metadata.genres.len(), 3);
    assert!(context.metadata.genres.contains(&"Action".to_string()));
    assert!(context.metadata.genres.contains(&"Adventure".to_string()));
    assert!(context.metadata.genres.contains(&"Comedy".to_string()));
    assert_eq!(context.metadata.tags.len(), 2);
    assert!(context.metadata.tags.contains(&"pirates".to_string()));
    assert!(context.metadata.tags.contains(&"treasure".to_string()));

    // Serialize to JSON and verify camelCase field names
    let json = serde_json::to_value(&context).unwrap();

    // Top-level fields should be camelCase
    assert!(json.get("seriesId").is_some(), "seriesId should exist");
    assert!(json.get("bookCount").is_some(), "bookCount should exist");
    assert!(
        json.get("series_id").is_none(),
        "series_id should not exist"
    );
    assert!(
        json.get("book_count").is_none(),
        "book_count should not exist"
    );

    // Metadata fields should be camelCase
    let metadata = json.get("metadata").unwrap();
    assert!(
        metadata.get("titleSort").is_some(),
        "titleSort should exist"
    );
    assert!(
        metadata.get("readingDirection").is_some(),
        "readingDirection should exist"
    );
    assert!(
        metadata.get("title_sort").is_none(),
        "title_sort should not exist"
    );

    // Verify genres and tags arrays are included
    assert_eq!(metadata["genres"].as_array().unwrap().len(), 3);
    assert_eq!(metadata["tags"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_series_context_template_rendering() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_and_token(&db, &state).await;

    // Create a library and series
    let library =
        LibraryRepository::create(&db, "Test Library", "/test", ScanningStrategy::Default)
            .await
            .unwrap();
    let series = SeriesRepository::create(&db, library.id, "Dragon Ball", None)
        .await
        .unwrap();

    // Update metadata with year and publisher
    // (metadata is automatically created when series is created)
    SeriesMetadataRepository::replace(
        &db,
        series.id,
        None,                         // title_sort
        None,                         // summary
        Some("Shueisha".to_string()), // publisher
        Some(1984),                   // year
        None,                         // reading_direction
    )
    .await
    .unwrap();

    // Add genre
    GenreRepository::add_genre_to_series(&db, series.id, "Action")
        .await
        .unwrap();

    // Create a plugin with a template that uses camelCase fields
    // Template: "{{metadata.title}} ({{metadata.year}}) - {{metadata.publisher}}"
    let app = create_test_router(state.clone()).await;
    let body = json!({
        "name": "context-test-plugin",
        "displayName": "Context Test Plugin",
        "command": "node",
        "permissions": ["metadata:read"],
        "searchQueryTemplate": "{{metadata.title}} ({{metadata.year}}) {{metadata.publisher}}"
    });
    let request = post_json_request_with_auth("/api/v1/admin/plugins", &body, &token);
    let (status, created): (StatusCode, Option<PluginStatusResponse>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::CREATED);
    let plugin = created.unwrap().plugin;

    // Get search title - should render the template with camelCase field access
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth(
        &format!(
            "/api/v1/series/{}/metadata/search-title?pluginId={}",
            series.id, plugin.id
        ),
        &token,
    );
    let (status, response): (StatusCode, Option<SearchTitleResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Expected response body");
    assert_eq!(response.original_title, "Dragon Ball");
    // Template should render: "Dragon Ball (1984) Shueisha"
    assert_eq!(response.search_title, "Dragon Ball (1984) Shueisha");
}

#[tokio::test]
async fn test_series_context_field_access_dual_path_support() {
    use codex::services::metadata::preprocessing::context::{
        FieldValue, MetadataContext, SeriesContext,
    };

    // Create a context with various fields
    let metadata = MetadataContext {
        title: Some("Test Series".to_string()),
        title_sort: Some("Test Series".to_string()),
        age_rating: Some(13),
        reading_direction: Some("rtl".to_string()),
        total_book_count: Some(50),
        genres: vec!["Action".to_string(), "Drama".to_string()],
        tags: vec!["fantasy".to_string()],
        title_lock: true,
        ..Default::default()
    };

    let context = SeriesContext::new()
        .book_count(10)
        .metadata(metadata)
        .external_id("plugin:test", "12345");

    // Test camelCase paths work
    assert_eq!(
        context.get_field("bookCount"),
        Some(FieldValue::Number(10.0))
    );
    assert_eq!(
        context.get_field("metadata.titleSort"),
        Some(FieldValue::String("Test Series".to_string()))
    );
    assert_eq!(
        context.get_field("metadata.ageRating"),
        Some(FieldValue::Number(13.0))
    );
    assert_eq!(
        context.get_field("metadata.readingDirection"),
        Some(FieldValue::String("rtl".to_string()))
    );
    assert_eq!(
        context.get_field("metadata.totalBookCount"),
        Some(FieldValue::Number(50.0))
    );
    assert_eq!(
        context.get_field("metadata.titleLock"),
        Some(FieldValue::Bool(true))
    );
    assert_eq!(
        context.get_field("externalIds.plugin:test"),
        Some(FieldValue::String("12345".to_string()))
    );
    assert_eq!(
        context.get_field("externalIds.count"),
        Some(FieldValue::Number(1.0))
    );

    // Test snake_case paths also work (backwards compatibility)
    assert_eq!(
        context.get_field("book_count"),
        Some(FieldValue::Number(10.0))
    );
    assert_eq!(
        context.get_field("metadata.title_sort"),
        Some(FieldValue::String("Test Series".to_string()))
    );
    assert_eq!(
        context.get_field("metadata.age_rating"),
        Some(FieldValue::Number(13.0))
    );
    assert_eq!(
        context.get_field("external_ids.plugin:test"),
        Some(FieldValue::String("12345".to_string()))
    );
    assert_eq!(
        context.get_field("external_ids.count"),
        Some(FieldValue::Number(1.0))
    );

    // Test genres and tags field access
    let genres = context.get_field("metadata.genres");
    assert!(matches!(genres, Some(FieldValue::Array(ref arr)) if arr.len() == 2));
    let tags = context.get_field("metadata.tags");
    assert!(matches!(tags, Some(FieldValue::Array(ref arr)) if arr.len() == 1));
}
