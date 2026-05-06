//! `SeaORM` entity for the `release_sources` table.
//!
//! One row per logical source a plugin (or core) exposes. A single plugin can
//! expose many sources: e.g., the Nyaa plugin exposes one source per uploader
//! subscription. Source-level state (poll cadence, last-poll status, ETag /
//! cursor) lives here so the scheduler and reverse-RPC handlers can manage
//! sources without round-tripping through the plugin.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "release_sources")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// Owning plugin id (string). The literal `"core"` is reserved for in-core
    /// synthetic sources (e.g., metadata-piggyback in Phase 5).
    pub plugin_id: String,
    /// Plugin-defined unique key (e.g., `nyaa:user:tsuna69`).
    pub source_key: String,
    pub display_name: String,
    /// `rss-uploader` | `rss-series` | `api-feed` | `metadata-feed` | `metadata-piggyback`.
    pub kind: String,
    pub enabled: bool,
    /// 5-field POSIX cron expression. NULL means "inherit the server-wide
    /// `release_tracking.default_cron_schedule` setting." The host
    /// normalizes to the 6-field format expected by `tokio-cron-scheduler`
    /// at scheduler-load time.
    pub cron_schedule: Option<String>,
    pub last_polled_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub etag: Option<String>,
    pub config: Option<serde_json::Value>,
    /// One-line human-readable summary of the most recent poll (e.g.
    /// `"fetched 12 items, matched 0, recorded 0"`). Written by the
    /// poll-source task on every successful completion. NULL until the
    /// first successful poll. Surfaced by the Release tracking settings UI
    /// under the per-row status badge so users can tell *why* a poll
    /// returned no announcements without grepping container logs.
    pub last_summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::release_ledger::Entity")]
    ReleaseLedger,
}

impl Related<super::release_ledger::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReleaseLedger.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Canonical strings for `plugin_id`.
pub mod plugin_id {
    /// In-core synthetic sources (e.g., metadata-piggyback in Phase 5). Not a
    /// real plugin; bypasses plugin-host lookup.
    #[allow(dead_code)] // wired up in Phase 5 (metadata piggyback)
    pub const CORE: &str = "core";
}

/// Canonical strings for `kind`.
pub mod kind {
    pub const RSS_UPLOADER: &str = "rss-uploader";
    pub const RSS_SERIES: &str = "rss-series";
    pub const API_FEED: &str = "api-feed";
    pub const METADATA_FEED: &str = "metadata-feed";
    pub const METADATA_PIGGYBACK: &str = "metadata-piggyback";

    pub fn is_valid(s: &str) -> bool {
        matches!(
            s,
            RSS_UPLOADER | RSS_SERIES | API_FEED | METADATA_FEED | METADATA_PIGGYBACK
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_validates_known_values() {
        assert!(kind::is_valid("rss-uploader"));
        assert!(kind::is_valid("rss-series"));
        assert!(kind::is_valid("api-feed"));
        assert!(kind::is_valid("metadata-feed"));
        assert!(kind::is_valid("metadata-piggyback"));
        assert!(!kind::is_valid("rss"));
        assert!(!kind::is_valid("api"));
        assert!(!kind::is_valid(""));
    }
}
