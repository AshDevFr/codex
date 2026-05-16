//! Toggle a single series's `series_tracking.tracked` flag.
//!
//! Shared by the per-series PATCH handler (via the bulk HTTP fall-through),
//! the bulk-track-for-releases HTTP endpoint, and the
//! `BulkTrackForReleases` async task handler. Centralizing here keeps the
//! "track on -> seed first, then flip" / "track off -> flip only" order
//! identical across all three call sites and ensures any future change to
//! the transition logic happens in one place.

use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::db::repositories::{SeriesRepository, SeriesTrackingRepository, TrackingUpdate};
use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster};
use crate::services::release::seed::seed_tracking_for_series;

/// Discrete outcomes for a single-series toggle attempt.
///
/// Serialized as lowercase strings so the existing frontend that already
/// renders `"tracked" | "untracked" | "skipped" | "errored"` from the sync
/// HTTP response keeps working when the same values are surfaced through
/// the task `result_data` JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToggleOutcome {
    /// `tracked` flipped from `false` to `true`.
    Tracked,
    /// `tracked` flipped from `true` to `false`.
    Untracked,
    /// No change: either already in the target state or the series was
    /// missing (see `detail` for which).
    Skipped,
    /// Internal error during processing (see `detail`).
    Errored,
}

impl ToggleOutcome {
    /// Lower-case wire string. Matches the legacy HTTP shape.
    pub fn as_str(self) -> &'static str {
        match self {
            ToggleOutcome::Tracked => "tracked",
            ToggleOutcome::Untracked => "untracked",
            ToggleOutcome::Skipped => "skipped",
            ToggleOutcome::Errored => "errored",
        }
    }
}

/// Per-series outcome row, mirroring the legacy `BulkTrackForReleasesItem`
/// fields so consumers can render the same row markup off either source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToggleResult {
    pub series_id: Uuid,
    pub outcome: ToggleOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Flip a single series to `tracked = true`, running the seed pass first so
/// auto-derived fields (aliases, `latest_known_*`, per-axis `track_*`) are
/// populated before the flag flips. Idempotent: already-tracked series are
/// reported as `Skipped` without re-running the seed.
///
/// Errors are *captured* in the returned `ToggleResult`, never propagated.
/// This is intentional: bulk callers want to keep processing the rest of
/// the batch when one series fails.
pub async fn track_one_series(
    db: &DatabaseConnection,
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
    user_id: Option<Uuid>,
    series_id: Uuid,
) -> ToggleResult {
    let series = match SeriesRepository::get_by_id(db, series_id).await {
        Ok(Some(s)) => s,
        Ok(None) => return skipped(series_id, "series not found"),
        Err(e) => return errored(series_id, format!("lookup failed: {}", e)),
    };

    let already_tracked = SeriesTrackingRepository::get(db, series_id)
        .await
        .ok()
        .flatten()
        .map(|r| r.tracked)
        .unwrap_or(false);
    if already_tracked {
        return skipped(series_id, "already tracked");
    }

    if let Err(e) = seed_tracking_for_series(db, series_id).await {
        return errored(series_id, format!("seed failed: {}", e));
    }
    let update = TrackingUpdate {
        tracked: Some(true),
        ..Default::default()
    };
    if let Err(e) = SeriesTrackingRepository::upsert(db, series_id, update).await {
        return errored(series_id, format!("upsert failed: {}", e));
    }

    emit_series_updated(event_broadcaster, series_id, series.library_id, user_id);

    ToggleResult {
        series_id,
        outcome: ToggleOutcome::Tracked,
        detail: None,
    }
}

/// Flip a single series to `tracked = false`. Does not touch aliases,
/// `latest_known_*`, or other tracking config so re-tracking later is
/// non-destructive. Already-untracked (or never-tracked) series are
/// reported as `Skipped`.
pub async fn untrack_one_series(
    db: &DatabaseConnection,
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
    user_id: Option<Uuid>,
    series_id: Uuid,
) -> ToggleResult {
    let series = match SeriesRepository::get_by_id(db, series_id).await {
        Ok(Some(s)) => s,
        Ok(None) => return skipped(series_id, "series not found"),
        Err(e) => return errored(series_id, format!("lookup failed: {}", e)),
    };

    let already_untracked = SeriesTrackingRepository::get(db, series_id)
        .await
        .ok()
        .flatten()
        .map(|r| !r.tracked)
        .unwrap_or(true);
    if already_untracked {
        return skipped(series_id, "already untracked");
    }

    let update = TrackingUpdate {
        tracked: Some(false),
        ..Default::default()
    };
    if let Err(e) = SeriesTrackingRepository::upsert(db, series_id, update).await {
        return errored(series_id, format!("upsert failed: {}", e));
    }

    emit_series_updated(event_broadcaster, series_id, series.library_id, user_id);

    ToggleResult {
        series_id,
        outcome: ToggleOutcome::Untracked,
        detail: None,
    }
}

