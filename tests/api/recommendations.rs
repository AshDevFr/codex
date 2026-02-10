//! Recommendations API endpoint tests
//!
//! Tests for recommendation endpoints:
//! - GET /api/v1/user/recommendations - Get personalized recommendations
//! - POST /api/v1/user/recommendations/refresh - Refresh recommendations
//! - POST /api/v1/user/recommendations/:external_id/dismiss - Dismiss a recommendation

#[path = "../common/mod.rs"]
mod common;

use common::db::setup_test_db;
use common::fixtures::create_test_user;
use common::http::{
    create_test_auth_state, create_test_router, generate_test_token, get_request,
    get_request_with_auth, make_json_request, post_json_request_with_auth, post_request_with_auth,
};
use hyper::StatusCode;
use serde_json::json;

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

/// Create a user-type plugin with a recommendation provider manifest
async fn create_recommendation_plugin(
    db: &sea_orm::DatabaseConnection,
    name: &str,
    display_name: &str,
) -> uuid::Uuid {
    let plugin = PluginsRepository::create(
        db,
        name,
        display_name,
        Some("A test recommendation plugin"),
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

    // Set manifest with recommendation provider capability
    let manifest = json!({
        "name": name,
        "displayName": display_name,
        "version": "1.0.0",
        "protocolVersion": "1.0",
        "pluginType": "user",
        "capabilities": {
            "userRecommendationProvider": true
        },
        "userDescription": "Get personalized recommendations"
    });
    PluginsRepository::update_manifest(db, plugin.id, Some(manifest))
        .await
        .unwrap();

    plugin.id
}

/// Create a user-type plugin WITHOUT recommendation capability
async fn create_non_recommendation_plugin(
    db: &sea_orm::DatabaseConnection,
    name: &str,
    display_name: &str,
) -> uuid::Uuid {
    let plugin = PluginsRepository::create(
        db,
        name,
        display_name,
        Some("A non-recommendation plugin"),
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

    plugin.id
}

// =============================================================================
// Authentication Tests
// =============================================================================

#[tokio::test]
async fn test_get_recommendations_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;

    let request = get_request("/api/v1/user/recommendations");
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_refresh_recommendations_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;

    let request = common::http::post_request("/api/v1/user/recommendations/refresh");
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_dismiss_recommendation_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state.clone()).await;

    let body = json!({});
    let request =
        common::http::post_json_request("/api/v1/user/recommendations/12345/dismiss", &body);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// =============================================================================
// No Plugin Tests
// =============================================================================

#[tokio::test]
async fn test_get_recommendations_no_plugin_enabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/user/recommendations", &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_refresh_recommendations_no_plugin_enabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth("/api/v1/user/recommendations/refresh", &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_dismiss_recommendations_no_plugin_enabled() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let body = json!({});
    let app = create_test_router(state.clone()).await;
    let request =
        post_json_request_with_auth("/api/v1/user/recommendations/12345/dismiss", &body, &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// =============================================================================
// Non-Recommendation Plugin Tests
// =============================================================================

#[tokio::test]
async fn test_get_recommendations_non_rec_plugin_returns_404() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    // Create and enable a non-recommendation plugin
    let plugin_id = create_non_recommendation_plugin(&db, "sync-only", "Sync Only Plugin").await;
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Should still return 404 since no *recommendation* plugin is enabled
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/user/recommendations", &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// =============================================================================
// Refresh Recommendations Tests (enqueue task)
// =============================================================================

#[tokio::test]
async fn test_refresh_recommendations_enqueues_task() {
    ensure_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id =
        create_recommendation_plugin(&db, "recommendations-anilist", "AniList Recommendations")
            .await;

    // Enable the plugin
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
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

    // Refresh recommendations
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth("/api/v1/user/recommendations/refresh", &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("Expected response body");
    assert!(body.get("taskId").is_some());
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("AniList Recommendations")
    );
}

// =============================================================================
// User Isolation Tests
// =============================================================================

#[tokio::test]
async fn test_recommendations_user_isolation() {
    ensure_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_a_id, token_a) = create_user_and_token(&db, &state, "usera").await;
    let (_, token_b) = create_user_and_token(&db, &state, "userb").await;

    let plugin_id =
        create_recommendation_plugin(&db, "recommendations-anilist", "AniList Recommendations")
            .await;

    // User A enables and connects the plugin
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token_a,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    let instance = UserPluginsRepository::get_by_user_and_plugin(&db, user_a_id, plugin_id)
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

    // User A can refresh
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth("/api/v1/user/recommendations/refresh", &token_a);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // User B cannot see recommendations (no plugin enabled)
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/user/recommendations", &token_b);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// =============================================================================
// Disabled Plugin Tests
// =============================================================================

#[tokio::test]
async fn test_recommendations_disabled_plugin_returns_404() {
    ensure_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id =
        create_recommendation_plugin(&db, "recommendations-anilist", "AniList Recommendations")
            .await;

    // Enable the plugin
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Disable the plugin
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/disable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // Should return 404 since plugin is disabled
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/user/recommendations", &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// =============================================================================
// Task Deduplication Tests
// =============================================================================

#[tokio::test]
async fn test_refresh_recommendations_deduplication() {
    ensure_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id =
        create_recommendation_plugin(&db, "recommendations-anilist", "AniList Recommendations")
            .await;

    // Enable the plugin
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
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

    // First refresh — should succeed
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth("/api/v1/user/recommendations/refresh", &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    let body = response.expect("Expected response body");
    assert!(body.get("taskId").is_some());

    // Second refresh — should return 409 Conflict
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth("/api/v1/user/recommendations/refresh", &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let body = response.expect("Expected error body");
    assert_eq!(
        body["message"],
        "Recommendation refresh already in progress"
    );
}
