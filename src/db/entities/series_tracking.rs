//! `SeaORM` entity for the `series_tracking` table.
//!
//! 1:1 sidecar to `series` carrying release-tracking flags. Lives in its own
//! table (not on `series` directly) so the subsystem stays cleanly separable -
//! disabling release tracking is a no-join, and removing it later doesn't
//! require a destructive migration on the core series table.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "series_tracking")]
pub struct Model {
    /// Primary key AND foreign key to series.id (1:1 sidecar).
    #[sea_orm(primary_key, auto_increment = false)]
    pub series_id: Uuid,
    /// Whether release tracking is enabled for this series.
    pub tracked: bool,
    /// 'ongoing' | 'complete' | 'hiatus' | 'cancelled' | 'unknown'.
    pub tracking_status: String,
    pub track_chapters: bool,
    pub track_volumes: bool,
    /// Latest external chapter (decimal handles 12.5, 110.1, etc.).
    pub latest_known_chapter: Option<f64>,
    pub latest_known_volume: Option<i32>,
    /// Sparse map: `{ "<volume>": { "first": <ch>, "last": <ch> } }`.
    pub volume_chapter_map: Option<serde_json::Value>,
    /// Per-series override of the source's poll interval (seconds). Null = use source default.
    pub poll_interval_override_s: Option<i32>,
    /// Per-series override of the server's confidence threshold. Null = use server default.
    pub confidence_threshold_override: Option<f64>,
    /// Per-series language preference (ISO 639-1 codes, e.g. `["en", "es"]`).
    /// `None` = fall back to the server-wide default (`release_tracking.default_languages`).
    /// Used by aggregation feeds like MangaUpdates that emit candidates in many
    /// languages; the plugin filters client-side before recording.
    pub languages: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::series::Entity",
        from = "Column::SeriesId",
        to = "super::series::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Series,
}

impl Related<super::series::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Series.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Canonical strings for `tracking_status`.
pub mod tracking_status {
    pub const ONGOING: &str = "ongoing";
    pub const COMPLETE: &str = "complete";
    pub const HIATUS: &str = "hiatus";
    pub const CANCELLED: &str = "cancelled";
    pub const UNKNOWN: &str = "unknown";

    pub fn is_valid(s: &str) -> bool {
        matches!(s, ONGOING | COMPLETE | HIATUS | CANCELLED | UNKNOWN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracking_status_validates_known_values() {
        assert!(tracking_status::is_valid("ongoing"));
        assert!(tracking_status::is_valid("complete"));
        assert!(tracking_status::is_valid("hiatus"));
        assert!(tracking_status::is_valid("cancelled"));
        assert!(tracking_status::is_valid("unknown"));
        assert!(!tracking_status::is_valid("paused"));
        assert!(!tracking_status::is_valid(""));
    }
}
