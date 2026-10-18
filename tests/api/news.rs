#[path = "../common/mod.rs"]
mod common;

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use codex::api::extractors::AppState;
use codex::api::routes::v1::dto::NewsResponse;
use codex::db::repositories::{
    NewNewsEntry, NewsEntryRepository, NewsFeedRepository, UserRepository,
};
use codex::utils::password;
use common::*;
use hyper::StatusCode;
use sea_orm::DatabaseConnection;

async fn token_for(
    db: &DatabaseConnection,
    state: &Arc<AppState>,
    username: &str,
    is_admin: bool,
) -> String {
    let hashed = password::hash_password("secret123").unwrap();
    let user = create_test_user(username, &format!("{username}@test.com"), &hashed, is_admin);
    let created = UserRepository::create(db, &user).await.unwrap();
    state
        .jwt_service
        .generate_token(created.id, created.username.clone(), created.get_role())
        .unwrap()
}

fn entry(
    project: &str,
    kind: &str,
    version: Option<&str>,
    published_at: DateTime<Utc>,
) -> NewNewsEntry {
    let id = format!("{project}-{}", version.unwrap_or("post"));
    NewNewsEntry {
        id: id.clone(),
        project: project.to_string(),
        kind: kind.to_string(),
        title: id,
        version: version.map(str::to_string),
        summary: "summary".to_string(),
        body: "body".to_string(),
        url: version.map(|v| format!("https://example.com/{v}")),
        published_at,
    }
}

async fn get_news(state: &Arc<AppState>, token: &str) -> NewsResponse {
    let app = create_test_router_with_app_state(state.clone());
    let (status, body): (StatusCode, Option<NewsResponse>) =
        make_json_request(app, get_request_with_auth("/api/v1/news", token)).await;
    assert_eq!(status, StatusCode::OK);
    body.expect("news response")
}

fn unread_ids(news: &NewsResponse) -> Vec<String> {
    let mut ids: Vec<String> = news
        .entries
        .iter()
        .filter(|e| e.unread)
        .map(|e| e.id.clone())
        .collect();
    ids.sort();
    ids
}

