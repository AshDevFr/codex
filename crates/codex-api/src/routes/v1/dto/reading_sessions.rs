//! DTOs for the batched reading-session endpoint.
//!
//! Clients accumulate sessions while offline and replay them in one request, so
//! the shapes here are built around a batch that may be partially applicable:
//! individual entries can be rejected without failing their neighbours, and the
//! whole batch can be sent again safely if the response never arrived.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::ReadProgressResponse;

/// What a submitted session asserts about a book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ReadingSessionKindDto {
    /// Reading happened; `toPage` or `toPercentage` is where it ended.
    Progress,
    /// The book was finished.
    Completed,
    /// The book was marked unread, starting a new read-through.
    Reset,
}

impl From<ReadingSessionKindDto> for codex_db::entities::reading_sessions::SessionKind {
    fn from(dto: ReadingSessionKindDto) -> Self {
        match dto {
            ReadingSessionKindDto::Progress => Self::Progress,
            ReadingSessionKindDto::Completed => Self::Completed,
            ReadingSessionKindDto::Reset => Self::Reset,
        }
    }
}

/// One reading session measured by a client.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadingSessionDto {
    /// Client-generated identifier for this session.
    ///
    /// Submitting the same id twice is a no-op, so a batch whose response was
    /// lost can be replayed without double-counting the reading.
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// The book this session belongs to.
    #[schema(example = "550e8400-e29b-41d4-a716-446655440002")]
    pub book_id: Uuid,

    /// Stable identifier for the device that produced this session.
    #[schema(example = "8f3d1c7a-2b44-4e51-9c2a-1f7d3e5b9a01")]
    pub device_id: String,

    /// Human-readable device name, for the reading statistics UI.
    #[schema(example = "Ash's iPhone")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,

    /// What this session asserts.
    pub kind: ReadingSessionKindDto,

    /// Page reached, for comics and PDF (1-indexed).
    #[schema(example = 42)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_page: Option<i32>,

    /// Fraction read, for EPUB (0.0 to 1.0).
    #[schema(example = 0.45)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_percentage: Option<f64>,

    /// R2Progression JSON (Readium standard) for EPUB position sync.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r2_progression: Option<String>,

    /// Active reading time in milliseconds, measured by the reader.
    ///
    /// Must be time the reader actually spent reading, with the timer paused
    /// while the app is backgrounded, the screen is locked, or the reader has
    /// been idle past the timeout. Wall-clock elapsed is not this value, and
    /// anything larger than the session's own span is clamped to it.
    #[schema(example = 934_000_i64)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_duration_ms: Option<i64>,

    /// Distinct pages advanced through during this session.
    #[schema(example = 31)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages_read: Option<i32>,

    /// When this session began, by the client's clock.
    #[schema(example = "2026-08-14T19:02:11Z")]
    pub client_started_at: DateTime<Utc>,

    /// When this session ended, by the client's clock.
    ///
    /// This is what orders sessions against each other, so it must reflect when
    /// the reading happened rather than when it is being submitted. A session
    /// read offline and synced hours later still sorts by when it was read.
    #[schema(example = "2026-08-14T19:20:45Z")]
    pub client_ended_at: DateTime<Utc>,
}

/// A batch of sessions to record.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecordReadingSessionsRequest {
    /// The sessions to record. Order within the batch does not matter.
    pub sessions: Vec<ReadingSessionDto>,
}

/// Why a session could not be recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReadingSessionRejectionReason {
    /// No such book, or it is not visible to this user. Expected when a book is
    /// deleted while a client is offline.
    BookNotFound,
    /// `clientEndedAt` precedes `clientStartedAt`.
    InvalidTimeRange,
    /// `toPercentage` is outside 0.0 to 1.0.
    InvalidPercentage,
    /// `activeDurationMs` or `pagesRead` is negative.
    InvalidMeasurement,
    /// The same id appears more than once within this batch.
    DuplicateInBatch,
}

/// One entry that was not recorded, and why.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RejectedReadingSessionDto {
    /// The submitted session's id.
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,

    /// Why it was rejected.
    pub reason: ReadingSessionRejectionReason,
}

/// The outcome of recording a batch.
///
/// Never fails wholesale for a bad entry: an unknown book is an ordinary
/// consequence of syncing after a deletion, and it must not block the rest of a
/// client's queued reading from being accepted.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecordReadingSessionsResponse {
    /// Ids that are now in the log. Includes ids that were already present, so
    /// a client can clear its outbox on either outcome.
    pub accepted: Vec<Uuid>,

    /// Entries that were not recorded.
    pub rejected: Vec<RejectedReadingSessionDto>,

    /// Current progress for every book touched by an accepted session, so the
    /// client can reconcile immediately without a second round trip.
    pub progress: Vec<ReadProgressResponse>,
}
