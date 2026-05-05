//! `BackfillTrackingFromMetadata` task handler.
//!
//! Walks series in scope and (re-)seeds tracking defaults from existing
//! data: aliases from metadata, `latest_known_*` from local book
//! classification, and per-axis `track_*` flags from book metadata. Routes
//! through `services::release::seed::seed_tracking_for_series` so the per-
//! series PATCH path, the bulk track-for-releases endpoint, and this task
//! all share one canonical seeding implementation.
//!
//! Does NOT toggle `tracked`. Enabling tracking is always an explicit user
//! action; this task is a maintenance pass that refreshes auto-derived
//! fields after a metadata refresh or library re-scan.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::db::entities::tasks;
use crate::db::repositories::SeriesRepository;
use crate::events::EventBroadcaster;
use crate::services::release::seed::{SeedReport, seed_tracking_for_series};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

pub struct BackfillTrackingFromMetadataHandler;

impl BackfillTrackingFromMetadataHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BackfillTrackingFromMetadataHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskHandler for BackfillTrackingFromMetadataHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        _event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let library_id = task.library_id;
            let series_ids: Option<Vec<Uuid>> = task
                .params
                .as_ref()
                .and_then(|p| p.get("series_ids"))
                .and_then(|v| serde_json::from_value(v.clone()).ok());

            let scope = describe_scope(library_id, series_ids.as_deref());
            info!("Task {}: Backfilling tracking aliases ({})", task.id, scope);

            let series_to_process = resolve_series_scope(db, library_id, series_ids).await?;
            let total = series_to_process.len();
            info!("Found {} series in scope", total);

            let mut summary = BackfillSummary::default();
            for series_id in series_to_process {
                match seed_tracking_for_series(db, series_id).await {
                    Ok(report) => summary.merge(report),
                    Err(e) => {
                        warn!("Seed failed for series {}: {}", series_id, e);
                        summary.errors += 1;
                    }
                }
            }

            info!(
                "Backfill complete ({}): {} series processed, {} aliases inserted, \
                 {} skipped duplicate, {} skipped non-latin, {} errors",
                scope,
                summary.processed,
                summary.aliases_inserted,
                summary.aliases_skipped_duplicate,
                summary.aliases_skipped_non_latin,
                summary.errors,
            );

            Ok(TaskResult::success_with_data(
                format!(
                    "Processed {} series, inserted {} new aliases \
                     ({} duplicates, {} non-Latin skipped, {} errors)",
                    summary.processed,
                    summary.aliases_inserted,
                    summary.aliases_skipped_duplicate,
                    summary.aliases_skipped_non_latin,
                    summary.errors,
                ),
                serde_json::json!({
                    "scope": scope,
                    "series_processed": summary.processed,
                    "aliases_inserted": summary.aliases_inserted,
                    "aliases_skipped_duplicate": summary.aliases_skipped_duplicate,
                    "aliases_skipped_non_latin": summary.aliases_skipped_non_latin,
                    "errors": summary.errors,
                }),
            ))
        })
    }
}

#[derive(Default)]
struct BackfillSummary {
    processed: usize,
    aliases_inserted: usize,
    aliases_skipped_duplicate: usize,
    aliases_skipped_non_latin: usize,
    errors: usize,
}

impl BackfillSummary {
    fn merge(&mut self, report: SeedReport) {
        self.processed += 1;
        self.aliases_inserted += report.aliases_inserted;
        self.aliases_skipped_duplicate += report.aliases_skipped_duplicate;
        self.aliases_skipped_non_latin += report.aliases_skipped_non_latin;
    }
}

fn describe_scope(library_id: Option<Uuid>, series_ids: Option<&[Uuid]>) -> String {
    match (library_id, series_ids) {
        (_, Some(ids)) => format!("scope=series_ids:{}", ids.len()),
        (Some(lib), _) => format!("scope=library:{}", lib),
        (None, None) => "scope=all".to_string(),
    }
}

