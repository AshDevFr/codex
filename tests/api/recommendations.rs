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
// Get Recommendations Tests (pure DB read, task auto-triggering)
// =============================================================================

#[tokio::test]
async fn test_get_recommendations_returns_empty_and_triggers_task() {
    // When a recommendation plugin is enabled and connected but no cached data exists,
    // GET should return 200 with empty recommendations and auto-trigger a refresh task.
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

    // GET recommendations — no cached data, should return empty list with task status
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/user/recommendations", &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("Expected response body");
    assert!(body["recommendations"].as_array().unwrap().is_empty());
    assert_eq!(body["pluginName"], "AniList Recommendations");
    // Should have auto-triggered a task
    assert_eq!(body["taskStatus"], "pending");
    assert!(body.get("taskId").is_some());
}

#[tokio::test]
async fn test_get_recommendations_returns_cached_data() {
    // When cached recommendation data exists in user_plugin_data, GET returns it.
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

    let instance = UserPluginsRepository::get_by_user_and_plugin(&db, user_id, plugin_id)
        .await
        .unwrap()
        .unwrap();

    // Store cached recommendations in user_plugin_data
    let cached_data = json!({
        "recommendations": [{
            "externalId": "99",
            "title": "Cached Manga",
            "score": 0.8,
            "reason": "From cache"
        }],
        "generatedAt": "2026-02-11T10:00:00Z",
        "cached": true
    });
    codex::db::repositories::UserPluginDataRepository::set(
        &db,
        instance.id,
        "recommendations",
        cached_data,
        None,
    )
    .await
    .unwrap();

    // GET recommendations — should return cached data
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/user/recommendations", &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("Expected response body");
    let recs = body["recommendations"].as_array().unwrap();
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0]["title"], "Cached Manga");
    assert!(body["cached"].as_bool().unwrap());
}

#[tokio::test]
async fn test_get_recommendations_enabled_but_not_connected() {
    // When a recommendation plugin is enabled but not connected (no OAuth tokens),
    // GET should still return 200 with empty list (pure DB read, no plugin spawn).
    ensure_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id =
        create_recommendation_plugin(&db, "recommendations-anilist", "AniList Recommendations")
            .await;

    // Enable the plugin but don't set OAuth tokens
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    // GET recommendations — returns empty list (no cached data), auto-triggers task
    let app = create_test_router(state.clone()).await;
    let request = get_request_with_auth("/api/v1/user/recommendations", &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("Expected response body");
    assert!(body["recommendations"].as_array().unwrap().is_empty());
}

// =============================================================================
// Dismiss Recommendation Tests
// =============================================================================

#[tokio::test]
async fn test_dismiss_recommendation_non_blocking() {
    // When a recommendation plugin is enabled and connected, dismiss should
    // return 200 immediately (non-blocking) and enqueue an async task.
    ensure_test_encryption_key();
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (user_id, token) = create_user_and_token(&db, &state, "testuser").await;

    let plugin_id =
        create_recommendation_plugin(&db, "recommendations-anilist", "AniList Recommendations")
            .await;

    // Enable and connect the plugin
    let app = create_test_router(state.clone()).await;
    let request = post_request_with_auth(
        &format!("/api/v1/user/plugins/{}/enable", plugin_id),
        &token,
    );
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

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

    // Store some cached recommendations first
    let cached_data = json!({
        "recommendations": [
            {
                "externalId": "12345",
                "title": "To Dismiss",
                "score": 0.8,
                "reason": "test"
            },
            {
                "externalId": "67890",
                "title": "To Keep",
                "score": 0.7,
                "reason": "test"
            }
        ],
        "generatedAt": "2026-02-11T10:00:00Z",
        "cached": true
    });
    codex::db::repositories::UserPluginDataRepository::set(
        &db,
        instance.id,
        "recommendations",
        cached_data,
        None,
    )
    .await
    .unwrap();

    // Dismiss recommendation 12345 — should return 200 immediately
    let body = json!({"reason": "not_interested"});
    let app = create_test_router(state.clone()).await;
    let request =
        post_json_request_with_auth("/api/v1/user/recommendations/12345/dismiss", &body, &token);
    let (status, response): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let body = response.expect("Expected response body");
    assert!(body["dismissed"].as_bool().unwrap());

    // Verify the cached data was updated (12345 removed, 67890 still there)
    let cached =
        codex::db::repositories::UserPluginDataRepository::get(&db, instance.id, "recommendations")
            .await
            .unwrap()
            .expect("Cached data should still exist");
    let recs = cached.data["recommendations"].as_array().unwrap();
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0]["externalId"], "67890");
}

#[tokio::test]
async fn test_dismiss_recommendation_without_reason() {
    // Dismiss should accept an empty body (reason is optional)
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    // No plugin enabled — should return 404, but validates the request is accepted
    let body = json!({});
    let app = create_test_router(state.clone()).await;
    let request =
        post_json_request_with_auth("/api/v1/user/recommendations/12345/dismiss", &body, &token);
    let (status, _): (StatusCode, Option<serde_json::Value>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_dismiss_recommendation_various_reasons() {
    // Verify all valid reason strings are accepted by the endpoint
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "testuser").await;

    for reason in &["not_interested", "already_read", "already_owned"] {
        let body = json!({"reason": reason});
        let app = create_test_router(state.clone()).await;
        let request = post_json_request_with_auth(
            "/api/v1/user/recommendations/test-id/dismiss",
            &body,
            &token,
        );
        let (status, _): (StatusCode, Option<serde_json::Value>) =
            make_json_request(app, request).await;

        // Will be 404 since no plugin enabled, but validates request parsing
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "reason '{}' should be accepted",
            reason
        );
    }
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
