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

/// Who produced a write that carries no session of its own.
///
/// The compatibility surfaces report a position and nothing else: no device, no
/// reading time. This reconstructs both from what the request does carry, so
/// reading done through Komga apps, OPDS readers and KOReader is not simply
/// absent from the record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceContext {
    pub id: String,
    pub name: Option<String>,
    /// Whether reading time may be reconstructed from the gaps between this
    /// device's writes.
    ///
    /// Off for anything whose client also reports measured sessions. The web
    /// reader writes progress *and* posts sessions for the same reading, so
    /// inferring from its progress writes as well would count that reading
    /// twice, once measured and once reconstructed.
    pub infer_duration: bool,
}

impl DeviceContext {
    /// A write from a client that reports its own sessions, or from inside
    /// Codex. Attributed, but never a source of inferred time.
    pub fn measured_elsewhere(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            infer_duration: false,
        }
    }

    /// The historical catch-all, for writes with nothing to attribute them to.
    pub fn legacy() -> Self {
        Self::measured_elsewhere(LEGACY_DEVICE_ID)
    }

    /// A progress write from a client that also reports measured sessions.
    ///
    /// These carry a position and nothing else, and exist only to keep the
    /// projection live *during* a sitting: the client's measured session does
    /// not arrive until the sitting ends. Attributing them to the client's own
    /// device is what lets the measured session recognise and absorb them when
    /// it does arrive, instead of leaving two rows describing one sitting.
    pub fn session_reporting_client(device_id: impl Into<String>) -> Self {
        Self {
            id: device_id.into(),
            name: None,
            infer_duration: false,
        }
    }

    /// A background job acting on the user's behalf, such as a tracker sync
    /// marking books read. Real reading did not happen, so nothing is inferred.
    pub fn internal() -> Self {
        Self {
            id: "codex-internal".to_string(),
            name: Some("Codex".to_string()),
            infer_duration: false,
        }
    }

    /// A request authenticated with an API key.
    ///
    /// The key is the most durable device identity these protocols offer, which
    /// is why the docs recommend one key per device: it turns an anonymous
    /// stream of progress writes into a named device in the statistics.
    pub fn api_key(key_id: Uuid, name: impl Into<String>) -> Self {
        Self {
            id: format!("apikey:{key_id}"),
            name: Some(name.into()),
            infer_duration: true,
        }
    }

    /// Last resort when a request carries no key: a hash of the user agent.
    ///
    /// Two devices running the same app collapse into one identity here. That
    /// is the honest limit of what the request tells us, and the fix is an API
    /// key rather than a cleverer hash.
    pub fn user_agent(user_agent: &str) -> Self {
        Self {
            id: format!("ua:{:x}", stable_hash(user_agent)),
            name: Some(friendly_agent_name(user_agent)),
            infer_duration: true,
        }
    }

    /// A KOReader device, which uniquely among the compat surfaces sends its
    /// own identity.
    pub fn koreader(device_id: &str, device_name: &str) -> Self {
        Self {
            id: format!("koreader:{device_id}"),
            name: Some(if device_name.is_empty() {
                "KOReader".to_string()
            } else {
                device_name.to_string()
            }),
            infer_duration: true,
        }
    }
}

/// Device identity for writes with no attributable origin.
pub const LEGACY_DEVICE_ID: &str = "legacy";

/// Map a user agent onto a recognisable client name.
///
/// Deliberately a short list of the clients people actually point at Codex.
/// Anything unrecognised keeps its raw agent string, which is more useful in a
/// statistics table than "Unknown".
fn friendly_agent_name(user_agent: &str) -> String {
    const KNOWN: [(&str, &str); 6] = [
        ("KOReader", "KOReader"),
        ("Komic", "Komic"),
        ("Chunky", "Chunky"),
        ("Panels", "Panels"),
        ("Paperback", "Paperback"),
        ("Moon+", "Moon+ Reader"),
    ];

    for (needle, label) in KNOWN {
        if user_agent.contains(needle) {
            return label.to_string();
        }
    }

    // Long agent strings are useless as a label and unbounded in length.
    user_agent.chars().take(60).collect()
}

