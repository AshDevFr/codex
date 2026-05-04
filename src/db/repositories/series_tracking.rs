//! Repository for the `series_tracking` sidecar table.
//!
//! Provides 1:1 read/write access to release-tracking metadata for a series
//! (whether it's tracked, current external chapter/volume, per-series overrides,
//! etc.). This repository is intentionally narrow - it doesn't reach into
//! `series_external_ids` (already its own repo) or `series_aliases` (sibling
//! repo); the release-tracking service composes them.

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::db::entities::series_tracking::{
    self, Entity as SeriesTracking, Model as SeriesTrackingRow, tracking_status,
};

/// Parameters for upserting a tracking row. Each `Option<Option<T>>` distinguishes
/// "leave alone" (`None`) from "explicitly clear" (`Some(None)`).
#[derive(Debug, Default, Clone)]
pub struct TrackingUpdate {
    pub tracked: Option<bool>,
    pub tracking_status: Option<String>,
    pub track_chapters: Option<bool>,
    pub track_volumes: Option<bool>,
    /// Outer `None` = leave alone; inner `None` = clear.
    pub latest_known_chapter: Option<Option<f64>>,
    pub latest_known_volume: Option<Option<i32>>,
    pub volume_chapter_map: Option<Option<serde_json::Value>>,
    pub poll_interval_override_s: Option<Option<i32>>,
    pub confidence_threshold_override: Option<Option<f64>>,
}

pub struct SeriesTrackingRepository;

