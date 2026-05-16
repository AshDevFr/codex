//! `BulkTrackForReleases` task handler.
//!
//! Drives the bulk-toggle work that used to happen synchronously inside the
//! `POST /series/bulk/{track,untrack}-for-releases` HTTP request. Each
//! series goes through the shared
//! [`crate::services::release::tracking_toggle`] helpers, which keep the
//! "track on -> seed first, then flip" / "track off -> flip only" ordering
//! identical to the per-series PATCH path.
//!
//! Per-series `SeriesUpdated` events are emitted from inside the loop (not
//! aggregated) so SSE consumers like the library grid and detail panel
//! refresh live as the task progresses, matching scan-style UX.

use anyhow::{Result, anyhow};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::db::entities::tasks;
use crate::events::{EventBroadcaster, TaskProgressEvent};
use crate::services::release::tracking_toggle::{
    ToggleOutcome, ToggleResult, track_one_series, untrack_one_series,
};
use crate::tasks::handlers::TaskHandler;
use crate::tasks::types::TaskResult;

pub struct BulkTrackForReleasesHandler;

impl BulkTrackForReleasesHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BulkTrackForReleasesHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskHandler for BulkTrackForReleasesHandler {
    fn handle<'a>(
        &'a self,
        task: &'a tasks::Model,
        db: &'a DatabaseConnection,
        event_broadcaster: Option<&'a Arc<EventBroadcaster>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TaskResult>> + Send + 'a>> {
        Box::pin(async move {
            let params = decode_params(task)?;

            let total = params.series_ids.len();
            info!(
                "Task {}: BulkTrackForReleases tracked={} for {} series",
                task.id, params.tracked, total
            );

            emit_progress(
                event_broadcaster,
                task,
                0,
                total,
                format!(
                    "Starting bulk {} for {} series",
                    if params.tracked { "track" } else { "untrack" },
                    total,
                ),
            );

            let mut summary = Summary::default();
            for (idx, series_id) in params.series_ids.iter().enumerate() {
                let result = if params.tracked {
                    track_one_series(db, event_broadcaster, None, *series_id).await
                } else {
                    untrack_one_series(db, event_broadcaster, None, *series_id).await
                };
                if result.outcome == ToggleOutcome::Errored {
                    warn!(
                        "Bulk track ({}): series {} errored: {}",
                        if params.tracked { "on" } else { "off" },
                        series_id,
                        result.detail.as_deref().unwrap_or("(no detail)"),
                    );
                }
                summary.absorb(&result);
                summary.results.push(result);

                let current = idx + 1;
                emit_progress(
                    event_broadcaster,
                    task,
                    current,
                    total,
                    format!(
                        "Processed {}/{} ({} changed, {} skipped, {} errored)",
                        current, total, summary.changed, summary.already_in_state, summary.errored,
                    ),
                );
            }

            info!(
                "Task {}: BulkTrackForReleases complete: {} changed, {} skipped, {} errored",
                task.id, summary.changed, summary.already_in_state, summary.errored,
            );

            Ok(TaskResult::success_with_data(
                format!(
                    "{} {} series ({} skipped, {} errored)",
                    if params.tracked {
                        "Tracked"
                    } else {
                        "Untracked"
                    },
                    summary.changed,
                    summary.already_in_state,
                    summary.errored,
                ),
                serde_json::json!({
                    "tracked": params.tracked,
                    "changed": summary.changed,
                    "already_in_state": summary.already_in_state,
                    "errored": summary.errored,
                    "results": summary.results,
                }),
            ))
        })
    }
}

struct DecodedParams {
    series_ids: Vec<Uuid>,
    tracked: bool,
}

fn decode_params(task: &tasks::Model) -> Result<DecodedParams> {
    let params = task
        .params
        .as_ref()
        .ok_or_else(|| anyhow!("BulkTrackForReleases task missing params"))?;
    let series_ids: Vec<Uuid> = params
        .get("series_ids")
        .ok_or_else(|| anyhow!("BulkTrackForReleases task missing series_ids in params"))
        .and_then(|v| serde_json::from_value(v.clone()).map_err(|e| anyhow!(e)))?;
    let tracked: bool = params
        .get("tracked")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| anyhow!("BulkTrackForReleases task missing tracked in params"))?;
    Ok(DecodedParams {
        series_ids,
        tracked,
    })
}

#[derive(Default)]
struct Summary {
    changed: usize,
    already_in_state: usize,
    errored: usize,
    results: Vec<ToggleResult>,
}

impl Summary {
    fn absorb(&mut self, result: &ToggleResult) {
        match result.outcome {
            ToggleOutcome::Tracked | ToggleOutcome::Untracked => self.changed += 1,
            ToggleOutcome::Skipped => self.already_in_state += 1,
            ToggleOutcome::Errored => self.errored += 1,
        }
    }
}

