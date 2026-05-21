//! Integration tests for filter preset endpoints.

#[path = "../common/mod.rs"]
mod common;

use codex::db::repositories::UserRepository;
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use serde_json::{Value, json};

async fn create_user_and_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
    username: &str,
) -> (uuid::Uuid, String) {
    let password_hash = password::hash_password("password123").unwrap();
    let user = create_test_user(
        username,
        &format!("{username}@example.com"),
        &password_hash,
        false,
    );
    let created = UserRepository::create(db, &user).await.unwrap();
    let token = state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap();
    (created.id, token)
}

fn sample_books_condition() -> Value {
    json!({
        "allOf": [
            { "title": { "operator": "contains", "value": "one punch" } }
        ]
    })
}

fn sample_series_condition() -> Value {
    json!({
        "allOf": [
            { "title": { "operator": "contains", "value": "ABC" } }
        ]
    })
}

// ============================================================================
// Auth required
// ============================================================================

#[tokio::test]
async fn test_list_requires_auth() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let request = get_request("/api/v1/filter-presets");
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_create_requires_auth() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let app = create_test_router(state).await;

    let body = json!({
        "name": "x",
        "scope": "search",
        "target": "books",
        "condition": sample_books_condition(),
    });
    let request = post_json_request("/api/v1/filter-presets", &body);
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Create
// ============================================================================

#[tokio::test]
async fn test_create_preset_success() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "alice").await;
    let app = create_test_router(state).await;

    let body = json!({
        "name": "Unread CBZ",
        "scope": "search",
        "target": "books",
        "condition": sample_books_condition(),
        "query": "one punch",
        "sort": "year:desc",
    });
    let request = post_request_with_auth_json("/api/v1/filter-presets", &token, &body.to_string());
    let (status, response): (StatusCode, Option<Value>) = make_json_request(app, request).await;

    assert_eq!(status, StatusCode::CREATED);
    let preset = response.unwrap();
    assert_eq!(preset["name"], "Unread CBZ");
    assert_eq!(preset["scope"], "search");
    assert_eq!(preset["target"], "books");
    assert_eq!(preset["query"], "one punch");
    assert_eq!(preset["sort"], "year:desc");
    assert!(preset["id"].is_string());
}

#[tokio::test]
async fn test_create_rejects_unknown_scope() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "alice").await;
    let app = create_test_router(state).await;

    let body = json!({
        "name": "bad",
        "scope": "nope",
        "target": "books",
        "condition": sample_books_condition(),
    });
    let request = post_request_with_auth_json("/api/v1/filter-presets", &token, &body.to_string());
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_rejects_mismatched_condition() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "alice").await;
    let app = create_test_router(state).await;

    // bookType is a Book-only variant
    let body = json!({
        "name": "bad",
        "scope": "search",
        "target": "series",
        "condition": { "bookType": { "operator": "is", "value": "manga" } },
    });
    let request = post_request_with_auth_json("/api/v1/filter-presets", &token, &body.to_string());
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_rejects_empty_name() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "alice").await;
    let app = create_test_router(state).await;

    let body = json!({
        "name": "   ",
        "scope": "search",
        "target": "books",
        "condition": sample_books_condition(),
    });
    let request = post_request_with_auth_json("/api/v1/filter-presets", &token, &body.to_string());
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_conflict_on_duplicate_name() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "alice").await;

    let body = json!({
        "name": "MyPreset",
        "scope": "search",
        "target": "books",
        "condition": sample_books_condition(),
    });

    let app1 = create_test_router(state.clone()).await;
    let r1 = post_request_with_auth_json("/api/v1/filter-presets", &token, &body.to_string());
    let (status1, _): (StatusCode, Option<Value>) = make_json_request(app1, r1).await;
    assert_eq!(status1, StatusCode::CREATED);

    let app2 = create_test_router(state).await;
    let r2 = post_request_with_auth_json("/api/v1/filter-presets", &token, &body.to_string());
    let (status2, _): (StatusCode, Option<Value>) = make_json_request(app2, r2).await;
    assert_eq!(status2, StatusCode::CONFLICT);
}

// ============================================================================
// List
// ============================================================================