async fn resolve_series_scope(
    db: &DatabaseConnection,
    library_id: Option<Uuid>,
    series_ids: Option<Vec<Uuid>>,
) -> Result<Vec<Uuid>> {
    if let Some(ids) = series_ids {
        return Ok(ids);
    }
    if let Some(lib_id) = library_id {
        let series_list = SeriesRepository::list_by_library(db, lib_id).await?;
        return Ok(series_list.into_iter().map(|s| s.id).collect());
    }
    let all = SeriesRepository::list_all(db).await?;
    Ok(all.into_iter().map(|s| s.id).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::repositories::{
        AlternateTitleRepository, LibraryRepository, SeriesAliasRepository, SeriesRepository,
        SeriesTrackingRepository,
    };
    use crate::db::test_helpers::create_test_db;

    async fn make_series(
        db: &DatabaseConnection,
        library_id: Uuid,
        name: &str,
        japanese: Option<&str>,
    ) -> Uuid {
        let series = SeriesRepository::create(db, library_id, name, None)
            .await
            .unwrap();
        if let Some(jp) = japanese {
            AlternateTitleRepository::create(db, series.id, "Japanese", jp)
                .await
                .unwrap();
        }
        series.id
    }

    /// The handler now delegates to `seed_tracking_for_series`; this test
    /// pins the latin-only filtering behavior at the seeded layer (the
    /// previous handler-internal logic seeded all scripts and is gone).
    #[tokio::test]
    async fn delegated_seed_inserts_latin_aliases_skipping_non_latin() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s1 = make_series(
            conn,
            lib.id,
            "My Hero Academia",
            Some("僕のヒーローアカデミア"),
        )
        .await;

        let report = seed_tracking_for_series(conn, s1).await.unwrap();
        // "My Hero Academia" appears in both `series.name` and metadata title;
        // dedup folds them. Japanese alt is skipped.
        assert_eq!(report.aliases_inserted, 1);
        assert_eq!(report.aliases_skipped_non_latin, 1);

        let aliases = SeriesAliasRepository::get_for_series(conn, s1)
            .await
            .unwrap();
        let texts: Vec<&str> = aliases.iter().map(|a| a.alias.as_str()).collect();
        assert!(texts.contains(&"My Hero Academia"));
        assert!(!texts.iter().any(|a| a.contains('僕')));
    }

    #[tokio::test]
    async fn delegated_seed_is_idempotent_on_rerun() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s1 = make_series(conn, lib.id, "Series A", Some("Alt A")).await;

        let first = seed_tracking_for_series(conn, s1).await.unwrap();
        // "Series A" + "Alt A" — both Latin, both inserted.
        assert_eq!(first.aliases_inserted, 2);

        let second = seed_tracking_for_series(conn, s1).await.unwrap();
        assert_eq!(second.aliases_inserted, 0);
        assert_eq!(second.aliases_skipped_duplicate, 2);

        let aliases = SeriesAliasRepository::get_for_series(conn, s1)
            .await
            .unwrap();
        assert_eq!(aliases.len(), 2);
    }

    #[tokio::test]
    async fn delegated_seed_does_not_enable_tracking() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s1 = make_series(conn, lib.id, "Some Title", None).await;

        seed_tracking_for_series(conn, s1).await.unwrap();

        let row = SeriesTrackingRepository::get(conn, s1).await.unwrap();
        assert!(
            row.map(|r| !r.tracked).unwrap_or(true),
            "seeding must not flip `tracked` on"
        );
    }

    #[tokio::test]
    async fn resolve_scope_prefers_explicit_series_ids() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s1 = make_series(conn, lib.id, "A", None).await;
        let _s2 = make_series(conn, lib.id, "B", None).await;

        let scoped = resolve_series_scope(conn, Some(lib.id), Some(vec![s1]))
            .await
            .unwrap();
        assert_eq!(scoped, vec![s1]);
    }

    #[tokio::test]
    async fn resolve_scope_library_returns_all_in_library() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib1 = LibraryRepository::create(conn, "L1", "/p1", ScanningStrategy::Default)
            .await
            .unwrap();
        let lib2 = LibraryRepository::create(conn, "L2", "/p2", ScanningStrategy::Default)
            .await
            .unwrap();
        let _a = make_series(conn, lib1.id, "A", None).await;
        let _b = make_series(conn, lib1.id, "B", None).await;
        let _c = make_series(conn, lib2.id, "C", None).await;

        let scoped = resolve_series_scope(conn, Some(lib1.id), None)
            .await
            .unwrap();
        assert_eq!(scoped.len(), 2);
    }

    #[tokio::test]
    async fn resolve_scope_no_args_returns_all_series() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let _a = make_series(conn, lib.id, "A", None).await;
        let _b = make_series(conn, lib.id, "B", None).await;

        let scoped = resolve_series_scope(conn, None, None).await.unwrap();
        assert_eq!(scoped.len(), 2);
    }

    #[test]
    fn describe_scope_strings() {
        let lib = Uuid::new_v4();
        assert!(describe_scope(None, None).starts_with("scope=all"));
        assert!(describe_scope(Some(lib), None).starts_with("scope=library:"));
        assert_eq!(
            describe_scope(Some(lib), Some(&[Uuid::new_v4(), Uuid::new_v4()])),
            "scope=series_ids:2",
        );
    }

    #[test]
    fn handler_creation() {
        let _ = BackfillTrackingFromMetadataHandler::new();
        let _ = BackfillTrackingFromMetadataHandler;
    }
}
