//! Repository for the append-only reading session log.
//!
//! Loading, appending, and persisting only. Every rule about what a reader's
//! state *is* lives in [`fold`], which is pure and testable without a database.

#![allow(dead_code)]

pub mod fold;

pub use fold::{Fold, FoldedCompletion, FoldedProgress, fold};

use crate::entities::{
    reading_sessions,
    reading_sessions::{DurationSource, Entity as ReadingSessions, SessionKind},
};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use sea_orm::*;
use uuid::Uuid;

/// How close together two sessions from the same device must be to merge.
///
/// OPDS page streaming writes progress on every page turn, so without merging,
/// the log would grow one row per page and the resulting statistics would be
/// meaningless. This must stay equal to the client-side idle timeout: a reader
/// that pauses and resumes inside the window is one session, and the two ends
/// have to agree on where that boundary falls or the same reading is counted
/// differently depending on which client reported it.
pub const COALESCE_WINDOW_MINUTES: i64 = 5;

/// An event to append to the log.
///
/// `pass` is deliberately absent: it is derived on append from the log's
/// current state, so callers cannot assign an inconsistent one.
#[derive(Clone, Debug)]
pub struct NewSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub book_id: Uuid,
    pub device_id: String,
    pub device_name: Option<String>,
    pub kind: SessionKind,
    pub to_page: Option<i32>,
    pub to_percentage: Option<f64>,
    pub r2_progression: Option<String>,
    pub active_duration_ms: Option<i64>,
    pub duration_source: DurationSource,
    pub pages_read: Option<i32>,
    pub client_started_at: DateTime<Utc>,
    pub client_ended_at: DateTime<Utc>,
}

impl NewSession {
    /// A session synthesized by a legacy write path (the native v1 routes, the
    /// Komga compatibility layer, KOReader sync, OPDS page streaming).
    ///
    /// Those surfaces report a position but cannot measure reading time, so the
    /// duration is left unknown rather than guessed at. The client timestamps
    /// collapse to the moment of the write, which is the best available truth
    /// when the producer reports an instant rather than a span.
    pub fn from_legacy_write(
        user_id: Uuid,
        book_id: Uuid,
        device_id: impl Into<String>,
        kind: SessionKind,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            book_id,
            device_id: device_id.into(),
            device_name: None,
            kind,
            to_page: None,
            to_percentage: None,
            r2_progression: None,
            active_duration_ms: None,
            duration_source: DurationSource::Unknown,
            pages_read: None,
            client_started_at: now,
            client_ended_at: now,
        }
    }

    pub fn with_page(mut self, page: i32) -> Self {
        self.to_page = Some(page);
        self
    }

    pub fn with_percentage(mut self, percentage: Option<f64>) -> Self {
        self.to_percentage = percentage;
        self
    }

    pub fn with_progression(mut self, progression: Option<String>) -> Self {
        self.r2_progression = progression;
        self
    }
}

pub struct ReadingSessionRepository;

impl ReadingSessionRepository {
    /// Every session for one user and book, in fold order.
    ///
    /// Ordered in SQL by the same key [`fold::sort_key`] applies, so the index
    /// does the work and the fold's defensive re-sort is a no-op.
    pub async fn load_for_book<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<Vec<reading_sessions::Model>> {
        let sessions = ReadingSessions::find()
            .filter(reading_sessions::Column::UserId.eq(user_id))
            .filter(reading_sessions::Column::BookId.eq(book_id))
            .order_by_asc(reading_sessions::Column::Pass)
            .order_by_asc(reading_sessions::Column::ClientEndedAt)
            .order_by_asc(reading_sessions::Column::ServerRecordedAt)
            .all(db)
            .await?;

        Ok(sessions)
    }