fn emit_series_updated(
    event_broadcaster: Option<&Arc<EventBroadcaster>>,
    series_id: Uuid,
    library_id: Uuid,
    user_id: Option<Uuid>,
) {
    if let Some(broadcaster) = event_broadcaster {
        let event = EntityChangeEvent {
            event: EntityEvent::SeriesUpdated {
                series_id,
                library_id,
                fields: Some(vec!["tracking".to_string()]),
            },
            timestamp: Utc::now(),
            user_id,
        };
        let _ = broadcaster.emit(event);
    }
}

fn skipped(series_id: Uuid, reason: impl Into<String>) -> ToggleResult {
    ToggleResult {
        series_id,
        outcome: ToggleOutcome::Skipped,
        detail: Some(reason.into()),
    }
}

fn errored(series_id: Uuid, reason: impl Into<String>) -> ToggleResult {
    ToggleResult {
        series_id,
        outcome: ToggleOutcome::Errored,
        detail: Some(reason.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::db::ScanningStrategy;
    use crate::db::repositories::{
        LibraryRepository, SeriesAliasRepository, SeriesRepository, SeriesTrackingRepository,
        TrackingUpdate,
    };
    use crate::db::test_helpers::create_test_db;

    async fn make_series(db: &DatabaseConnection, library_id: Uuid, name: &str) -> Uuid {
        SeriesRepository::create(db, library_id, name, None)
            .await
            .unwrap()
            .id
    }

    #[tokio::test]
    async fn track_one_flips_tracked_and_seeds_aliases() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s = make_series(conn, lib.id, "Vinland Saga").await;

        let result = track_one_series(conn, None, None, s).await;
        assert_eq!(result.outcome, ToggleOutcome::Tracked);
        assert!(result.detail.is_none());

        let row = SeriesTrackingRepository::get(conn, s)
            .await
            .unwrap()
            .unwrap();
        assert!(row.tracked);

        let aliases = SeriesAliasRepository::get_for_series(conn, s)
            .await
            .unwrap();
        assert!(!aliases.is_empty(), "seed should insert an alias");
    }

    #[tokio::test]
    async fn track_one_is_idempotent_when_already_tracked() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s = make_series(conn, lib.id, "Berserk").await;
        SeriesTrackingRepository::upsert(
            conn,
            s,
            TrackingUpdate {
                tracked: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let result = track_one_series(conn, None, None, s).await;
        assert_eq!(result.outcome, ToggleOutcome::Skipped);
        assert_eq!(result.detail.as_deref(), Some("already tracked"));
    }

    #[tokio::test]
    async fn track_one_reports_missing_series_as_skipped() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let bogus = Uuid::new_v4();

        let result = track_one_series(conn, None, None, bogus).await;
        assert_eq!(result.outcome, ToggleOutcome::Skipped);
        assert!(
            result.detail.as_deref().unwrap_or("").contains("not found"),
            "missing series detail should mention 'not found'"
        );
    }

    #[tokio::test]
    async fn untrack_one_flips_tracked_off_and_preserves_aliases() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s = make_series(conn, lib.id, "Tracked").await;
        SeriesTrackingRepository::upsert(
            conn,
            s,
            TrackingUpdate {
                tracked: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        SeriesAliasRepository::create(conn, s, "User Alias", "manual")
            .await
            .unwrap();

        let result = untrack_one_series(conn, None, None, s).await;
        assert_eq!(result.outcome, ToggleOutcome::Untracked);

        let row = SeriesTrackingRepository::get(conn, s)
            .await
            .unwrap()
            .unwrap();
        assert!(!row.tracked);

        // Manual alias must survive the untrack — the soft-toggle contract.
        let aliases = SeriesAliasRepository::get_for_series(conn, s)
            .await
            .unwrap();
        assert!(aliases.iter().any(|a| a.alias == "User Alias"));
    }

    #[tokio::test]
    async fn untrack_one_is_idempotent_when_never_tracked() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let lib = LibraryRepository::create(conn, "L", "/p", ScanningStrategy::Default)
            .await
            .unwrap();
        let s = make_series(conn, lib.id, "Never tracked").await;

        let result = untrack_one_series(conn, None, None, s).await;
        assert_eq!(result.outcome, ToggleOutcome::Skipped);
        assert_eq!(result.detail.as_deref(), Some("already untracked"));
    }

    #[tokio::test]
    async fn untrack_one_reports_missing_series_as_skipped() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let bogus = Uuid::new_v4();

        let result = untrack_one_series(conn, None, None, bogus).await;
        assert_eq!(result.outcome, ToggleOutcome::Skipped);
        assert!(result.detail.as_deref().unwrap_or("").contains("not found"));
    }

    #[test]
    fn toggle_outcome_strings_match_legacy_wire_shape() {
        assert_eq!(ToggleOutcome::Tracked.as_str(), "tracked");
        assert_eq!(ToggleOutcome::Untracked.as_str(), "untracked");
        assert_eq!(ToggleOutcome::Skipped.as_str(), "skipped");
        assert_eq!(ToggleOutcome::Errored.as_str(), "errored");

        let serialized = serde_json::to_string(&ToggleOutcome::Tracked).unwrap();
        assert_eq!(serialized, "\"tracked\"");
    }
}
