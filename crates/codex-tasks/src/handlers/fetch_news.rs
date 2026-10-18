//! Handler for the `FetchNews` task.
//!
//! Downloads the Codex news feeds and stores their entries. The scheduler
//! only enqueues this when news is enabled, but the handler checks again: a
//! task can also arrive through the task API, and a server with news turned
//! off must never contact the feed host.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::sync::Arc;
use tracing::info;

use crate::handlers::TaskHandler;
use crate::types::TaskResult;
use codex_db::entities::tasks;
use codex_events::EventBroadcaster;
use codex_services::news::{FetchOutcome, NEWS_FEED_BASE_URL, NewsFetcher};

pub struct FetchNewsHandler {
    enabled: bool,
    base_url: String,
}

impl FetchNewsHandler {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            base_url: NEWS_FEED_BASE_URL.to_string(),
        }
    }

    /// Point the handler at another feed host. Tests only.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

impl TaskHandler for FetchNewsHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            if !self.enabled {
                info!("Task {}: news is disabled, not fetching", task.id);
                return Ok(TaskResult::success_with_data(
                    "News is disabled",
                    json!({ "skipped": true }),
                ));
            }

            let fetcher = NewsFetcher::with_base_url(&self.base_url)?;
            let results = fetcher.fetch_all(db).await;

            // A failing feed is already logged by the fetcher and is not a
            // task failure: retrying a host that is down, or that this server
            // cannot reach at all, only adds noise.
            let mut stored = 0;
            let mut feeds = serde_json::Map::new();
            for (project, result) in &results {
                let status = match result {
                    Ok(FetchOutcome::Updated(count)) => {
                        stored += count;
                        json!({ "status": "updated", "entries": count })
                    }
                    Ok(FetchOutcome::NotModified) => json!({ "status": "not_modified" }),
                    Err(error) => json!({ "status": "failed", "error": format!("{error:#}") }),
                };
                feeds.insert(project.clone(), status);
            }

            info!(
                "Task {}: news fetch complete, {} entries stored",
                task.id, stored
            );
            Ok(TaskResult::success_with_data(
                format!("Fetched news ({stored} entries stored)"),
                json!({ "feeds": feeds }),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::routing::get;
    use chrono::Utc;
    use codex_db::repositories::NewsEntryRepository;
    use codex_db::test_helpers::create_test_db;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

    fn task() -> tasks::Model {
        tasks::Model {
            id: Uuid::new_v4(),
            task_type: "fetch_news".to_string(),
            library_id: None,
            series_id: None,
            book_id: None,
            params: None,
            status: "processing".to_string(),
            priority: 100,
            locked_by: None,
            locked_until: None,
            attempts: 1,
            max_attempts: 3,
            last_error: None,
            reschedule_count: 0,
            max_reschedules: 10,
            result: None,
            scheduled_for: Utc::now(),
            created_at: Utc::now(),
            started_at: Some(Utc::now()),
            completed_at: None,
            progress: None,
        }
    }

    /// A feed host that counts requests and serves one Codex entry.
    async fn stub() -> (Arc<AtomicUsize>, String) {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let app = Router::new().route(
            "/feeds/{file}",
            get(move |axum::extract::Path(file): axum::extract::Path<String>| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    let project = file.trim_end_matches(".json").to_string();
                    format!(
                        r#"{{"version": 1, "entries": [{{"id": "{project}-1", "project": "{project}",
                        "kind": "post", "title": "t", "published_at": "2026-10-02T23:24:00Z",
                        "summary": "s", "body": "b"}}]}}"#
                    )
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (hits, format!("http://{addr}/feeds"))
    }

    #[tokio::test]
    async fn disabled_news_makes_no_request() {
        let (db, _t) = create_test_db().await;
        let (hits, base) = stub().await;

        let result = FetchNewsHandler::new(false)
            .with_base_url(base)
            .handle(&task(), db.sea_orm_connection(), None)
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(
            hits.load(Ordering::SeqCst),
            0,
            "no outbound request when disabled"
        );
    }

    #[tokio::test]
    async fn enabled_news_fetches_every_feed() {
        let (db, _t) = create_test_db().await;
        let (hits, base) = stub().await;

        let result = FetchNewsHandler::new(true)
            .with_base_url(base)
            .handle(&task(), db.sea_orm_connection(), None)
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(hits.load(Ordering::SeqCst), 2);
        let rows =
            NewsEntryRepository::list_recent(db.sea_orm_connection(), &["codex", "codex-ios"], 10)
                .await
                .unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[tokio::test]
    async fn an_unreachable_host_is_not_a_task_failure() {
        let (db, _t) = create_test_db().await;

        // Port 9 (discard) on loopback refuses connections.
        let result = FetchNewsHandler::new(true)
            .with_base_url("http://127.0.0.1:9/feeds")
            .handle(&task(), db.sea_orm_connection(), None)
            .await
            .unwrap();

        assert!(result.success);
    }
}
