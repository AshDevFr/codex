//! Permission-gating matrix for reading-progress endpoints.
//!
//! Reading progress is user-scoped data with its own permission pair:
//! `progress:read` to view it and `progress:write` to mutate it. A content
//! key (`books:read`) must not be able to move, erase, or fabricate reading
//! history, and a stats-only key (`progress:read`) must see the progress
//! surface without any mutation rights. These tests pin the gate on every
//! progress endpoint across the v1 and Komga APIs with API keys carrying
//! exactly one permission each.

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

/// A reader-role user plus one API key per permission under test. The keys
/// all belong to the same user so progress written through one key is
/// visible through another.
struct Keys {
    books_read: String,
    progress_read: String,
    progress_write: String,
}

async fn user_with_keys(db: &sea_orm::DatabaseConnection) -> Keys {
    let password_hash = password::hash_password("reader123").unwrap();
    let user = create_test_user("permuser", "perm@example.com", &password_hash, false);
    let user = UserRepository::create(db, &user).await.unwrap();

    let mut keys = Vec::new();
    for (label, perm) in [
        ("booksread", Permission::BooksRead),
        ("progread", Permission::ProgressRead),
        ("progwrite", Permission::ProgressWrite),
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
        books_read: keys.next().unwrap(),
        progress_read: keys.next().unwrap(),
        progress_write: keys.next().unwrap(),
    }
}

async fn library_series_book(
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

fn session_body(book_id: Uuid) -> serde_json::Value {
    json!({
        "sessions": [{
            "id": Uuid::new_v4(),
            "bookId": book_id,
            "deviceId": "test-device",
            "kind": "progress",
            "toPage": 10,
            "clientStartedAt": "2026-06-01T09:00:00Z",
            "clientEndedAt": "2026-06-01T09:10:00Z",
        }]
    })
}

#[tokio::test]
async fn content_key_cannot_read_or_write_progress() {
    let (db, _temp_dir) = setup_test_db().await;
    let (series_id, book) = library_series_book(&db).await;
    let keys = user_with_keys(&db).await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let key = &keys.books_read;
    let cases: Vec<(&str, String, Option<serde_json::Value>)> = vec![
        (
            "PUT",
            format!("/api/v1/books/{}/progress", book.id),
            Some(json!({"currentPage": 5})),
        ),
        ("GET", format!("/api/v1/books/{}/progress", book.id), None),
        (
            "DELETE",
            format!("/api/v1/books/{}/progress", book.id),
            None,
        ),
        ("POST", format!("/api/v1/books/{}/read", book.id), None),
        ("POST", format!("/api/v1/books/{}/unread", book.id), None),
        (
            "GET",
            format!("/api/v1/books/{}/progression", book.id),
            None,
        ),
        (
            "GET",
            format!("/api/v1/books/{}/read-history", book.id),
            None,
        ),
        (
            "DELETE",
            format!("/api/v1/books/{}/read-history", book.id),
            None,
        ),
        ("POST", format!("/api/v1/series/{}/read", series_id), None),
        ("POST", format!("/api/v1/series/{}/unread", series_id), None),
        (
            "GET",
            format!("/api/v1/series/{}/read-history", series_id),
            None,
        ),
        (
            "DELETE",
            format!("/api/v1/series/{}/read-history", series_id),
            None,
        ),
        (
            "POST",
            "/api/v1/books/bulk/read".to_string(),
            Some(json!({"bookIds": [book.id]})),
        ),
        (
            "POST",
            "/api/v1/books/bulk/unread".to_string(),
            Some(json!({"bookIds": [book.id]})),
        ),
        (
            "POST",
            "/api/v1/series/bulk/read".to_string(),
            Some(json!({"seriesIds": [series_id]})),
        ),
        (
            "POST",
            "/api/v1/series/bulk/unread".to_string(),
            Some(json!({"seriesIds": [series_id]})),
        ),
        (
            "POST",
            "/api/v1/reading-sessions".to_string(),
            Some(session_body(book.id)),
        ),
        ("GET", "/api/v1/progress".to_string(), None),
        ("DELETE", "/api/v1/user/read-history".to_string(), None),
    ];

    for (method, uri, body) in cases {
        let status = status_of(app.clone(), method, &uri, key, body).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{method} {uri} must be forbidden for a books:read-only key"
        );
    }
}

