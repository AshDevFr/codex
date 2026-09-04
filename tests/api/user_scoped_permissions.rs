//! Permission-gating matrix for the user-scoped content endpoints.
//!
//! Want-to-read, recommendations, and series exports are user-scoped, but
//! they all expose series metadata, so they require `series:read`. Filter
//! presets, user preferences, and the export field catalog stay auth-only:
//! they are UI state and static data with no library content in them.
//! These tests pin those gates with API keys carrying exactly one
//! permission each.

#[path = "../common/mod.rs"]
mod common;

use codex::api::permissions::{Permission, serialize_permissions};
use codex::db::ScanningStrategy;
use codex::db::repositories::{
    ApiKeyRepository, BookRepository, LibraryRepository, SeriesRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::{Request, StatusCode};
use serde_json::json;
use std::collections::HashSet;
use uuid::Uuid;

/// One reader-role user, one API key per permission under test.
struct Keys {
    /// A valid key whose permission has nothing to do with series content.
    progress_read: String,
    series_read: String,
}

async fn user_with_keys(db: &sea_orm::DatabaseConnection) -> Keys {
    let password_hash = password::hash_password("reader123").unwrap();
    let user = create_test_user("scopeduser", "scoped@example.com", &password_hash, false);
    let user = UserRepository::create(db, &user).await.unwrap();

    let mut keys = Vec::new();
    for (label, perm) in [
        ("progread", Permission::ProgressRead),
        ("seriesread", Permission::SeriesRead),
    ] {
        let plain = format!("codex_{label}_secret");
        let key_hash = password::hash_password(&plain).unwrap();
        let mut set = HashSet::new();
        set.insert(perm);
        let api_key = create_test_api_key(
            user.id,
            label,
            &key_hash,
            &format!("codex_{label}"),
            serde_json::from_str(&serialize_permissions(&set)).unwrap(),
        );
        ApiKeyRepository::create(db, &api_key).await.unwrap();
        keys.push(plain);
    }
    let mut keys = keys.into_iter();
    Keys {
        progress_read: keys.next().unwrap(),
        series_read: keys.next().unwrap(),
    }
}

async fn series_and_book(
    db: &sea_orm::DatabaseConnection,
) -> (Uuid, codex::db::entities::books::Model) {
    let library = LibraryRepository::create(db, "Lib", "/lib", ScanningStrategy::Default)
        .await
        .unwrap();
    let series = SeriesRepository::create(db, library.id, "Series", None)
        .await
        .unwrap();
    let book = create_test_book(
        series.id,
        library.id,
        "/lib/book.cbz",
        "book.cbz",
        "hash123",
        "cbz",
        100,
    );
    let book = BookRepository::create(db, &book, None).await.unwrap();
    (series.id, book)
}

fn api_request(
    method: &str,
    uri: &str,
    key: &str,
    body: Option<serde_json::Value>,
) -> Request<String> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("X-API-Key", key);
    match body {
        Some(value) => builder
            .header("content-type", "application/json")
            .body(value.to_string())
            .unwrap(),
        None => builder.body(String::new()).unwrap(),
    }
}

async fn status_of(
    app: axum::Router,
    method: &str,
    uri: &str,
    key: &str,
    body: Option<serde_json::Value>,
) -> StatusCode {
    let (status, _) = make_raw_request(app, api_request(method, uri, key, body)).await;
    status
}

fn export_body() -> serde_json::Value {
    json!({
        "format": "json",
        "libraryIds": ["550e8400-e29b-41d4-a716-446655440000"],
        "fields": ["title", "genres"]
    })
}

#[tokio::test]
async fn key_without_series_read_is_locked_out_of_series_surfaces() {
    let (db, _temp_dir) = setup_test_db().await;
    let (series_id, book) = series_and_book(&db).await;
    let keys = user_with_keys(&db).await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let key = &keys.progress_read;
    let export_id = Uuid::new_v4();
    let cases: Vec<(&str, String, Option<serde_json::Value>)> = vec![
        ("GET", "/api/v1/want-to-read".to_string(), None),
        (
            "POST",
            "/api/v1/want-to-read".to_string(),
            Some(json!({"seriesId": series_id})),
        ),
        (
            "POST",
            "/api/v1/want-to-read/bulk".to_string(),
            Some(json!({"seriesIds": [series_id]})),
        ),
        (
            "PUT",
            "/api/v1/want-to-read/order".to_string(),
            Some(json!({"entryIds": []})),
        ),
        (
            "DELETE",
            format!("/api/v1/want-to-read/series/{series_id}"),
            None,
        ),
        (
            "DELETE",
            format!("/api/v1/want-to-read/books/{}", book.id),
            None,
        ),
        ("GET", "/api/v1/user/recommendations".to_string(), None),
        (
            "POST",
            "/api/v1/user/recommendations/refresh".to_string(),
            None,
        ),
        (
            "POST",
            "/api/v1/user/recommendations/12345/dismiss".to_string(),
            Some(json!({})),
        ),
        (
            "POST",
            "/api/v1/user/exports/series".to_string(),
            Some(export_body()),
        ),
        ("GET", "/api/v1/user/exports/series".to_string(), None),
        (
            "GET",
            format!("/api/v1/user/exports/series/{export_id}"),
            None,
        ),
        (
            "DELETE",
            format!("/api/v1/user/exports/series/{export_id}"),
            None,
        ),
        (
            "GET",
            format!("/api/v1/user/exports/series/{export_id}/download"),
            None,
        ),
    ];

    for (method, uri, body) in cases {
        let status = status_of(app.clone(), method, &uri, key, body).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{method} {uri} must be forbidden without series:read"
        );
    }
}

#[tokio::test]
async fn series_read_key_uses_the_series_surfaces() {
    let (db, _temp_dir) = setup_test_db().await;
    let (series_id, _book) = series_and_book(&db).await;
    let keys = user_with_keys(&db).await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let key = &keys.series_read;

    let status = status_of(
        app.clone(),
        "POST",
        "/api/v1/want-to-read",
        key,
        Some(json!({"seriesId": series_id})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "series:read adds want-to-read");

    let status = status_of(app.clone(), "GET", "/api/v1/want-to-read", key, None).await;
    assert_eq!(status, StatusCode::OK, "series:read lists want-to-read");

    // 404: the gate passed and the handler reported no recommendation
    // plugin is enabled in the test environment.
    let status = status_of(
        app.clone(),
        "GET",
        "/api/v1/user/recommendations",
        key,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let status = status_of(
        app.clone(),
        "POST",
        "/api/v1/user/exports/series",
        key,
        Some(export_body()),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "series:read creates exports");

    let status = status_of(app.clone(), "GET", "/api/v1/user/exports/series", key, None).await;
    assert_eq!(status, StatusCode::OK, "series:read lists exports");
}

#[tokio::test]
async fn field_catalog_stays_auth_only() {
    let (db, _temp_dir) = setup_test_db().await;
    let keys = user_with_keys(&db).await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Static data, no library content: any authenticated key may read it.
    let status = status_of(
        app.clone(),
        "GET",
        "/api/v1/user/exports/series/fields",
        &keys.progress_read,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
