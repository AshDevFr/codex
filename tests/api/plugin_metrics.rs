//! Plugin metrics API endpoint tests

#[path = "../common/mod.rs"]
mod common;

use codex::api::routes::v1::dto::PluginMetricsResponse;
use common::db::setup_test_db;
use common::fixtures::create_test_user;
use common::http::{
    create_test_app_state, create_test_router_with_app_state, generate_test_token, get_request,
    get_request_with_auth, make_json_request,
};
use hyper::StatusCode;

// ============================================================
// GET /api/v1/metrics/plugins tests
// ============================================================

#[tokio::test]
async fn test_get_plugin_metrics_empty() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;

    // Create admin user and get token
    let user = create_test_user(
        "admin",
        "admin@example.com",
        &codex::utils::password::hash_password("admin123").unwrap(),
        true,
    );
    let created_user = codex::db::repositories::UserRepository::create(&db, &user)
        .await
        .unwrap();
    let token = generate_test_token(&state, &created_user);

    let app = create_test_router_with_app_state(state.clone());
    let request = get_request_with_auth("/api/v1/metrics/plugins", &token);

    let (status, response): (StatusCode, Option<PluginMetricsResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");
    assert_eq!(response.summary.total_plugins, 0);
    assert_eq!(response.summary.total_requests, 0);
    assert!(response.plugins.is_empty());
}

#[tokio::test]
async fn test_get_plugin_metrics_with_data() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;

    // Create admin user and get token
    let user = create_test_user(
        "admin",
        "admin@example.com",
        &codex::utils::password::hash_password("admin123").unwrap(),
        true,
    );
    let created_user = codex::db::repositories::UserRepository::create(&db, &user)
        .await
        .unwrap();
    let token = generate_test_token(&state, &created_user);

    // Record some metrics
    let plugin_id = uuid::Uuid::new_v4();
    state
        .plugin_metrics_service
        .record_success(plugin_id, "Test Plugin", "search", 100)
        .await;
    state
        .plugin_metrics_service
        .record_success(plugin_id, "Test Plugin", "search", 200)
        .await;
    state
        .plugin_metrics_service
        .record_failure(
            plugin_id,
            "Test Plugin",
            "get_metadata",
            300,
            Some("TIMEOUT"),
        )
        .await;

    let app = create_test_router_with_app_state(state.clone());
    let request = get_request_with_auth("/api/v1/metrics/plugins", &token);

    let (status, response): (StatusCode, Option<PluginMetricsResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::OK);
    let response = response.expect("Response should be present");

    // Check summary
    assert_eq!(response.summary.total_plugins, 1);
    assert_eq!(response.summary.total_requests, 3);
    assert_eq!(response.summary.total_success, 2);
    assert_eq!(response.summary.total_failed, 1);

    // Check individual plugin metrics
    assert_eq!(response.plugins.len(), 1);
    let plugin = &response.plugins[0];
    assert_eq!(plugin.plugin_id, plugin_id);
    assert_eq!(plugin.plugin_name, "Test Plugin");
    assert_eq!(plugin.requests_total, 3);
    assert_eq!(plugin.requests_success, 2);
    assert_eq!(plugin.requests_failed, 1);

    // Check method breakdown
    let by_method = plugin
        .by_method
        .as_ref()
        .expect("Should have method breakdown");
    assert!(by_method.contains_key("search"));
    assert!(by_method.contains_key("get_metadata"));
    let search = by_method.get("search").unwrap();
    assert_eq!(search.requests_total, 2);
    assert_eq!(search.requests_success, 2);

    // Check failure counts
    let failures = plugin
        .failure_counts
        .as_ref()
        .expect("Should have failure counts");
    assert_eq!(failures.get("TIMEOUT"), Some(&1));
}

#[tokio::test]
async fn test_get_plugin_metrics_requires_auth() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db).await;
    let app = create_test_router_with_app_state(state);

    let request = get_request("/api/v1/metrics/plugins");
    let (status, _): (StatusCode, Option<PluginMetricsResponse>) =
        make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_plugin_metrics_allowed_for_reader() {
    let (db, _temp_dir) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;

    // Create regular user (Reader role) and get token
    let user = create_test_user(
        "reader",
        "reader@example.com",
        &codex::utils::password::hash_password("reader123").unwrap(),
        false, // not admin = Reader role
    );
    let created_user = codex::db::repositories::UserRepository::create(&db, &user)
        .await
        .unwrap();
    let token = generate_test_token(&state, &created_user);

    let app = create_test_router_with_app_state(state);
    let request = get_request_with_auth("/api/v1/metrics/plugins", &token);

    let (status, response): (StatusCode, Option<PluginMetricsResponse>) =
        make_json_request(app, request).await;

    // Reader should have libraries:read permission which is required
    assert_eq!(status, StatusCode::OK);
    assert!(response.is_some());
}