fn emit_progress(
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
    task: &tasks::Model,
    current: usize,
    total: usize,
    message: String,
) {
    if let Some(broadcaster) = event_broadcaster {
        let _ = broadcaster.emit_task(TaskProgressEvent::progress(
            task.id,
            "bulk_track_for_releases",
            current,
            total,
            Some(message),
            task.library_id,
            task.series_id,
            task.book_id,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::db::ScanningStrategy;
    use crate::db::repositories::{
        LibraryRepository, SeriesAliasRepository, SeriesRepository, SeriesTrackingRepository,
        TaskRepository, TrackingUpdate,
    };
    use crate::db::test_helpers::create_test_db;
    use crate::tasks::types::TaskType;

    async fn fetch_task(db: &DatabaseConnection, id: Uuid) -> tasks::Model {
        TaskRepository::get_by_id(db, id)
            .await
            .expect("get_by_id")
            .expect("task row")
    }

    async fn run_handler(db: &DatabaseConnection, task: &tasks::Model) -> serde_json::Value {
        let handler = BulkTrackForReleasesHandler::new();
        let result = handler
            .handle(task, db, None)
            .await
            .expect("handler should succeed");
        assert!(result.success, "handler should report success");
        result.data.expect("handler should return result data")
    }

    #[tokio::test]
    async fn handler_tracks_all_series_in_batch() {
        let (test_db, _temp) = create_test_db().await;
        let db = test_db.sea_orm_connection().clone();
        let lib = LibraryRepository::create(&db, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s1 = SeriesRepository::create(&db, lib.id, "Vinland Saga", None)
            .await
            .unwrap()
            .id;
        let s2 = SeriesRepository::create(&db, lib.id, "Berserk", None)
            .await
            .unwrap()
            .id;

        let task_id = TaskRepository::enqueue(
            &db,
            TaskType::BulkTrackForReleases {
                series_ids: vec![s1, s2],
                tracked: true,
            },
            None,
        )
        .await
        .unwrap();
        let task = fetch_task(&db, task_id).await;
        let data = run_handler(&db, &task).await;

        assert_eq!(data["tracked"], true);
        assert_eq!(data["changed"], 2);
        assert_eq!(data["already_in_state"], 0);
        assert_eq!(data["errored"], 0);
        assert_eq!(data["results"].as_array().unwrap().len(), 2);

        for series_id in [s1, s2] {
            let row = SeriesTrackingRepository::get(&db, series_id)
                .await
                .unwrap()
                .unwrap();
            assert!(row.tracked, "series {} should be tracked", series_id);
            let aliases = SeriesAliasRepository::get_for_series(&db, series_id)
                .await
                .unwrap();
            assert!(!aliases.is_empty(), "seed should run on track-on");
        }
    }

    #[tokio::test]
    async fn handler_untracks_all_series_in_batch() {
        let (test_db, _temp) = create_test_db().await;
        let db = test_db.sea_orm_connection().clone();
        let lib = LibraryRepository::create(&db, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s = SeriesRepository::create(&db, lib.id, "Tracked", None)
            .await
            .unwrap()
            .id;
        SeriesTrackingRepository::upsert(
            &db,
            s,
            TrackingUpdate {
                tracked: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let task_id = TaskRepository::enqueue(
            &db,
            TaskType::BulkTrackForReleases {
                series_ids: vec![s],
                tracked: false,
            },
            None,
        )
        .await
        .unwrap();
        let task = fetch_task(&db, task_id).await;
        let data = run_handler(&db, &task).await;

        assert_eq!(data["tracked"], false);
        assert_eq!(data["changed"], 1);
        assert_eq!(data["already_in_state"], 0);
        assert_eq!(data["errored"], 0);

        let row = SeriesTrackingRepository::get(&db, s)
            .await
            .unwrap()
            .unwrap();
        assert!(!row.tracked);
    }

    #[tokio::test]
    async fn handler_treats_missing_series_as_skipped() {
        let (test_db, _temp) = create_test_db().await;
        let db = test_db.sea_orm_connection().clone();
        let lib = LibraryRepository::create(&db, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let real = SeriesRepository::create(&db, lib.id, "Real", None)
            .await
            .unwrap()
            .id;
        let bogus = Uuid::new_v4();

        let task_id = TaskRepository::enqueue(
            &db,
            TaskType::BulkTrackForReleases {
                series_ids: vec![bogus, real],
                tracked: true,
            },
            None,
        )
        .await
        .unwrap();
        let task = fetch_task(&db, task_id).await;
        let data = run_handler(&db, &task).await;

        // bogus -> skipped (counts toward already_in_state, matching the
        // legacy sync endpoint's bookkeeping); real -> changed.
        assert_eq!(data["changed"], 1);
        assert_eq!(data["already_in_state"], 1);
        assert_eq!(data["errored"], 0);

        let results = data["results"].as_array().unwrap();
        let bogus_row = results
            .iter()
            .find(|r| r["series_id"].as_str() == Some(&bogus.to_string()))
            .expect("bogus row present");
        assert_eq!(bogus_row["outcome"], "skipped");
        assert!(
            bogus_row["detail"]
                .as_str()
                .unwrap_or("")
                .contains("not found"),
            "missing-series detail should mention 'not found'"
        );
    }

    #[tokio::test]
    async fn handler_marks_already_tracked_as_skipped() {
        let (test_db, _temp) = create_test_db().await;
        let db = test_db.sea_orm_connection().clone();
        let lib = LibraryRepository::create(&db, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let already = SeriesRepository::create(&db, lib.id, "Already", None)
            .await
            .unwrap()
            .id;
        let fresh = SeriesRepository::create(&db, lib.id, "Fresh", None)
            .await
            .unwrap()
            .id;
        SeriesTrackingRepository::upsert(
            &db,
            already,
            TrackingUpdate {
                tracked: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let task_id = TaskRepository::enqueue(
            &db,
            TaskType::BulkTrackForReleases {
                series_ids: vec![already, fresh],
                tracked: true,
            },
            None,
        )
        .await
        .unwrap();
        let task = fetch_task(&db, task_id).await;
        let data = run_handler(&db, &task).await;

        assert_eq!(data["changed"], 1);
        assert_eq!(data["already_in_state"], 1);
        assert_eq!(data["errored"], 0);

        let results = data["results"].as_array().unwrap();
        // Preserves input order.
        assert_eq!(
            results[0]["series_id"].as_str(),
            Some(already.to_string().as_str())
        );
        assert_eq!(results[0]["outcome"], "skipped");
        assert_eq!(
            results[1]["series_id"].as_str(),
            Some(fresh.to_string().as_str())
        );
        assert_eq!(results[1]["outcome"], "tracked");
    }

    #[tokio::test]
    async fn handler_marks_already_untracked_as_skipped() {
        let (test_db, _temp) = create_test_db().await;
        let db = test_db.sea_orm_connection().clone();
        let lib = LibraryRepository::create(&db, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let never = SeriesRepository::create(&db, lib.id, "Never tracked", None)
            .await
            .unwrap()
            .id;

        let task_id = TaskRepository::enqueue(
            &db,
            TaskType::BulkTrackForReleases {
                series_ids: vec![never],
                tracked: false,
            },
            None,
        )
        .await
        .unwrap();
        let task = fetch_task(&db, task_id).await;
        let data = run_handler(&db, &task).await;

        assert_eq!(data["changed"], 0);
        assert_eq!(data["already_in_state"], 1);
        assert_eq!(data["errored"], 0);
    }

    #[tokio::test]
    async fn handler_aggregates_mixed_batch_counts_correctly() {
        let (test_db, _temp) = create_test_db().await;
        let db = test_db.sea_orm_connection().clone();
        let lib = LibraryRepository::create(&db, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let fresh = SeriesRepository::create(&db, lib.id, "Fresh", None)
            .await
            .unwrap()
            .id;
        let already = SeriesRepository::create(&db, lib.id, "Already", None)
            .await
            .unwrap()
            .id;
        SeriesTrackingRepository::upsert(
            &db,
            already,
            TrackingUpdate {
                tracked: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let bogus = Uuid::new_v4();

        let task_id = TaskRepository::enqueue(
            &db,
            TaskType::BulkTrackForReleases {
                series_ids: vec![fresh, already, bogus],
                tracked: true,
            },
            None,
        )
        .await
        .unwrap();
        let task = fetch_task(&db, task_id).await;
        let data = run_handler(&db, &task).await;

        assert_eq!(data["changed"], 1);
        // already-tracked + missing both fold into already_in_state, matching
        // the legacy sync endpoint's counting.
        assert_eq!(data["already_in_state"], 2);
        assert_eq!(data["errored"], 0);
        assert_eq!(data["results"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn handler_rejects_task_missing_params() {
        let (test_db, _temp) = create_test_db().await;
        let db = test_db.sea_orm_connection().clone();
        // Hand-craft a task row with no params; enqueueing via TaskType always
        // populates params, so go through the entity directly.
        use sea_orm::{ActiveModelTrait, Set};
        let task_id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let model = tasks::ActiveModel {
            id: Set(task_id),
            task_type: Set("bulk_track_for_releases".to_string()),
            library_id: Set(None),
            series_id: Set(None),
            book_id: Set(None),
            params: Set(None),
            status: Set("pending".to_string()),
            priority: Set(155),
            locked_by: Set(None),
            locked_until: Set(None),
            attempts: Set(0),
            max_attempts: Set(3),
            last_error: Set(None),
            reschedule_count: Set(0),
            max_reschedules: Set(5),
            result: Set(None),
            scheduled_for: Set(now),
            created_at: Set(now),
            started_at: Set(None),
            completed_at: Set(None),
        };
        let task = model.insert(&db).await.unwrap();

        let handler = BulkTrackForReleasesHandler::new();
        let err = handler
            .handle(&task, &db, None)
            .await
            .expect_err("handler must reject task with no params");
        assert!(
            err.to_string().contains("missing params"),
            "error must mention missing params, got: {err}"
        );
    }

    #[test]
    fn handler_creation() {
        let _ = BulkTrackForReleasesHandler::new();
        let _ = BulkTrackForReleasesHandler;
    }
}