impl SeriesTrackingRepository {
    /// Get the tracking row for a series, if one exists.
    pub async fn get(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Option<SeriesTrackingRow>> {
        let result = SeriesTracking::find_by_id(series_id).one(db).await?;
        Ok(result)
    }

    /// Get the tracking row, defaulting to a virtual untracked row if none exists.
    /// The returned row is NOT persisted unless explicitly upserted.
    pub async fn get_or_default(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<SeriesTrackingRow> {
        if let Some(row) = Self::get(db, series_id).await? {
            return Ok(row);
        }
        let now = Utc::now();
        Ok(SeriesTrackingRow {
            series_id,
            tracked: false,
            tracking_status: tracking_status::UNKNOWN.to_string(),
            track_chapters: true,
            track_volumes: true,
            latest_known_chapter: None,
            latest_known_volume: None,
            volume_chapter_map: None,
            poll_interval_override_s: None,
            confidence_threshold_override: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Upsert: insert if missing, otherwise apply the update fields. Fields with
    /// `None` in `update` are left untouched.
    pub async fn upsert(
        db: &DatabaseConnection,
        series_id: Uuid,
        update: TrackingUpdate,
    ) -> Result<SeriesTrackingRow> {
        // Validate tracking_status before doing any DB work.
        if let Some(ref status) = update.tracking_status
            && !tracking_status::is_valid(status)
        {
            anyhow::bail!("invalid tracking_status: {}", status);
        }

        let now = Utc::now();
        let existing = SeriesTracking::find_by_id(series_id).one(db).await?;

        match existing {
            Some(existing) => {
                let mut active: series_tracking::ActiveModel = existing.into();
                if let Some(v) = update.tracked {
                    active.tracked = Set(v);
                }
                if let Some(v) = update.tracking_status {
                    active.tracking_status = Set(v);
                }
                if let Some(v) = update.track_chapters {
                    active.track_chapters = Set(v);
                }
                if let Some(v) = update.track_volumes {
                    active.track_volumes = Set(v);
                }
                if let Some(v) = update.latest_known_chapter {
                    active.latest_known_chapter = Set(v);
                }
                if let Some(v) = update.latest_known_volume {
                    active.latest_known_volume = Set(v);
                }
                if let Some(v) = update.volume_chapter_map {
                    active.volume_chapter_map = Set(v);
                }
                if let Some(v) = update.poll_interval_override_s {
                    active.poll_interval_override_s = Set(v);
                }
                if let Some(v) = update.confidence_threshold_override {
                    active.confidence_threshold_override = Set(v);
                }
                active.updated_at = Set(now);
                let model = active.update(db).await?;
                Ok(model)
            }
            None => {
                let active = series_tracking::ActiveModel {
                    series_id: Set(series_id),
                    tracked: Set(update.tracked.unwrap_or(false)),
                    tracking_status: Set(update
                        .tracking_status
                        .unwrap_or_else(|| tracking_status::UNKNOWN.to_string())),
                    track_chapters: Set(update.track_chapters.unwrap_or(true)),
                    track_volumes: Set(update.track_volumes.unwrap_or(true)),
                    latest_known_chapter: Set(update.latest_known_chapter.unwrap_or(None)),
                    latest_known_volume: Set(update.latest_known_volume.unwrap_or(None)),
                    volume_chapter_map: Set(update.volume_chapter_map.unwrap_or(None)),
                    poll_interval_override_s: Set(update.poll_interval_override_s.unwrap_or(None)),
                    confidence_threshold_override: Set(update
                        .confidence_threshold_override
                        .unwrap_or(None)),
                    created_at: Set(now),
                    updated_at: Set(now),
                };
                let model = active.insert(db).await?;
                Ok(model)
            }
        }
    }

    /// Convenience: toggle `tracked` on an existing or virtual row.
    pub async fn set_tracked(
        db: &DatabaseConnection,
        series_id: Uuid,
        tracked: bool,
    ) -> Result<SeriesTrackingRow> {
        Self::upsert(
            db,
            series_id,
            TrackingUpdate {
                tracked: Some(tracked),
                ..Default::default()
            },
        )
        .await
    }

    /// List all tracked series IDs. Used by the polling service to enumerate
    /// what to ask plugins for. Paginated to keep memory bounded for large
    /// libraries; pass `limit = 0` for no limit (callers should normally page).
    pub async fn list_tracked_ids(
        db: &DatabaseConnection,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<Uuid>> {
        use sea_orm::QuerySelect;
        let mut query = SeriesTracking::find().filter(series_tracking::Column::Tracked.eq(true));
        if limit > 0 {
            query = query.limit(limit);
        }
        if offset > 0 {
            query = query.offset(offset);
        }
        let results = query.all(db).await?;
        Ok(results.into_iter().map(|m| m.series_id).collect())
    }

    /// Batched lookup: fetch tracking rows for many series in one query and
    /// return them keyed by `series_id`. Series without a tracking row are
    /// absent from the map (callers should treat this as untracked).
    pub async fn get_for_series_ids(
        db: &DatabaseConnection,
        series_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, SeriesTrackingRow>> {
        if series_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let rows = SeriesTracking::find()
            .filter(series_tracking::Column::SeriesId.is_in(series_ids.to_vec()))
            .all(db)
            .await?;
        Ok(rows.into_iter().map(|r| (r.series_id, r)).collect())
    }

    /// Count tracked series.
    pub async fn count_tracked(db: &DatabaseConnection) -> Result<u64> {
        use sea_orm::PaginatorTrait;
        let count = SeriesTracking::find()
            .filter(series_tracking::Column::Tracked.eq(true))
            .count(db)
            .await?;
        Ok(count)
    }

    /// Delete the tracking row for a series. Cascade from series delete handles
    /// the normal case; this is for explicit user-initiated "stop tracking and
    /// forget overrides."
    pub async fn delete(db: &DatabaseConnection, series_id: Uuid) -> Result<bool> {
        let result = SeriesTracking::delete_by_id(series_id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::repositories::{LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;

    async fn make_series(db: &DatabaseConnection) -> Uuid {
        let library =
            LibraryRepository::create(db, "Test Library", "/test/path", ScanningStrategy::Default)
                .await
                .unwrap();
        let series = SeriesRepository::create(db, library.id, "Test Series", None)
            .await
            .unwrap();
        series.id
    }

    #[tokio::test]
    async fn get_returns_none_when_no_row() {
        let (db, _temp) = create_test_db().await;
        let series_id = make_series(db.sea_orm_connection()).await;

        let row = SeriesTrackingRepository::get(db.sea_orm_connection(), series_id)
            .await
            .unwrap();
        assert!(row.is_none());
    }

    #[tokio::test]
    async fn get_or_default_returns_untracked_row() {
        let (db, _temp) = create_test_db().await;
        let series_id = make_series(db.sea_orm_connection()).await;

        let row = SeriesTrackingRepository::get_or_default(db.sea_orm_connection(), series_id)
            .await
            .unwrap();
        assert_eq!(row.series_id, series_id);
        assert!(!row.tracked);
        assert_eq!(row.tracking_status, "unknown");
        assert!(row.track_chapters);
        assert!(row.track_volumes);
    }

    #[tokio::test]
    async fn upsert_inserts_then_updates() {
        let (db, _temp) = create_test_db().await;
        let series_id = make_series(db.sea_orm_connection()).await;

        // First upsert inserts.
        let row = SeriesTrackingRepository::upsert(
            db.sea_orm_connection(),
            series_id,
            TrackingUpdate {
                tracked: Some(true),
                tracking_status: Some("ongoing".to_string()),
                latest_known_chapter: Some(Some(142.0)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(row.tracked);
        assert_eq!(row.tracking_status, "ongoing");
        assert_eq!(row.latest_known_chapter, Some(142.0));

        // Second upsert updates only specified fields.
        let row2 = SeriesTrackingRepository::upsert(
            db.sea_orm_connection(),
            series_id,
            TrackingUpdate {
                latest_known_chapter: Some(Some(143.0)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(row2.tracked, "tracked should be preserved");
        assert_eq!(
            row2.tracking_status, "ongoing",
            "status should be preserved"
        );
        assert_eq!(row2.latest_known_chapter, Some(143.0));
    }

    #[tokio::test]
    async fn upsert_can_clear_optional_fields() {
        let (db, _temp) = create_test_db().await;
        let series_id = make_series(db.sea_orm_connection()).await;

        SeriesTrackingRepository::upsert(
            db.sea_orm_connection(),
            series_id,
            TrackingUpdate {
                latest_known_chapter: Some(Some(50.0)),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // Explicit clear via Some(None).
        let cleared = SeriesTrackingRepository::upsert(
            db.sea_orm_connection(),
            series_id,
            TrackingUpdate {
                latest_known_chapter: Some(None),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(cleared.latest_known_chapter, None);
    }

    #[tokio::test]
    async fn upsert_rejects_invalid_status() {
        let (db, _temp) = create_test_db().await;
        let series_id = make_series(db.sea_orm_connection()).await;

        let err = SeriesTrackingRepository::upsert(
            db.sea_orm_connection(),
            series_id,
            TrackingUpdate {
                tracking_status: Some("paused".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("invalid tracking_status"));
    }

    #[tokio::test]
    async fn set_tracked_toggles_flag() {
        let (db, _temp) = create_test_db().await;
        let series_id = make_series(db.sea_orm_connection()).await;

        let row = SeriesTrackingRepository::set_tracked(db.sea_orm_connection(), series_id, true)
            .await
            .unwrap();
        assert!(row.tracked);

        let row = SeriesTrackingRepository::set_tracked(db.sea_orm_connection(), series_id, false)
            .await
            .unwrap();
        assert!(!row.tracked);
    }

    #[tokio::test]
    async fn list_tracked_ids_filters_to_tracked() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let s1 = SeriesRepository::create(conn, library.id, "A", None)
            .await
            .unwrap();
        let s2 = SeriesRepository::create(conn, library.id, "B", None)
            .await
            .unwrap();
        let _s3 = SeriesRepository::create(conn, library.id, "C", None)
            .await
            .unwrap();

        SeriesTrackingRepository::set_tracked(conn, s1.id, true)
            .await
            .unwrap();
        SeriesTrackingRepository::set_tracked(conn, s2.id, false)
            .await
            .unwrap();
        // s3 has no tracking row at all.

        let ids = SeriesTrackingRepository::list_tracked_ids(conn, 0, 0)
            .await
            .unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], s1.id);

        let count = SeriesTrackingRepository::count_tracked(conn).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn get_for_series_ids_returns_only_existing_rows() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let s1 = SeriesRepository::create(conn, library.id, "A", None)
            .await
            .unwrap();
        let s2 = SeriesRepository::create(conn, library.id, "B", None)
            .await
            .unwrap();
        let s3 = SeriesRepository::create(conn, library.id, "C", None)
            .await
            .unwrap();

        SeriesTrackingRepository::set_tracked(conn, s1.id, true)
            .await
            .unwrap();
        SeriesTrackingRepository::set_tracked(conn, s2.id, false)
            .await
            .unwrap();
        // s3 has no tracking row.

        let map = SeriesTrackingRepository::get_for_series_ids(conn, &[s1.id, s2.id, s3.id])
            .await
            .unwrap();
        assert_eq!(map.len(), 2);
        assert!(map.get(&s1.id).map(|r| r.tracked).unwrap_or(false));
        assert_eq!(map.get(&s2.id).map(|r| r.tracked), Some(false));
        assert!(!map.contains_key(&s3.id));

        let empty = SeriesTrackingRepository::get_for_series_ids(conn, &[])
            .await
            .unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn cascade_deletes_tracking_when_series_deleted() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let series_id = make_series(conn).await;

        SeriesTrackingRepository::set_tracked(conn, series_id, true)
            .await
            .unwrap();

        // Delete the series; tracking should follow via FK cascade.
        SeriesRepository::delete(conn, series_id).await.unwrap();

        let row = SeriesTrackingRepository::get(conn, series_id)
            .await
            .unwrap();
        assert!(row.is_none(), "tracking row should be cascaded away");
    }
}
