//! `BackfillTrackingFromMetadata` task handler.
//!
//! Walks series in scope and seeds `series_aliases` rows from existing metadata
//! (canonical title + alternate titles). Idempotent on re-run — `SeriesAliasRepository::create`
//! returns the existing row when the same alias already exists for a series.
//!
//! Does NOT toggle `tracked`. Enabling tracking is always an explicit user
//! action; this task is a one-time data-prep pass that the admin can run after
//! upgrading or after a metadata refresh.

use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::entities::series_aliases::alias_source;
use crate::db::entities::tasks;
use crate::db::repositories::{
    AlternateTitleRepository, SeriesAliasRepository, SeriesMetadataRepository, SeriesRepository,
};
use crate::events::EventBroadcaster;
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
                match backfill_one(db, series_id).await {
                    Ok(per_series) => {
                        summary.merge(per_series);
                    }
                    Err(e) => {
                        warn!("Backfill failed for series {}: {}", series_id, e);
                        summary.errors += 1;
                    }
                }
            }

            info!(
                "Backfill complete ({}): {} series processed, {} aliases inserted, {} skipped, {} errors",
                scope,
                summary.processed,
                summary.aliases_inserted,
                summary.aliases_skipped_duplicate,
                summary.errors,
            );

            Ok(TaskResult::success_with_data(
                format!(
                    "Processed {} series, inserted {} new aliases ({} duplicates skipped, {} errors)",
                    summary.processed,
                    summary.aliases_inserted,
                    summary.aliases_skipped_duplicate,
                    summary.errors,
                ),
                serde_json::json!({
                    "scope": scope,
                    "series_processed": summary.processed,
                    "aliases_inserted": summary.aliases_inserted,
                    "aliases_skipped_duplicate": summary.aliases_skipped_duplicate,
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
    errors: usize,
}

impl BackfillSummary {
    fn merge(&mut self, other: PerSeriesSummary) {
        self.processed += 1;
        self.aliases_inserted += other.inserted;
        self.aliases_skipped_duplicate += other.skipped_duplicate;
    }
}

#[derive(Default)]
struct PerSeriesSummary {
    inserted: usize,
    skipped_duplicate: usize,
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

async fn backfill_one(db: &DatabaseConnection, series_id: Uuid) -> Result<PerSeriesSummary> {
    let metadata = match SeriesMetadataRepository::get_by_series_id(db, series_id).await? {
        Some(m) => m,
        None => {
            // Metadata is required for a series to exist normally; if missing,
            // the series row is in an unexpected state - skip it.
            debug!("Series {} has no metadata, skipping", series_id);
            return Ok(PerSeriesSummary::default());
        }
    };

    let mut candidates: Vec<String> = Vec::new();
    candidates.push(metadata.title.clone());
    if let Some(sort) = metadata.title_sort.as_ref()
        && !sort.trim().is_empty()
    {
        candidates.push(sort.clone());
    }

    let alt_titles = AlternateTitleRepository::get_for_series(db, series_id).await?;
    for alt in alt_titles {
        if !alt.title.trim().is_empty() {
            candidates.push(alt.title);
        }
    }

    let mut summary = PerSeriesSummary::default();
    for alias in candidates {
        let trimmed = alias.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Track inserts vs idempotent skips by counting before/after.
        let before = SeriesAliasRepository::count_for_series(db, series_id).await?;
        match SeriesAliasRepository::create(db, series_id, trimmed, alias_source::METADATA).await {
            Ok(_) => {
                let after = SeriesAliasRepository::count_for_series(db, series_id).await?;
                if after > before {
                    summary.inserted += 1;
                } else {
                    summary.skipped_duplicate += 1;
                }
            }
            Err(e) => {
                // Aliases that normalize to empty (e.g., "!!!---!!!" entries from
                // odd metadata) are non-fatal — log and skip.
                debug!(
                    "Skipping alias '{}' for series {}: {}",
                    trimmed, series_id, e
                );
            }
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::repositories::{LibraryRepository, SeriesAliasRepository, SeriesRepository};
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

    #[tokio::test]
    async fn handler_seeds_aliases_from_title_and_alternates() {
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

        let summary = backfill_one(conn, s1).await.unwrap();
        assert_eq!(summary.inserted, 2);
        assert_eq!(summary.skipped_duplicate, 0);

        let aliases = SeriesAliasRepository::get_for_series(conn, s1)
            .await
            .unwrap();
        let texts: Vec<&str> = aliases.iter().map(|a| a.alias.as_str()).collect();
        assert!(texts.contains(&"My Hero Academia"));
        assert!(texts.contains(&"僕のヒーローアカデミア"));
        assert!(aliases.iter().all(|a| a.source == "metadata"));
    }

    #[tokio::test]
    async fn handler_is_idempotent_on_rerun() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s1 = make_series(conn, lib.id, "Series A", Some("Alt A")).await;

        let first = backfill_one(conn, s1).await.unwrap();
        assert_eq!(first.inserted, 2);

        let second = backfill_one(conn, s1).await.unwrap();
        assert_eq!(second.inserted, 0, "re-run should not insert duplicates");
        assert_eq!(second.skipped_duplicate, 2);

        let aliases = SeriesAliasRepository::get_for_series(conn, s1)
            .await
            .unwrap();
        assert_eq!(aliases.len(), 2);
    }

    #[tokio::test]
    async fn handler_does_not_enable_tracking() {
        use crate::db::repositories::SeriesTrackingRepository;
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s1 = make_series(conn, lib.id, "Some Title", None).await;

        backfill_one(conn, s1).await.unwrap();

        let row = SeriesTrackingRepository::get(conn, s1).await.unwrap();
        assert!(
            row.is_none(),
            "backfill should not create or modify tracking row"
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