#[tokio::test]
async fn test_list_returns_only_callers_presets() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, alice_token) = create_user_and_token(&db, &state, "alice").await;
    let (_, bob_token) = create_user_and_token(&db, &state, "bob").await;

    // Alice creates a preset
    let create_body = json!({
        "name": "AlicePreset",
        "scope": "search",
        "target": "books",
        "condition": sample_books_condition(),
    });
    let app = create_test_router(state.clone()).await;
    let r = post_request_with_auth_json(
        "/api/v1/filter-presets",
        &alice_token,
        &create_body.to_string(),
    );
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::CREATED);

    // Bob sees nothing
    let app = create_test_router(state.clone()).await;
    let r = get_request_with_auth("/api/v1/filter-presets", &bob_token);
    let (status, body): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::OK);
    let presets = body.unwrap()["presets"].as_array().unwrap().clone();
    assert!(presets.is_empty(), "bob should see no presets");

    // Alice sees her own
    let app = create_test_router(state).await;
    let r = get_request_with_auth("/api/v1/filter-presets", &alice_token);
    let (status, body): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::OK);
    let presets = body.unwrap()["presets"].as_array().unwrap().clone();
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0]["name"], "AlicePreset");
}

#[tokio::test]
async fn test_list_filters_by_scope_target() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "alice").await;

    for (scope, target, name, cond) in [
        ("search", "books", "SB", sample_books_condition()),
        ("search", "series", "SS", sample_series_condition()),
        ("list", "books", "LB", sample_books_condition()),
    ] {
        let body = json!({
            "name": name,
            "scope": scope,
            "target": target,
            "condition": cond,
        });
        let app = create_test_router(state.clone()).await;
        let r = post_request_with_auth_json("/api/v1/filter-presets", &token, &body.to_string());
        let (s, _): (StatusCode, Option<Value>) = make_json_request(app, r).await;
        assert_eq!(s, StatusCode::CREATED);
    }

    let app = create_test_router(state.clone()).await;
    let r = get_request_with_auth("/api/v1/filter-presets?scope=search", &token);
    let (status, body): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::OK);
    let presets = body.unwrap()["presets"].as_array().unwrap().clone();
    assert_eq!(presets.len(), 2);
    assert!(presets.iter().all(|p| p["scope"] == "search"));

    let app = create_test_router(state).await;
    let r = get_request_with_auth("/api/v1/filter-presets?scope=search&target=books", &token);
    let (status, body): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::OK);
    let presets = body.unwrap()["presets"].as_array().unwrap().clone();
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0]["name"], "SB");
}

// ============================================================================
// Detail
// ============================================================================

#[tokio::test]
async fn test_get_other_users_preset_returns_404() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, alice_token) = create_user_and_token(&db, &state, "alice").await;
    let (_, bob_token) = create_user_and_token(&db, &state, "bob").await;

    let body = json!({
        "name": "AlicePrivate",
        "scope": "search",
        "target": "books",
        "condition": sample_books_condition(),
    });
    let app = create_test_router(state.clone()).await;
    let r = post_request_with_auth_json("/api/v1/filter-presets", &alice_token, &body.to_string());
    let (status, response): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::CREATED);
    let preset_id = response.unwrap()["id"].as_str().unwrap().to_string();

    // Bob tries to read it
    let app = create_test_router(state).await;
    let r = get_request_with_auth(&format!("/api/v1/filter-presets/{preset_id}"), &bob_token);
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_own_preset() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "alice").await;

    let body = json!({
        "name": "Mine",
        "scope": "search",
        "target": "books",
        "condition": sample_books_condition(),
    });
    let app = create_test_router(state.clone()).await;
    let r = post_request_with_auth_json("/api/v1/filter-presets", &token, &body.to_string());
    let (status, response): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::CREATED);
    let preset_id = response.unwrap()["id"].as_str().unwrap().to_string();

    let app = create_test_router(state).await;
    let r = get_request_with_auth(&format!("/api/v1/filter-presets/{preset_id}"), &token);
    let (status, body): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.unwrap()["name"], "Mine");
}

// ============================================================================
// Update
// ============================================================================