#[tokio::test]
async fn news_requires_admin() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let token = token_for(&db, &state, "reader", false).await;

    let app = create_test_router_with_app_state(state.clone());
    let (status, _) = make_request(app, get_request_with_auth("/api/v1/news", &token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let app = create_test_router_with_app_state(state.clone());
    let (status, _) = make_request(app, post_request_with_auth("/api/v1/news/seen", &token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn disabled_news_reports_disabled_and_nothing_else() {
    let (db, _t) = setup_test_db().await;
    let mut disabled = (*create_test_app_state(db.clone()).await).clone();
    disabled.news_enabled = false;
    let state = Arc::new(disabled);
    let token = token_for(&db, &state, "admin", true).await;
    NewsEntryRepository::upsert_many(
        &db,
        &[entry("codex", "release", Some("999.0.0"), Utc::now())],
        Utc::now(),
    )
    .await
    .unwrap();

    let news = get_news(&state, &token).await;

    assert!(!news.enabled);
    assert!(news.entries.is_empty());
    assert_eq!(news.unread_count, 0);
    assert!(news.update_available.is_none());
}

#[tokio::test]
async fn nothing_fetched_yet_is_an_empty_enabled_drawer() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let token = token_for(&db, &state, "admin", true).await;

    let news = get_news(&state, &token).await;

    assert!(news.enabled);
    assert!(news.entries.is_empty());
    assert_eq!(news.unread_count, 0);
    assert!(news.update_available.is_none());
    assert!(news.last_fetched_at.is_none());
}

#[tokio::test]
async fn a_fresh_install_sees_recent_news_but_not_old_news_or_its_own_release() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let token = token_for(&db, &state, "admin", true).await;
    let now = Utc::now();
    let running = state.app_version;

    NewsEntryRepository::upsert_many(
        &db,
        &[
            // Inside the 30-day grace before the first fetch: unread.
            entry(
                "codex-ios",
                "release",
                Some("1.1.0"),
                now - Duration::days(10),
            ),
            // Older than the grace: read.
            entry(
                "codex-ios",
                "release",
                Some("1.0"),
                now - Duration::days(60),
            ),
            // The release this server runs: never news.
            entry("codex", "release", Some(running), now - Duration::days(1)),
            // An older release: never news.
            entry("codex", "release", Some("0.0.1"), now - Duration::days(2)),
            // A newer release: news, and an update.
            entry("codex", "release", Some("999.0.0"), now - Duration::days(3)),
        ],
        now,
    )
    .await
    .unwrap();

    let news = get_news(&state, &token).await;

    assert_eq!(news.entries.len(), 5);
    assert_eq!(
        unread_ids(&news),
        vec!["codex-999.0.0".to_string(), "codex-ios-1.1.0".to_string()]
    );
    assert_eq!(news.unread_count, 2);
    let update = news.update_available.expect("999.0.0 is newer");
    assert_eq!(update.current_version, running);
    assert_eq!(update.latest_version, "999.0.0");
    assert_eq!(update.url.as_deref(), Some("https://example.com/999.0.0"));
    // Newest first.
    assert_eq!(news.entries[0].id, format!("codex-{running}"));
}

#[tokio::test]
async fn marking_seen_clears_unread_until_something_newer_arrives() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let token = token_for(&db, &state, "admin", true).await;
    let other_admin = token_for(&db, &state, "admin2", true).await;
    let now = Utc::now();

    NewsEntryRepository::upsert_many(
        &db,
        &[entry(
            "codex-ios",
            "release",
            Some("1.1.0"),
            now - Duration::days(1),
        )],
        now,
    )
    .await
    .unwrap();
    assert_eq!(get_news(&state, &token).await.unread_count, 1);

    let app = create_test_router_with_app_state(state.clone());
    let (status, _) = make_request(app, post_request_with_auth("/api/v1/news/seen", &token)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(get_news(&state, &token).await.unread_count, 0);
    assert_eq!(
        get_news(&state, &other_admin).await.unread_count,
        1,
        "read state is per admin"
    );

    NewsEntryRepository::upsert_many(
        &db,
        &[entry(
            "codex-ios",
            "release",
            Some("1.2.0"),
            Utc::now() + Duration::minutes(1),
        )],
        Utc::now(),
    )
    .await
    .unwrap();
    let news = get_news(&state, &token).await;
    assert_eq!(unread_ids(&news), vec!["codex-ios-1.2.0".to_string()]);
}

#[tokio::test]
async fn no_update_when_the_newest_release_is_not_newer() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let token = token_for(&db, &state, "admin", true).await;
    let now = Utc::now();

    NewsEntryRepository::upsert_many(
        &db,
        &[
            entry("codex", "release", Some(state.app_version), now),
            entry("codex", "release", Some("nightly"), now),
            // iOS versions are never compared with the server's.
            entry("codex-ios", "release", Some("999.0.0"), now),
        ],
        now,
    )
    .await
    .unwrap();

    assert!(get_news(&state, &token).await.update_available.is_none());
}

#[tokio::test]
async fn last_fetched_at_reports_the_latest_feed_success() {
    let (db, _t) = setup_test_db().await;
    let state = create_test_app_state(db.clone()).await;
    let token = token_for(&db, &state, "admin", true).await;
    let older = Utc::now() - Duration::hours(5);
    let newer = Utc::now() - Duration::hours(1);
    NewsFeedRepository::record_success(&db, "codex", None, older)
        .await
        .unwrap();
    NewsFeedRepository::record_success(&db, "codex-ios", None, newer)
        .await
        .unwrap();

    let fetched = get_news(&state, &token).await.last_fetched_at.unwrap();
    assert!((fetched - newer).num_seconds().abs() < 2);
}
