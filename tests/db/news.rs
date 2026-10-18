//! News repository tests, run against both backends.
//!
//! The upsert is the part worth running on PostgreSQL: it is a single
//! `INSERT ... ON CONFLICT` whose update list must leave `first_seen_at`
//! alone, and the PostgreSQL test database is shared between runs, so each
//! test uses its own project name to stay isolated.

#[path = "../common/mod.rs"]
mod common;

use chrono::{Duration, TimeZone, Utc};
use codex_db::repositories::{NewNewsEntry, NewsEntryRepository, NewsFeedRepository};
use common::*;
use sea_orm::DatabaseConnection;

fn unique_project(label: &str) -> String {
    format!("{label}-{}", uuid::Uuid::new_v4())
}

fn entry(project: &str, id: &str, title: &str, days_ago: i64) -> NewNewsEntry {
    NewNewsEntry {
        id: format!("{project}-{id}"),
        project: project.to_string(),
        kind: "release".to_string(),
        title: title.to_string(),
        version: Some(id.trim_start_matches('v').to_string()),
        summary: format!("Summary of {title}"),
        body: format!("Body of {title}"),
        url: None,
        published_at: Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap()
            - Duration::days(days_ago),
    }
}

async fn exercise_upsert_keeps_first_seen(db: &DatabaseConnection) {
    let project = unique_project("upsert");
    let first = Utc::now() - Duration::days(3);
    let later = Utc::now();

    NewsEntryRepository::upsert_many(db, &[entry(&project, "v1.0.0", "One", 2)], first)
        .await
        .unwrap();

    let mut edited = entry(&project, "v1.0.0", "One, edited", 2);
    edited.summary = "New summary".to_string();
    NewsEntryRepository::upsert_many(db, &[edited, entry(&project, "v1.1.0", "Two", 1)], later)
        .await
        .unwrap();

    let rows = NewsEntryRepository::list_recent(db, &[project.as_str()], 10)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2, "the re-imported entry must not duplicate");

    let one = rows.iter().find(|r| r.id.ends_with("v1.0.0")).unwrap();
    assert_eq!(one.title, "One, edited");
    assert_eq!(one.summary, "New summary");
    assert!(
        (one.first_seen_at - first).num_seconds().abs() < 2,
        "an update must not move first_seen_at"
    );
    assert!((one.updated_at - later).num_seconds().abs() < 2);
}

async fn exercise_list_recent_order_and_filter(db: &DatabaseConnection) {
    let project = unique_project("order");
    let other = unique_project("other");
    let now = Utc::now();

    NewsEntryRepository::upsert_many(
        db,
        &[
            entry(&project, "v1.0.0", "Oldest", 10),
            entry(&project, "v1.2.0", "Newest", 0),
            entry(&project, "v1.1.0", "Middle", 5),
            entry(&other, "v9.0.0", "Elsewhere", 0),
        ],
        now,
    )
    .await
    .unwrap();

    let titles: Vec<String> = NewsEntryRepository::list_recent(db, &[project.as_str()], 2)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.title)
        .collect();
    assert_eq!(
        titles,
        vec!["Newest", "Middle"],
        "newest first, limited, filtered"
    );

    let both = NewsEntryRepository::list_recent(db, &[project.as_str(), other.as_str()], 10)
        .await
        .unwrap();
    assert_eq!(both.len(), 4);
}

async fn exercise_list_releases_skips_posts(db: &DatabaseConnection) {
    let project = unique_project("releases");
    let mut post = entry(&project, "post-hello", "Hello", 0);
    post.kind = "post".to_string();
    post.version = None;

    NewsEntryRepository::upsert_many(db, &[entry(&project, "v1.0.0", "One", 1), post], Utc::now())
        .await
        .unwrap();

    let releases = NewsEntryRepository::list_releases(db, &project)
        .await
        .unwrap();
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].version.as_deref(), Some("1.0.0"));
}

async fn exercise_feed_bookkeeping(db: &DatabaseConnection) {
    let project = unique_project("feed");
    assert!(
        NewsFeedRepository::get(db, &project)
            .await
            .unwrap()
            .is_none()
    );

    let t1 = Utc::now() - Duration::hours(30);
    NewsFeedRepository::record_success(db, &project, Some("\"a\"".to_string()), t1)
        .await
        .unwrap();
    let t2 = Utc::now();
    NewsFeedRepository::record_success(db, &project, Some("\"b\"".to_string()), t2)
        .await
        .unwrap();

    let feed = NewsFeedRepository::get(db, &project)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(feed.etag.as_deref(), Some("\"b\""));
    assert!((feed.last_success_at.unwrap() - t2).num_seconds().abs() < 2);
}

#[tokio::test]
async fn test_news_upsert_keeps_first_seen_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_upsert_keeps_first_seen(&db).await;
}

#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn test_news_upsert_keeps_first_seen_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };
    exercise_upsert_keeps_first_seen(&db).await;
}

#[tokio::test]
async fn test_news_list_recent_order_and_filter_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_list_recent_order_and_filter(&db).await;
}

#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn test_news_list_recent_order_and_filter_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };
    exercise_list_recent_order_and_filter(&db).await;
}

#[tokio::test]
async fn test_news_list_releases_skips_posts_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_list_releases_skips_posts(&db).await;
}

#[tokio::test]
async fn test_news_feed_bookkeeping_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_feed_bookkeeping(&db).await;
}

#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn test_news_feed_bookkeeping_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };
    exercise_feed_bookkeeping(&db).await;
}

/// Only meaningful on a fresh database, so SQLite only.
#[tokio::test]
async fn test_news_earliest_first_seen_sqlite() {
    let (db, _t) = setup_test_db().await;
    assert!(
        NewsEntryRepository::earliest_first_seen(&db)
            .await
            .unwrap()
            .is_none()
    );

    let early = Utc::now() - Duration::days(7);
    NewsEntryRepository::upsert_many(&db, &[entry("codex", "v1.0.0", "One", 9)], early)
        .await
        .unwrap();
    NewsEntryRepository::upsert_many(&db, &[entry("codex", "v1.1.0", "Two", 1)], Utc::now())
        .await
        .unwrap();

    let got = NewsEntryRepository::earliest_first_seen(&db)
        .await
        .unwrap()
        .unwrap();
    assert!((got - early).num_seconds().abs() < 2);
}