    /// The pass a new event of `kind` belongs to.
    ///
    /// A reset opens the next pass rather than closing the current one, so the
    /// reset row and everything read after it share a pass number. That is what
    /// lets the fold recognise "reset and nothing since" as a pass with no
    /// reading, which must project to no progress row at all.
    async fn next_pass<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
        kind: SessionKind,
    ) -> Result<i32> {
        let current: Option<i32> = ReadingSessions::find()
            .filter(reading_sessions::Column::UserId.eq(user_id))
            .filter(reading_sessions::Column::BookId.eq(book_id))
            .select_only()
            .column_as(reading_sessions::Column::Pass.max(), "max_pass")
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten();

        let current = current.unwrap_or(1);
        Ok(match kind {
            SessionKind::Reset => current + 1,
            _ => current,
        })
    }

    /// Append an event, merging it into the previous session where that is the
    /// honest representation of what happened.
    pub async fn append<C: ConnectionTrait>(
        db: &C,
        session: NewSession,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let pass = Self::next_pass(db, session.user_id, session.book_id, session.kind).await?;

        if let Some(existing) = Self::coalesce_candidate(db, &session, pass).await? {
            Self::extend(db, existing, &session, now).await?;
            return Ok(());
        }

        let model = reading_sessions::ActiveModel {
            id: Set(session.id),
            user_id: Set(session.user_id),
            book_id: Set(session.book_id),
            device_id: Set(session.device_id),
            device_name: Set(session.device_name),
            pass: Set(pass),
            kind: Set(session.kind.as_str().to_string()),
            to_page: Set(session.to_page),
            to_percentage: Set(session.to_percentage),
            r2_progression: Set(session.r2_progression),
            active_duration_ms: Set(session.active_duration_ms),
            duration_source: Set(session.duration_source.as_str().to_string()),
            pages_read: Set(session.pages_read),
            client_started_at: Set(session.client_started_at),
            client_ended_at: Set(session.client_ended_at),
            server_recorded_at: Set(now),
        };
        model.insert(db).await?;

        Ok(())
    }

    /// The session this event should merge into, if any.
    ///
    /// Only plain reading merges. A completion or a reset marks a transition,
    /// and the fold reads those transitions in order, so folding one into a
    /// neighbouring row would erase the very thing it needs to see.
    async fn coalesce_candidate<C: ConnectionTrait>(
        db: &C,
        session: &NewSession,
        pass: i32,
    ) -> Result<Option<reading_sessions::Model>> {
        if session.kind != SessionKind::Progress {
            return Ok(None);
        }

        let window_start = session.client_ended_at - Duration::minutes(COALESCE_WINDOW_MINUTES);

        let candidate = ReadingSessions::find()
            .filter(reading_sessions::Column::UserId.eq(session.user_id))
            .filter(reading_sessions::Column::BookId.eq(session.book_id))
            .filter(reading_sessions::Column::DeviceId.eq(session.device_id.clone()))
            .filter(reading_sessions::Column::Pass.eq(pass))
            .filter(reading_sessions::Column::Kind.eq(SessionKind::Progress.as_str()))
            .filter(reading_sessions::Column::ClientEndedAt.gte(window_start))
            .filter(reading_sessions::Column::ClientEndedAt.lte(session.client_ended_at))
            .order_by_desc(reading_sessions::Column::ClientEndedAt)
            .one(db)
            .await?;

        Ok(candidate)
    }

    /// Merge an event into an existing session.
    ///
    /// Durations and page counts are **summed** rather than recomputed from the
    /// endpoints: the merged row spans a pause, and wall-clock across that span
    /// is not reading time.
    async fn extend<C: ConnectionTrait>(
        db: &C,
        existing: reading_sessions::Model,
        session: &NewSession,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let mut active: reading_sessions::ActiveModel = existing.clone().into();

        if session.to_page.is_some() {
            active.to_page = Set(session.to_page);
        }
        if session.to_percentage.is_some() {
            active.to_percentage = Set(session.to_percentage);
        }
        if session.r2_progression.is_some() {
            active.r2_progression = Set(session.r2_progression.clone());
        }

        active.client_ended_at = Set(session.client_ended_at.max(existing.client_ended_at));
        active.client_started_at = Set(session.client_started_at.min(existing.client_started_at));
        active.server_recorded_at = Set(now);

        active.active_duration_ms = Set(sum_options(
            existing.active_duration_ms,
            session.active_duration_ms,
        ));
        active.pages_read = Set(sum_options(existing.pages_read, session.pages_read));

        // A merged row is only as trustworthy as its weakest contributor: if
        // either side's duration was reconstructed rather than measured, the
        // total is reconstructed.
        let merged_source = match (existing.duration_source(), session.duration_source) {
            (DurationSource::Measured, DurationSource::Measured) => DurationSource::Measured,
            (DurationSource::Unknown, other) | (other, DurationSource::Unknown) => other,
            _ => DurationSource::Inferred,
        };
        active.duration_source = Set(merged_source.as_str().to_string());

        active.update(db).await?;
        Ok(())
    }

    /// Delete every session for a user and book, for an explicit history reset.
    pub async fn delete_for_book<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<u64> {
        let result = ReadingSessions::delete_many()
            .filter(reading_sessions::Column::UserId.eq(user_id))
            .filter(reading_sessions::Column::BookId.eq(book_id))
            .exec(db)
            .await?;

        Ok(result.rows_affected)
    }
}

/// Sum two optional counters, treating absence as "nothing to add" while
/// preserving `None` when neither side reported anything.
fn sum_options<T: std::ops::Add<Output = T> + Copy>(
    left: Option<T>,
    right: Option<T>,
) -> Option<T> {
    match (left, right) {
        (Some(a), Some(b)) => Some(a + b),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summing_optional_counters_preserves_absence() {
        assert_eq!(sum_options(Some(3), Some(4)), Some(7));
        assert_eq!(sum_options(Some(3), None), Some(3));
        assert_eq!(sum_options(None, Some(4)), Some(4));
        assert_eq!(sum_options::<i64>(None, None), None);
    }
}
