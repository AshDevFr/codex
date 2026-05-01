//! `SeaORM` entity for the `release_ledger` table.
//!
//! Dedup-keyed announcement ledger. Sources write rows here; the inbox UI
//! reads from it. Dedup keys: `(source_id, external_release_id)` and
//! `info_hash` (where present). Cross-source duplicates (Nyaa + MangaDex
//! both seeing ch47) become two ledger rows; the UI groups them at display
//! time so the user can pick a source.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "release_ledger")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub series_id: Uuid,
    pub source_id: Uuid,
    /// Plugin-stable identity for the release. Required for dedup.
    pub external_release_id: String,
    /// Optional. Torrent sources have it; HTTP sources don't.
    pub info_hash: Option<String>,
    /// Decimal handles 12.5, 110.1, etc.
    pub chapter: Option<f64>,
    pub volume: Option<i32>,
    pub language: Option<String>,
    /// `{ "jxl": true, "container": "cbz", ... }`.
    pub format_hints: Option<serde_json::Value>,
    pub group_or_uploader: Option<String>,
    /// Where the user goes to acquire (Nyaa torrent page, MangaDex chapter, ...).
    pub payload_url: String,
    pub confidence: f64,
    /// `announced` | `dismissed` | `marked_acquired` | `hidden`.
    pub state: String,
    pub metadata: Option<serde_json::Value>,
    pub observed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
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
    #[sea_orm(
        belongs_to = "super::release_sources::Entity",
        from = "Column::SourceId",
        to = "super::release_sources::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    ReleaseSource,
}

impl Related<super::series::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Series.def()
    }
}

impl Related<super::release_sources::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReleaseSource.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Canonical strings for `state`.
pub mod state {
    pub const ANNOUNCED: &str = "announced";
    pub const DISMISSED: &str = "dismissed";
    pub const MARKED_ACQUIRED: &str = "marked_acquired";
    pub const HIDDEN: &str = "hidden";

    pub fn is_valid(s: &str) -> bool {
        matches!(s, ANNOUNCED | DISMISSED | MARKED_ACQUIRED | HIDDEN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_validates_known_values() {
        assert!(state::is_valid("announced"));
        assert!(state::is_valid("dismissed"));
        assert!(state::is_valid("marked_acquired"));
        assert!(state::is_valid("hidden"));
        assert!(!state::is_valid("acquired"));
        assert!(!state::is_valid("new"));
        assert!(!state::is_valid(""));
    }
}
