// Library jobs API integration tests (Phase 9).

#![allow(unused_variables)]

#[path = "../common/mod.rs"]
mod common;

use codex::db::ScanningStrategy;
use codex::db::repositories::{LibraryRepository, UserRepository};
use codex::utils::password;
use common::*;
use hyper::StatusCode;

async fn create_admin_token(
    db: &sea_orm::DatabaseConnection,
    state: &codex::api::extractors::AuthState,
) -> String {
    let password_hash = password::hash_password("admin123").unwrap();
    let user = create_test_user("admin", "admin@example.com", &password_hash, true);
    let created = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

#[tokio::test]
async fn list_jobs_empty_library() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_token(&db, &state).await;
    let lib = LibraryRepository::create(&db, "L", "/tmp/L", ScanningStrategy::Default)
        .await
        .unwrap();

    let app = create_test_router(state).await;
    let req = get_request_with_auth(&format!("/api/v1/libraries/{}/jobs", lib.id), &token);
    let (status, body) = make_json_request::<serde_json::Value>(app, req).await;
    let body = body.unwrap_or_default();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["jobs"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn list_jobs_unknown_library_404() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_token(&db, &state).await;

    let app = create_test_router(state).await;
    let req = get_request_with_auth(
        "/api/v1/libraries/00000000-0000-0000-0000-000000000000/jobs",
        &token,
    );
    let (status, _body) = make_json_request::<serde_json::Value>(app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_jobs_unauthenticated_401() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let lib = LibraryRepository::create(&db, "L", "/tmp/L", ScanningStrategy::Default)
        .await
        .unwrap();

    let app = create_test_router(state).await;
    let req = get_request(&format!("/api/v1/libraries/{}/jobs", lib.id));
    let (status, _body) = make_request(app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_job_with_unknown_provider_returns_400() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_token(&db, &state).await;
    let lib = LibraryRepository::create(&db, "L", "/tmp/L", ScanningStrategy::Default)
        .await
        .unwrap();

    let body = serde_json::json!({
        "name": "Test Job",
        "enabled": false,
        "cronSchedule": "0 0 4 * * *",
        "config": {
            "type": "metadata_refresh",
            "provider": "plugin:nope",
            "scope": "series_only",
            "fieldGroups": ["ratings"],
            "extraFields": [],
            "bookFieldGroups": [],
            "bookExtraFields": [],
            "existingSourceIdsOnly": true,
            "skipRecentlySyncedWithinS": 3600,
            "maxConcurrency": 4,
        }
    });

    let app = create_test_router(state).await;
    let req = post_request_with_auth_json(
        &format!("/api/v1/libraries/{}/jobs", lib.id),
        &token,
        &body.to_string(),
    );
    let (status, body) = make_json_request::<serde_json::Value>(app, req).await;
    let body = body.unwrap_or_default();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let msg = body["message"].as_str().unwrap_or_default();
    assert!(msg.contains("not installed"), "got: {msg}");
}

#[tokio::test]
async fn create_job_rejects_books_only_scope() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_token(&db, &state).await;
    let lib = LibraryRepository::create(&db, "L", "/tmp/L", ScanningStrategy::Default)
        .await
        .unwrap();

    let body = serde_json::json!({
        "name": "Books Job",
        "enabled": false,
        "cronSchedule": "0 0 4 * * *",
        "config": {
            "type": "metadata_refresh",
            "provider": "plugin:nope",
            "scope": "books_only",
            "fieldGroups": [],
            "extraFields": [],
            "bookFieldGroups": ["ratings"],
            "bookExtraFields": [],
            "existingSourceIdsOnly": true,
            "skipRecentlySyncedWithinS": 3600,
            "maxConcurrency": 4,
        }
    });

    let app = create_test_router(state).await;
    let req = post_request_with_auth_json(
        &format!("/api/v1/libraries/{}/jobs", lib.id),
        &token,
        &body.to_string(),
    );
    let (status, body) = make_json_request::<serde_json::Value>(app, req).await;
    let body = body.unwrap_or_default();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let msg = body["message"].as_str().unwrap_or_default();
    assert!(msg.contains("not yet implemented"), "got: {msg}");
}

#[tokio::test]
async fn create_job_rejects_invalid_cron() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_token(&db, &state).await;
    let lib = LibraryRepository::create(&db, "L", "/tmp/L", ScanningStrategy::Default)
        .await
        .unwrap();

    let body = serde_json::json!({
        "name": "Bad cron",
        "enabled": false,
        "cronSchedule": "not a cron",
        "config": {
            "type": "metadata_refresh",
            "provider": "plugin:any",
            "scope": "series_only",
        }
    });

    let app = create_test_router(state).await;
    let req = post_request_with_auth_json(
        &format!("/api/v1/libraries/{}/jobs", lib.id),
        &token,
        &body.to_string(),
    );
    let (status, _body) = make_json_request::<serde_json::Value>(app, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn field_groups_catalog_returns_known_groups() {
    let (db, _temp) = setup_test_db().await;
    let state = create_test_auth_state(db.clone()).await;
    let token = create_admin_token(&db, &state).await;

    let app = create_test_router(state).await;
    let req = get_request_with_auth("/api/v1/library-jobs/metadata-refresh/field-groups", &token);
    let (status, body) = make_json_request::<serde_json::Value>(app, req).await;
    let body = body.unwrap_or_default();
    assert_eq!(status, StatusCode::OK);
    let groups = body.as_array().unwrap();
    assert!(
        groups.len() >= 12,
        "expected 12 groups, got {}",
        groups.len()
    );
    // Spot check the contract: ratings group includes both rating + externalRatings
    let ratings = groups.iter().find(|g| g["id"] == "ratings").unwrap();
    let fields: Vec<&str> = ratings["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(fields.contains(&"rating"));
    assert!(fields.contains(&"externalRatings"));
}