#[tokio::test]
async fn progress_write_key_mutates_progress_but_cannot_read_it() {
    let (db, _temp_dir) = setup_test_db().await;
    let (series_id, book) = library_series_book(&db).await;
    let keys = user_with_keys(&db).await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    let key = &keys.progress_write;

    let status = status_of(
        app.clone(),
        "PUT",
        &format!("/api/v1/books/{}/progress", book.id),
        key,
        Some(json!({"currentPage": 5})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "progress:write must update progress"
    );

    let status = status_of(
        app.clone(),
        "POST",
        &format!("/api/v1/books/{}/read", book.id),
        key,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "progress:write must mark as read");

    let status = status_of(
        app.clone(),
        "POST",
        &format!("/api/v1/series/{}/read", series_id),
        key,
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "progress:write must mark series as read"
    );

    let status = status_of(
        app.clone(),
        "POST",
        "/api/v1/books/bulk/read",
        key,
        Some(json!({"bookIds": [book.id]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "progress:write must bulk mark read");

    let status = status_of(
        app.clone(),
        "POST",
        "/api/v1/reading-sessions",
        key,
        Some(session_body(book.id)),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "progress:write must record reading sessions"
    );

    // Write-only is not read: viewing progress needs progress:read.
    let status = status_of(
        app.clone(),
        "GET",
        &format!("/api/v1/books/{}/progress", book.id),
        key,
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "progress:write alone must not read progress"
    );
}

#[tokio::test]
async fn stats_key_reads_progress_and_nothing_more() {
    let (db, _temp_dir) = setup_test_db().await;
    let (series_id, book) = library_series_book(&db).await;
    let keys = user_with_keys(&db).await;
    let state = create_test_auth_state(db).await;
    let app = create_test_router(state).await;

    // Seed progress through the write key so the read key has data to see.
    let status = status_of(
        app.clone(),
        "PUT",
        &format!("/api/v1/books/{}/progress", book.id),
        &keys.progress_write,
        Some(json!({"currentPage": 5})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let key = &keys.progress_read;

    let status = status_of(
        app.clone(),
        "GET",
        &format!("/api/v1/books/{}/progress", book.id),
        key,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "progress:read must view progress");

    let status = status_of(app.clone(), "GET", "/api/v1/progress", key, None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "progress:read must list own progress"
    );

    let status = status_of(
        app.clone(),
        "GET",
        &format!("/api/v1/books/{}/read-history", book.id),
        key,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "progress:read must view history");

    let status = status_of(
        app.clone(),
        "GET",
        &format!("/api/v1/series/{}/read-history", series_id),
        key,
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "progress:read must view series history"
    );

    let status = status_of(
        app.clone(),
        "GET",
        "/api/v1/reading-stats?seriesLimit=1",
        key,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "progress:read must view stats");

    // Read-only is not write.
    for (method, uri, body) in [
        (
            "PUT",
            format!("/api/v1/books/{}/progress", book.id),
            Some(json!({"currentPage": 9})),
        ),
        (
            "DELETE",
            format!("/api/v1/books/{}/progress", book.id),
            None,
        ),
        ("POST", format!("/api/v1/books/{}/read", book.id), None),
        ("DELETE", "/api/v1/user/read-history".to_string(), None),
    ] {
        let status = status_of(app.clone(), method, &uri, key, body).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{method} {uri} must be forbidden for a progress:read-only key"
        );
    }
}

#[tokio::test]
async fn komga_progress_endpoints_follow_the_same_gates() {
    let (db, _temp_dir) = setup_test_db().await;
    let (series_id, book) = library_series_book(&db).await;
    let keys = user_with_keys(&db).await;
    let (_state, app) = setup_test_app_with_komga(db).await;

    // A content key must not mutate progress through the compat API either.
    for (method, uri, body) in [
        (
            "PATCH",
            format!("/komga/api/v1/books/{}/read-progress", book.id),
            Some(json!({"page": 42, "completed": false})),
        ),
        (
            "DELETE",
            format!("/komga/api/v1/books/{}/read-progress", book.id),
            None,
        ),
        (
            "GET",
            format!("/komga/api/v1/books/{}/progression", book.id),
            None,
        ),
        (
            "PUT",
            format!("/komga/api/v1/books/{}/progression", book.id),
            Some(json!({})),
        ),
        (
            "POST",
            format!("/komga/api/v1/series/{}/read-progress", series_id),
            None,
        ),
        (
            "DELETE",
            format!("/komga/api/v1/series/{}/read-progress", series_id),
            None,
        ),
    ] {
        let status = status_of(app.clone(), method, &uri, &keys.books_read, body).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{method} {uri} must be forbidden for a books:read-only key"
        );
    }

    // The write key drives the mutations.
    let status = status_of(
        app.clone(),
        "PATCH",
        &format!("/komga/api/v1/books/{}/read-progress", book.id),
        &keys.progress_write,
        Some(json!({"page": 42, "completed": false})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let status = status_of(
        app.clone(),
        "POST",
        &format!("/komga/api/v1/series/{}/read-progress", series_id),
        &keys.progress_write,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // The read key sees progression (204: progress exists, no R2 payload).
    let status = status_of(
        app.clone(),
        "GET",
        &format!("/komga/api/v1/books/{}/progression", book.id),
        &keys.progress_read,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn komga_accepts_the_api_key_as_a_bearer_token() {
    let (db, _temp_dir) = setup_test_db().await;
    let (_series_id, book) = library_series_book(&db).await;
    let keys = user_with_keys(&db).await;
    let (_state, app) = setup_test_app_with_komga(db).await;

    // Komga clients commonly send API keys as Bearer credentials; the
    // FlexibleAuthContext extractor routes the codex_ prefix to API key
    // verification just like the main extractor.
    let request = Request::builder()
        .method("PATCH")
        .uri(format!("/komga/api/v1/books/{}/read-progress", book.id))
        .header("authorization", format!("Bearer {}", keys.progress_write))
        .header("content-type", "application/json")
        .body(r#"{"page": 7, "completed": false}"#.to_string())
        .unwrap();
    let (status, _) = make_raw_request(app, request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}
