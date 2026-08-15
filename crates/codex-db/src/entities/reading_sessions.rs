//! `SeaORM` Entity for the reading_sessions table
//!
//! An append-only log of reading activity, and the source of truth from which
//! `read_progress` and `read_completions` are folded.
//!
//! `read_progress` records where a reader ended up, which carries none of the
//! facts that produced it. Two clients writing that row can only be reconciled
//! by picking a winner, and a bare page number cannot distinguish "arrived most
//! recently" from "read furthest". Recording the activity restores that
//! distinction.
//!
//! Rows are inserted, never updated, with one exception: an append that falls
//! within the coalescing window of the previous session for the same device and
//! pass extends that row instead of adding another. OPDS page streaming writes
//! progress on every page turn, so without coalescing the log would grow one row
//! per page.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "reading_sessions")]
pub struct Model {
    /// Client-generated, so replaying a batch whose response was lost collides
    /// on the primary key rather than double-counting the reading.
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    pub book_id: Uuid,
    /// Stable per install. Producers with no device concept of their own
    /// (Komga, OPDS) derive one from the API key or user agent.
    pub device_id: String,
    pub device_name: Option<String>,
    /// Which read-through this belongs to. A `Reset` starts the next one.
    pub pass: i32,
    /// [`SessionKind`] as stored.
    pub kind: String,
    /// Position for comics and PDF.
    pub to_page: Option<i32>,
    /// Position for EPUB, as a fraction between 0.0 and 1.0.
    pub to_percentage: Option<f64>,
    /// R2Progression JSON (Readium standard) for EPUB position sync.
    pub r2_progression: Option<String>,
    /// Active reading time measured by the reader. Never derived from the
    /// client timestamps below, because wall-clock elapsed counts a book left
    /// open and untouched as reading. `None` when the producer cannot measure
    /// it, which is the honest value for OPDS, Komga, and KOReader.
    pub active_duration_ms: Option<i64>,
    /// [`DurationSource`] as stored. Measured and reconstructed time are kept
    /// distinguishable so statistics can report provenance instead of blending
    /// them into one untrustworthy total.
    pub duration_source: String,
    pub pages_read: Option<i32>,
    pub client_started_at: DateTime<Utc>,
    /// When the reading happened, by the client's clock. **This is the fold's
    /// primary sort key.** Ordering by arrival instead would let a stale
    /// session that syncs late overwrite a fresher one, which is the defect
    /// this table exists to fix.
    pub client_ended_at: DateTime<Utc>,
    /// Stamped by the server on arrival. A tiebreak only, so a device with a
    /// skewed clock cannot win ordering outright.
    pub server_recorded_at: DateTime<Utc>,
}

/// What a session row asserts.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionKind {
    /// Reading happened, and the position is where it ended.
    Progress,
    /// The book was finished.
    Completed,
    /// The book was marked unread, starting a new pass. Recording this as an
    /// event rather than as a deletion is what makes the ordering between
    /// "finished" and "starting over" recorded data instead of an accident of
    /// arrival order.
    Reset,
}

impl SessionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Progress => "progress",
            Self::Completed => "completed",
            Self::Reset => "reset",
        }
    }

    /// Unrecognised values parse as `Progress`. A row written by a newer server
    /// and read by an older one should degrade to "some reading happened" and
    /// keep the position, rather than being dropped from the fold entirely.
    pub fn from_str_lenient(value: &str) -> Self {
        match value {
            "completed" => Self::Completed,
            "reset" => Self::Reset,
            _ => Self::Progress,
        }
    }
}

/// Where a session's `active_duration_ms` came from.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DurationSource {
    /// A reader measured active time directly.
    Measured,
    /// Reconstructed server-side from the gaps between observed requests.
    /// Systematically undercounts, which is the honest direction to be wrong.
    Inferred,
    /// No duration information available.
    Unknown,
}

impl DurationSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Measured => "measured",
            Self::Inferred => "inferred",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_str_lenient(value: &str) -> Self {
        match value {
            "measured" => Self::Measured,
            "inferred" => Self::Inferred,
            _ => Self::Unknown,
        }
    }
}

impl Model {
    pub fn session_kind(&self) -> SessionKind {
        SessionKind::from_str_lenient(&self.kind)
    }

    pub fn duration_source(&self) -> DurationSource {
        DurationSource::from_str_lenient(&self.duration_source)
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::books::Entity",
        from = "Column::BookId",
        to = "super::books::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Books,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Users,
}

impl Related<super::books::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Books.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