/// FNV-1a. Not cryptographic, and does not need to be: this only has to be
/// stable across processes so one device keeps one identity between restarts.
fn stable_hash(value: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

/// What happened to an appended event.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AppendOutcome {
    /// A new row was written.
    Inserted,
    /// Merged into the preceding session from the same device.
    Merged,
    /// This id was already in the log, so nothing changed. Replaying a batch
    /// whose response was lost lands here.
    Duplicate,
}

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
    /// Whether this event may merge into the preceding session from the same
    /// device.
    ///
    /// True only for events synthesized from a legacy write, where the id is
    /// generated here and nothing will ever refer to it again. A merge discards
    /// the incoming id, so allowing it for a client-submitted session would
    /// break idempotent replay: the id would be absent from the log, the replay
    /// would look new, and its duration would be added a second time. Clients
    /// measure their own sessions as units and have already done this merging.
    pub coalesce: bool,
    /// Whether merging this event into the previous session should count the
    /// gap between them as reading time. See [`DeviceContext::infer_duration`].
    pub infer_duration: bool,
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
        device: &DeviceContext,
        kind: SessionKind,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            book_id,
            device_id: device.id.clone(),
            device_name: device.name.clone(),
            infer_duration: device.infer_duration,
            kind,
            to_page: None,
            to_percentage: None,
            r2_progression: None,
            active_duration_ms: None,
            duration_source: DurationSource::Unknown,
            pages_read: None,
            client_started_at: now,
            client_ended_at: now,
            coalesce: true,
        }
    }

    /// A session measured and reported by a client.
    ///
    /// The id comes from the client so a replayed batch is recognised rather
    /// than counted twice, which is also why these never merge.
    ///
    /// The reported duration is clamped to the wall-clock span: a client cannot
    /// have been actively reading for longer than the session lasted, so a
    /// larger figure is a bug in the client and is truncated rather than
    /// trusted. A backwards span clamps to zero.
    #[allow(clippy::too_many_arguments)]
    pub fn from_client(
        id: Uuid,
        user_id: Uuid,
        book_id: Uuid,
        device_id: impl Into<String>,
        device_name: Option<String>,
        kind: SessionKind,
        active_duration_ms: Option<i64>,
        pages_read: Option<i32>,
        client_started_at: DateTime<Utc>,
        client_ended_at: DateTime<Utc>,
    ) -> Self {
        let clamped = active_duration_ms.map(|reported| {
            let span = (client_ended_at - client_started_at).num_milliseconds();
            reported.clamp(0, span.max(0))
        });

        Self {
            id,
            user_id,
            book_id,
            device_id: device_id.into(),
            device_name,
            kind,
            to_page: None,
            to_percentage: None,
            r2_progression: None,
            active_duration_ms: clamped,
            // A client that reported nothing leaves the provenance unknown
            // rather than claiming a measurement it did not make.
            duration_source: if clamped.is_some() {
                DurationSource::Measured
            } else {
                DurationSource::Unknown
            },
            pages_read,
            client_started_at,
            client_ended_at,
            coalesce: false,
            infer_duration: false,
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

    /// The sessions of the current pass only, which is all the fold reads.
    ///
    /// Earlier passes are history: their completions were banked while they
    /// were current and the fold discards them. Loading them anyway makes the
    /// cost of every write grow with how many times a book has been re-read,
    /// which is a strange thing for a page turn to depend on.
    ///
    /// Two queries rather than a subquery, because the first is answered
    /// entirely from the fold index and the second is the same lookup the
    /// unfiltered load does, only narrower.
    pub async fn load_current_pass<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<Vec<reading_sessions::Model>> {
        let Some(current_pass) = Self::max_pass(db, user_id, book_id).await? else {
            return Ok(Vec::new());
        };

        let sessions = ReadingSessions::find()
            .filter(reading_sessions::Column::UserId.eq(user_id))
            .filter(reading_sessions::Column::BookId.eq(book_id))
            .filter(reading_sessions::Column::Pass.eq(current_pass))
            .order_by_asc(reading_sessions::Column::ClientEndedAt)
            .order_by_asc(reading_sessions::Column::ServerRecordedAt)
            .all(db)
            .await?;

        Ok(sessions)
    }

    /// The highest pass recorded for a book, or `None` when the log is empty.
    async fn max_pass<C: ConnectionTrait>(
        db: &C,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<Option<i32>> {
        let max = ReadingSessions::find()
            .filter(reading_sessions::Column::UserId.eq(user_id))
            .filter(reading_sessions::Column::BookId.eq(book_id))
            .select_only()
            .column_as(reading_sessions::Column::Pass.max(), "max_pass")
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten();

        Ok(max)
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
        let current = Self::max_pass(db, user_id, book_id).await?.unwrap_or(1);
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
    ) -> Result<AppendOutcome> {
        // Before anything else: an id already in the log is a replay, and a
        // replay must change nothing. This has to precede the merge check,
        // because merging would consume the incoming id without storing it and
        // the next replay would look new.
        if ReadingSessions::find_by_id(session.id)
            .one(db)
            .await?
            .is_some()
        {
            return Ok(AppendOutcome::Duplicate);
        }

        let pass = Self::next_pass(db, session.user_id, session.book_id, session.kind).await?;

        if let Some(existing) = Self::coalesce_candidate(db, &session, pass).await? {
            Self::extend(db, existing, &session, now).await?;
            return Ok(AppendOutcome::Merged);
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
        let inserted = model.insert(db).await?;

        // A measured session describes a whole sitting, including everything
        // the position-only writes made during it already said. Fold them in
        // now that the authoritative row exists.
        if session.duration_source == DurationSource::Measured {
            Self::supersede_position_only(db, &inserted).await?;
        }

        Ok(AppendOutcome::Inserted)
    }

    /// Absorb and remove the position-only rows a measured session covers.
    ///
    /// A client that measures its own sessions still has to write progress as
    /// it reads, because the projection has to stay live and the measured
    /// session does not arrive until the sitting ends. Those writes are
    /// synthesized into position-only rows. Once the measured row lands it says
    /// everything they said and more, so leaving them behind would make one
    /// sitting look like several and put a second device in the statistics.
    ///
    /// Scoped to the same device and pass, and to rows that carry no time of
    /// their own, so this can only ever remove the client's own redundant
    /// writes. Anything measured or reconstructed is left alone.
    async fn supersede_position_only<C: ConnectionTrait>(
        db: &C,
        measured: &reading_sessions::Model,
    ) -> Result<u64> {
        let covered = ReadingSessions::find()
            .filter(reading_sessions::Column::UserId.eq(measured.user_id))
            .filter(reading_sessions::Column::BookId.eq(measured.book_id))
            .filter(reading_sessions::Column::DeviceId.eq(measured.device_id.clone()))
            .filter(reading_sessions::Column::Pass.eq(measured.pass))
            .filter(reading_sessions::Column::Id.ne(measured.id))
            .filter(reading_sessions::Column::DurationSource.eq(DurationSource::Unknown.as_str()))
            .filter(reading_sessions::Column::ActiveDurationMs.is_null())
            .filter(reading_sessions::Column::ClientEndedAt.gte(measured.client_started_at))
            .filter(reading_sessions::Column::ClientEndedAt.lte(measured.client_ended_at))
            .all(db)
            .await?;

        if covered.is_empty() {
            return Ok(0);
        }

        // Carry forward the furthest position any of them reached before
        // dropping them. The measured session should normally already be at or
        // past it, but a page turn landing between the last position write and
        // the session close would otherwise be lost, and losing a reader's
        // place is much worse than keeping a redundant row.
        let mut active: reading_sessions::ActiveModel = measured.clone().into();
        let mut changed = false;

        if let Some(furthest) = covered.iter().filter_map(|s| s.to_page).max()
            && measured.to_page.is_none_or(|current| furthest > current)
        {
            active.to_page = Set(Some(furthest));
            changed = true;
        }
        if let Some(furthest) = covered
            .iter()
            .filter_map(|s| s.to_percentage)
            .fold(None, |acc: Option<f64>, p| {
                Some(acc.map_or(p, |best: f64| best.max(p)))
            })
            && measured
                .to_percentage
                .is_none_or(|current| furthest > current)
        {
            active.to_percentage = Set(Some(furthest));
            changed = true;
        }
        // The position-only writes are the only carrier of an EPUB locator
        // during a sitting, so a measured session that has none inherits the
        // most recent one rather than dropping it.
        if measured.r2_progression.is_none()
            && let Some(progression) = covered.iter().rev().find_map(|s| s.r2_progression.clone())
        {
            active.r2_progression = Set(Some(progression));
            changed = true;
        }

        if changed {
            active.update(db).await?;
        }

        let ids: Vec<Uuid> = covered.iter().map(|s| s.id).collect();
        let deleted = ReadingSessions::delete_many()
            .filter(reading_sessions::Column::Id.is_in(ids))
            .exec(db)
            .await?;

        Ok(deleted.rows_affected)
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
        if !session.coalesce || session.kind != SessionKind::Progress {
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

        // Reconstruct reading time from the gap since the previous write, for
        // clients that cannot report it themselves.
        //
        // The gap is bounded by the coalescing window, because a candidate is
        // only found inside it, so a reader who wanders off simply starts a new
        // session rather than accruing the time they were away. It undercounts:
        // the last page before someone stops has no successor to be measured
        // against, and a client that prefetches a whole book and reads offline
        // is invisible here. Undercounting is the honest direction to be wrong.
        let (contributed, source) = if session.infer_duration {
            let gap = (session.client_ended_at - existing.client_ended_at)
                .num_milliseconds()
                .max(0);
            (Some(gap), DurationSource::Inferred)
        } else {
            (session.active_duration_ms, session.duration_source)
        };

        active.active_duration_ms = Set(sum_options(existing.active_duration_ms, contributed));
        active.pages_read = Set(sum_options(existing.pages_read, session.pages_read));

        // A merged row is only as trustworthy as its weakest contributor: if
        // either side's duration was reconstructed rather than measured, the
        // total is reconstructed.
        let merged_source = match (existing.duration_source(), source) {
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
    use chrono::TimeZone;

    fn at(minutes: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 14, 10, 0, 0).unwrap() + Duration::minutes(minutes)
    }

    fn client_session(duration_ms: Option<i64>, from: i64, to: i64) -> NewSession {
        NewSession::from_client(
            Uuid::new_v4(),
            Uuid::nil(),
            Uuid::nil(),
            "device-a",
            None,
            SessionKind::Progress,
            duration_ms,
            None,
            at(from),
            at(to),
        )
    }

    #[test]
    fn summing_optional_counters_preserves_absence() {
        assert_eq!(sum_options(Some(3), Some(4)), Some(7));
        assert_eq!(sum_options(Some(3), None), Some(3));
        assert_eq!(sum_options(None, Some(4)), Some(4));
        assert_eq!(sum_options::<i64>(None, None), None);
    }

    /// A duration inside the session's own span is taken as reported.
    #[test]
    fn a_plausible_duration_is_kept() {
        let ten_minutes = 10 * 60 * 1000;
        let session = client_session(Some(ten_minutes), 0, 20);

        assert_eq!(session.active_duration_ms, Some(ten_minutes));
        assert_eq!(session.duration_source, DurationSource::Measured);
    }

    /// A client cannot have read for longer than the session lasted. Claiming
    /// otherwise is a bug, and the value is truncated rather than trusted into
    /// the reading statistics.
    #[test]
    fn a_duration_longer_than_the_span_is_clamped_to_it() {
        let ten_hours = 10 * 60 * 60 * 1000;
        let session = client_session(Some(ten_hours), 0, 5);

        assert_eq!(session.active_duration_ms, Some(5 * 60 * 1000));
    }

    /// A backwards span cannot yield negative reading time. The handler
    /// rejects these before they reach here; this is the second line.
    #[test]
    fn a_backwards_span_clamps_to_zero() {
        let session = client_session(Some(60_000), 20, 0);

        assert_eq!(session.active_duration_ms, Some(0));
    }

    /// Reporting no duration leaves provenance unknown rather than claiming a
    /// measurement that was never taken.
    #[test]
    fn an_absent_duration_is_not_recorded_as_measured() {
        let session = client_session(None, 0, 20);

        assert_eq!(session.active_duration_ms, None);
        assert_eq!(session.duration_source, DurationSource::Unknown);
    }

    /// Client sessions never merge. A merge discards the incoming id, and an
    /// id absent from the log would make the next replay look new and add its
    /// duration a second time.
    #[test]
    fn client_sessions_are_not_coalescable() {
        assert!(!client_session(None, 0, 20).coalesce);
    }

    /// Legacy writes do merge: they arrive one per page turn and their ids are
    /// generated here, so nothing ever refers to them again.
    #[test]
    fn legacy_writes_are_coalescable() {
        let session = NewSession::from_legacy_write(
            Uuid::nil(),
            Uuid::nil(),
            &DeviceContext::legacy(),
            SessionKind::Progress,
            at(0),
        );

        assert!(session.coalesce);
    }

    /// The compat surfaces opt in to reconstruction; everything else does not.
    #[test]
    fn only_compat_surfaces_opt_into_inference() {
        assert!(DeviceContext::api_key(Uuid::nil(), "Komic").infer_duration);
        assert!(DeviceContext::user_agent("KOReader/2024").infer_duration);
        assert!(DeviceContext::koreader("kobo-1", "Kobo").infer_duration);

        // The web reader posts measured sessions for the same reading, so
        // inferring from its progress writes too would count it twice.
        assert!(!DeviceContext::legacy().infer_duration);
        assert!(!DeviceContext::internal().infer_duration);
    }

    /// One device keeps one identity across restarts, and different clients
    /// stay distinguishable.
    #[test]
    fn user_agent_identity_is_stable_and_distinct() {
        assert_eq!(
            DeviceContext::user_agent("Komic/1.2 iOS").id,
            DeviceContext::user_agent("Komic/1.2 iOS").id
        );
        assert_ne!(
            DeviceContext::user_agent("Komic/1.2 iOS").id,
            DeviceContext::user_agent("Chunky/3.0 iPad").id
        );
    }

    #[test]
    fn known_clients_get_a_readable_name() {
        assert_eq!(
            DeviceContext::user_agent("Komic/1.2 (iOS 18)")
                .name
                .as_deref(),
            Some("Komic")
        );
        assert_eq!(
            DeviceContext::user_agent("KOReader/2024.04")
                .name
                .as_deref(),
            Some("KOReader")
        );
    }

    /// An unrecognised agent keeps its string rather than becoming "Unknown",
    /// but bounded so a pathological header cannot fill the column.
    #[test]
    fn an_unknown_agent_keeps_a_truncated_label() {
        let long = "x".repeat(500);
        let name = DeviceContext::user_agent(&long).name.unwrap();

        assert_eq!(name.len(), 60);
    }

    /// KOReader is the one compat protocol that sends its own device identity.
    #[test]
    fn koreader_uses_the_identity_it_sends() {
        let device = DeviceContext::koreader("abc123", "Kobo Clara");

        assert_eq!(device.id, "koreader:abc123");
        assert_eq!(device.name.as_deref(), Some("Kobo Clara"));
    }

    #[test]
    fn koreader_falls_back_to_a_generic_name() {
        assert_eq!(
            DeviceContext::koreader("abc123", "").name.as_deref(),
            Some("KOReader")
        );
    }
}