#[tokio::test]
async fn test_update_preset_success() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "alice").await;

    let body = json!({
        "name": "Original",
        "scope": "search",
        "target": "books",
        "condition": sample_books_condition(),
    });
    let app = create_test_router(state.clone()).await;
    let r = post_request_with_auth_json("/api/v1/filter-presets", &token, &body.to_string());
    let (_, response): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    let preset_id = response.unwrap()["id"].as_str().unwrap().to_string();

    let new_condition = json!({ "title": { "operator": "is", "value": "newval" } });
    let update_body = json!({
        "name": "Renamed",
        "condition": new_condition,
        "sort": "title:asc",
    });
    let app = create_test_router(state).await;
    let r = put_json_request_with_auth(
        &format!("/api/v1/filter-presets/{preset_id}"),
        &update_body,
        &token,
    );
    let (status, body): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::OK);
    let preset = body.unwrap();
    assert_eq!(preset["name"], "Renamed");
    assert_eq!(preset["sort"], "title:asc");
    // query was omitted from request -> cleared (PUT is full replacement)
    assert!(preset.get("query").map(|v| v.is_null()).unwrap_or(true));
}

#[tokio::test]
async fn test_update_other_users_preset_returns_404() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, alice_token) = create_user_and_token(&db, &state, "alice").await;
    let (_, bob_token) = create_user_and_token(&db, &state, "bob").await;

    let body = json!({
        "name": "AlicePreset",
        "scope": "search",
        "target": "books",
        "condition": sample_books_condition(),
    });
    let app = create_test_router(state.clone()).await;
    let r = post_request_with_auth_json("/api/v1/filter-presets", &alice_token, &body.to_string());
    let (_, response): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    let preset_id = response.unwrap()["id"].as_str().unwrap().to_string();

    let update_body = json!({
        "name": "Hacked",
        "condition": sample_books_condition(),
    });
    let app = create_test_router(state).await;
    let r = put_json_request_with_auth(
        &format!("/api/v1/filter-presets/{preset_id}"),
        &update_body,
        &bob_token,
    );
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_rejects_mismatched_condition() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "alice").await;

    let body = json!({
        "name": "SeriesPreset",
        "scope": "search",
        "target": "series",
        "condition": sample_series_condition(),
    });
    let app = create_test_router(state.clone()).await;
    let r = post_request_with_auth_json("/api/v1/filter-presets", &token, &body.to_string());
    let (_, response): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    let preset_id = response.unwrap()["id"].as_str().unwrap().to_string();

    // Try to update with a Book-only condition shape on a target=series preset
    let update_body = json!({
        "name": "SeriesPreset",
        "condition": { "bookType": { "operator": "is", "value": "manga" } },
    });
    let app = create_test_router(state).await;
    let r = put_json_request_with_auth(
        &format!("/api/v1/filter-presets/{preset_id}"),
        &update_body,
        &token,
    );
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ============================================================================
// Delete
// ============================================================================

#[tokio::test]
async fn test_delete_preset_success() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, token) = create_user_and_token(&db, &state, "alice").await;

    let body = json!({
        "name": "ToDelete",
        "scope": "search",
        "target": "books",
        "condition": sample_books_condition(),
    });
    let app = create_test_router(state.clone()).await;
    let r = post_request_with_auth_json("/api/v1/filter-presets", &token, &body.to_string());
    let (_, response): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    let preset_id = response.unwrap()["id"].as_str().unwrap().to_string();

    let app = create_test_router(state.clone()).await;
    let r = delete_request_with_auth(&format!("/api/v1/filter-presets/{preset_id}"), &token);
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Second delete -> 404
    let app = create_test_router(state).await;
    let r = delete_request_with_auth(&format!("/api/v1/filter-presets/{preset_id}"), &token);
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_other_users_preset_returns_404() {
    let (db, _tmp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let (_, alice_token) = create_user_and_token(&db, &state, "alice").await;
    let (_, bob_token) = create_user_and_token(&db, &state, "bob").await;

    let body = json!({
        "name": "Untouchable",
        "scope": "search",
        "target": "books",
        "condition": sample_books_condition(),
    });
    let app = create_test_router(state.clone()).await;
    let r = post_request_with_auth_json("/api/v1/filter-presets", &alice_token, &body.to_string());
    let (_, response): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    let preset_id = response.unwrap()["id"].as_str().unwrap().to_string();

    let app = create_test_router(state.clone()).await;
    let r = delete_request_with_auth(&format!("/api/v1/filter-presets/{preset_id}"), &bob_token);
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Alice's preset still exists
    let app = create_test_router(state).await;
    let r = get_request_with_auth(&format!("/api/v1/filter-presets/{preset_id}"), &alice_token);
    let (status, _): (StatusCode, Option<Value>) = make_json_request(app, r).await;
    assert_eq!(status, StatusCode::OK);
}
