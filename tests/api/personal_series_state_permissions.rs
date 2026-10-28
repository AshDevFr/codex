//! Permission-gating matrix for a user's own ratings and want-to-read queue.
//!
//! Both are user-scoped data that used to be writable with `series:read`
//! alone, so a key the user scoped to read-only could overwrite or delete
//! their ratings and reshuffle their queue. Writes now need `ratings:write`
//! and `want-to-read:write`; reads stay under `series:read`. These tests pin
//! the gate on every endpoint with API keys carrying exactly the permissions
//! under test.

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

/// A reader-role user plus one API key per permission set under test. The
/// keys all belong to the same user so data written through one key is
/// visible through another.
struct Keys {
    series_read: String,
    ratings_write: String,
    want_to_read_write: String,
}

async fn user_with_keys(db: &sea_orm::DatabaseConnection) -> Keys {
    let password_hash = password::hash_password("reader123").unwrap();
    let user = create_test_user("stateuser", "state@example.com", &password_hash, false);
    let user = UserRepository::create(db, &user).await.unwrap();

    let mut keys = Vec::new();
    for (label, perms) in [
        ("seriesread", vec![Permission::SeriesRead]),
        ("ratewrite", vec![Permission::RatingsWrite]),
        ("wtrwrite", vec![Permission::WantToReadWrite]),
    ] {
        let plain = format!("codex_{label}_secret");
        let key_hash = password::hash_password(&plain).unwrap();
        let set: HashSet<Permission> = perms.into_iter().collect();
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
        series_read: keys.next().unwrap(),
        ratings_write: keys.next().unwrap(),
        want_to_read_write: keys.next().unwrap(),
    }
}

async fn library_series_book(db: &sea_orm::DatabaseConnection) -> (Uuid, Uuid) {
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
    (series.id, book.id)
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

fn rating_writes(series_id: Uuid) -> Vec<(&'static str, String, Option<serde_json::Value>)> {
    vec![
        (
            "PUT",
            format!("/api/v1/series/{series_id}/rating"),
            Some(json!({"rating": 80})),
        ),
        ("DELETE", format!("/api/v1/series/{series_id}/rating"), None),
    ]
}

fn want_to_read_writes(
    series_id: Uuid,
    book_id: Uuid,
) -> Vec<(&'static str, String, Option<serde_json::Value>)> {
    vec![
        (
            "POST",
            "/api/v1/want-to-read".to_string(),
            Some(json!({"seriesId": series_id})),
        ),
        (
            "POST",
            "/api/v1/want-to-read/bulk".to_string(),
            Some(json!({"bookIds": [book_id]})),
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
            format!("/api/v1/want-to-read/books/{book_id}"),
            None,
        ),
    ]
}

#[tokio::test]
async fn series_read_key_reads_but_cannot_write_personal_state() {
    let (db, _temp_dir) = setup_test_db().await;
    let (series_id, book_id) = library_series_book(&db).await;
    let keys = user_with_keys(&db).await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;
    let key = &keys.series_read;

    for (method, uri, body) in rating_writes(series_id)
        .into_iter()
        .chain(want_to_read_writes(series_id, book_id))
    {
        assert_eq!(
            status_of(app.clone(), method, &uri, key, body).await,
            StatusCode::FORBIDDEN,
            "{method} {uri} must need a write permission, not series:read"
        );
    }

    // Reads stay under series:read. No rating exists yet, hence 204.
    assert_eq!(
        status_of(
            app.clone(),
            "GET",
            &format!("/api/v1/series/{series_id}/rating"),
            key,
            None
        )
        .await,
        StatusCode::NO_CONTENT
    );
    for uri in ["/api/v1/user/ratings", "/api/v1/want-to-read"] {
        assert_eq!(
            status_of(app.clone(), "GET", uri, key, None).await,
            StatusCode::OK,
            "GET {uri} must stay readable with series:read"
        );
    }
}

#[tokio::test]
async fn ratings_write_key_manages_ratings_only() {
    let (db, _temp_dir) = setup_test_db().await;
    let (series_id, book_id) = library_series_book(&db).await;
    let keys = user_with_keys(&db).await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;
    let key = &keys.ratings_write;

    let uri = format!("/api/v1/series/{series_id}/rating");
    assert_eq!(
        status_of(app.clone(), "PUT", &uri, key, Some(json!({"rating": 80}))).await,
        StatusCode::OK
    );
    assert_eq!(
        status_of(app.clone(), "DELETE", &uri, key, None).await,
        StatusCode::NO_CONTENT
    );

    for (method, uri, body) in want_to_read_writes(series_id, book_id) {
        assert_eq!(
            status_of(app.clone(), method, &uri, key, body).await,
            StatusCode::FORBIDDEN,
            "ratings:write must not grant {method} {uri}"
        );
    }
}

#[tokio::test]
async fn want_to_read_write_key_manages_queue_only() {
    let (db, _temp_dir) = setup_test_db().await;
    let (series_id, book_id) = library_series_book(&db).await;
    let keys = user_with_keys(&db).await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;
    let key = &keys.want_to_read_write;

    let expected = [
        StatusCode::CREATED,
        StatusCode::OK,
        StatusCode::NO_CONTENT,
        StatusCode::NO_CONTENT,
        StatusCode::NO_CONTENT,
    ];
    for ((method, uri, body), want) in want_to_read_writes(series_id, book_id)
        .into_iter()
        .zip(expected)
    {
        assert_eq!(
            status_of(app.clone(), method, &uri, key, body).await,
            want,
            "{method} {uri}"
        );
    }

    for (method, uri, body) in rating_writes(series_id) {
        assert_eq!(
            status_of(app.clone(), method, &uri, key, body).await,
            StatusCode::FORBIDDEN,
            "want-to-read:write must not grant {method} {uri}"
        );
    }
}
