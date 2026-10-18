//! `NewsFetcher` against a local stub of the feed host.
//!
//! The stub serves whatever each test configures per project and records the
//! request headers, so the tests can check both what gets stored and what the
//! server sends (no version, conditional requests).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tokio::sync::Mutex;

use codex_db::repositories::{NewsEntryRepository, NewsFeedRepository};
use codex_db::test_helpers::create_test_db;
use codex_services::news::{FetchOutcome, NewsFetcher, needs_refresh};

#[derive(Clone)]
enum Reply {
    /// 200 with this body and ETag; 304 when the request carries the ETag.
    Feed {
        body: String,
        etag: &'static str,
    },
    Status(StatusCode),
    /// Never answers within the client timeout.
    Hang,
}

#[derive(Clone, Default)]
struct Stub {
    replies: Arc<Mutex<HashMap<String, Reply>>>,
    requests: Arc<Mutex<Vec<HeaderMap>>>,
}

async fn serve_feed(
    State(stub): State<Stub>,
    Path(file): Path<String>,
    headers: HeaderMap,
) -> Response {
    stub.requests.lock().await.push(headers.clone());
    let project = file.trim_end_matches(".json").to_string();
    let reply = stub.replies.lock().await.get(&project).cloned();
    match reply {
        Some(Reply::Feed { body, etag }) => {
            if headers
                .get(header::IF_NONE_MATCH)
                .is_some_and(|v| v.as_bytes() == etag.as_bytes())
            {
                return StatusCode::NOT_MODIFIED.into_response();
            }
            ([(header::ETAG, etag)], body).into_response()
        }
        Some(Reply::Status(status)) => status.into_response(),
        Some(Reply::Hang) => {
            tokio::time::sleep(Duration::from_secs(30)).await;
            StatusCode::OK.into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn start_stub() -> (Stub, String) {
    let stub = Stub::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/feeds/{file}", get(serve_feed))
        .with_state(stub.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (stub, format!("http://{addr}/feeds"))
}

fn feed_json(project: &str, ids: &[&str]) -> String {
    let entries: Vec<String> = ids
        .iter()
        .map(|id| {
            format!(
                r#"{{"id": "{project}-{id}", "project": "{project}", "kind": "release",
                "title": "{id}", "published_at": "2026-10-02T23:24:00Z",
                "version": "{}", "summary": "s", "body": "b"}}"#,
                id.trim_start_matches('v')
            )
        })
        .collect();
    format!(
        r#"{{"version": 1, "project": "{project}", "generated_at": "2026-10-17T00:00:00Z", "entries": [{}]}}"#,
        entries.join(",")
    )
}

#[tokio::test]
async fn a_200_stores_entries_and_the_etag() {
    let (db, _t) = create_test_db().await;
    let db = db.sea_orm_connection();
    let (stub, base) = start_stub().await;
    stub.replies.lock().await.insert(
        "codex".into(),
        Reply::Feed {
            body: feed_json("codex", &["v2.6.2", "v2.6.3"]),
            etag: "\"one\"",
        },
    );

    let fetcher = NewsFetcher::with_base_url(&base).unwrap();
    let outcome = fetcher.fetch_one(db, "codex").await.unwrap();

    assert_eq!(outcome, FetchOutcome::Updated(2));
    let rows = NewsEntryRepository::list_recent(db, &["codex"], 10)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    let feed = NewsFeedRepository::get(db, "codex").await.unwrap().unwrap();
    assert_eq!(feed.etag.as_deref(), Some("\"one\""));
    assert!(feed.last_success_at.is_some());
}

#[tokio::test]
async fn the_request_identifies_nothing_but_the_product() {
    let (db, _t) = create_test_db().await;
    let db = db.sea_orm_connection();
    let (stub, base) = start_stub().await;
    stub.replies.lock().await.insert(
        "codex".into(),
        Reply::Feed {
            body: feed_json("codex", &["v2.6.3"]),
            etag: "\"one\"",
        },
    );

    NewsFetcher::with_base_url(&base)
        .unwrap()
        .fetch_one(db, "codex")
        .await
        .unwrap();

    let requests = stub.requests.lock().await;
    let headers = &requests[0];
    assert_eq!(headers.get(header::USER_AGENT).unwrap(), "Codex");
    let version = env!("CARGO_PKG_VERSION");
    for (name, value) in headers {
        assert!(
            !value.to_str().unwrap_or_default().contains(version),
            "header {name} leaks the version"
        );
    }
}

#[tokio::test]
async fn a_second_fetch_is_conditional_and_a_304_still_counts_as_success() {
    let (db, _t) = create_test_db().await;
    let db = db.sea_orm_connection();
    let (stub, base) = start_stub().await;
    stub.replies.lock().await.insert(
        "codex".into(),
        Reply::Feed {
            body: feed_json("codex", &["v2.6.3"]),
            etag: "\"one\"",
        },
    );
    let fetcher = NewsFetcher::with_base_url(&base).unwrap();

    fetcher.fetch_one(db, "codex").await.unwrap();
    let first = NewsFeedRepository::get(db, "codex").await.unwrap().unwrap();
    let outcome = fetcher.fetch_one(db, "codex").await.unwrap();

    assert_eq!(outcome, FetchOutcome::NotModified);
    let requests = stub.requests.lock().await;
    assert_eq!(
        requests[1].get(header::IF_NONE_MATCH).unwrap(),
        "\"one\"",
        "the stored ETag must be sent back"
    );
    let second = NewsFeedRepository::get(db, "codex").await.unwrap().unwrap();
    assert_eq!(
        second.etag.as_deref(),
        Some("\"one\""),
        "a 304 keeps the ETag"
    );
    assert!(second.last_success_at >= first.last_success_at);
}

#[tokio::test]
async fn a_server_error_keeps_what_was_stored() {
    let (db, _t) = create_test_db().await;
    let db = db.sea_orm_connection();
    let (stub, base) = start_stub().await;
    let fetcher = NewsFetcher::with_base_url(&base).unwrap();
    stub.replies.lock().await.insert(
        "codex".into(),
        Reply::Feed {
            body: feed_json("codex", &["v2.6.3"]),
            etag: "\"one\"",
        },
    );
    fetcher.fetch_one(db, "codex").await.unwrap();
    let before = NewsFeedRepository::get(db, "codex").await.unwrap().unwrap();

    stub.replies.lock().await.insert(
        "codex".into(),
        Reply::Status(StatusCode::INTERNAL_SERVER_ERROR),
    );
    assert!(fetcher.fetch_one(db, "codex").await.is_err());

    let rows = NewsEntryRepository::list_recent(db, &["codex"], 10)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "a failed fetch must not drop stored entries");
    let after = NewsFeedRepository::get(db, "codex").await.unwrap().unwrap();
    assert_eq!(
        after.last_success_at, before.last_success_at,
        "a failure is not a success"
    );
}

#[tokio::test]
async fn an_oversized_feed_is_refused() {
    let (db, _t) = create_test_db().await;
    let db = db.sea_orm_connection();
    let (stub, base) = start_stub().await;
    stub.replies.lock().await.insert(
        "codex".into(),
        Reply::Feed {
            body: "x".repeat(3 * 1024 * 1024),
            etag: "\"big\"",
        },
    );

    let error = NewsFetcher::with_base_url(&base)
        .unwrap()
        .fetch_one(db, "codex")
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("larger than"), "{error:#}");
    assert!(
        NewsFeedRepository::get(db, "codex")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn a_hanging_host_times_out() {
    let (db, _t) = create_test_db().await;
    let db = db.sea_orm_connection();
    let (stub, base) = start_stub().await;
    stub.replies
        .lock()
        .await
        .insert("codex".into(), Reply::Hang);

    let started = std::time::Instant::now();
    let result = NewsFetcher::with_base_url(&base)
        .unwrap()
        .fetch_one(db, "codex")
        .await;
    assert!(result.is_err());
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "the client timeout must fire"
    );
}

#[tokio::test]
async fn fetch_all_keeps_going_past_a_failing_feed_and_staleness_follows() {
    let (db, _t) = create_test_db().await;
    let db = db.sea_orm_connection();
    let (stub, base) = start_stub().await;
    // codex-ios answers 404 (unconfigured); codex succeeds.
    stub.replies.lock().await.insert(
        "codex".into(),
        Reply::Feed {
            body: feed_json("codex", &["v2.6.3"]),
            etag: "\"one\"",
        },
    );
    let fetcher = NewsFetcher::with_base_url(&base).unwrap();

    assert!(needs_refresh(db, chrono::Utc::now()).await.unwrap());
    let results = fetcher.fetch_all(db).await;
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|(p, r)| p == "codex" && r.is_ok()));
    assert!(results.iter().any(|(p, r)| p == "codex-ios" && r.is_err()));
    assert!(
        needs_refresh(db, chrono::Utc::now()).await.unwrap(),
        "one feed never succeeded, so startup must still refresh"
    );

    stub.replies.lock().await.insert(
        "codex-ios".into(),
        Reply::Feed {
            body: feed_json("codex-ios", &["1.1.0"]),
            etag: "\"ios\"",
        },
    );
    fetcher.fetch_all(db).await;
    assert!(!needs_refresh(db, chrono::Utc::now()).await.unwrap());
    assert!(
        needs_refresh(db, chrono::Utc::now() + chrono::Duration::hours(25))
            .await
            .unwrap(),
        "a day-old fetch is stale"
    );
}
