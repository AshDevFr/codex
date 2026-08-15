//! The fold: an ordered slice of reading sessions in, the projections that
//! `read_progress` and `read_completions` should hold out.
//!
//! Deliberately free of database access. Every rule that decides what a reader's
//! state *is* lives here as a pure function over a slice, so it can be tested
//! exhaustively against hand-written event sequences without a database, and so
//! the repository layer is left with nothing but loading and persisting.
//!
//! # Ordering
//!
//! Events sort by `(pass, client_ended_at, server_recorded_at)`. The primary key
//! is when the reading *happened*, not when the server heard about it, and that
//! choice is the entire point of the table:
//!
//! ```text
//! iPad    read to page 12, finished 09:30, synced 10:05
//! iPhone  read to page 40, finished 10:00, synced 10:00
//! ```
//!
//! Ordered by arrival the iPad lands last and drags the reader back to page 12.
//! Ordered by when the reading happened, the iPhone's session is later and wins
//! whichever order the two arrive in. `server_recorded_at` breaks ties only, so
//! a device with a skewed clock cannot win ordering outright.
//!
//! There is deliberately **no** "furthest position wins" rule. A reader tapping
//! back from page 50 to page 49 produces a later session with a lower position,
//! and a `max` would silently discard it. Taking the last value in client-time
//! order handles the cross-device conflict and the deliberate rewind with one
//! rule, and removes any need for clients to signal rewind intent.

use crate::entities::reading_sessions::{Model as Session, SessionKind};
use chrono::{DateTime, Utc};

/// What `read_progress` should hold for one user and book.
#[derive(Clone, Debug, PartialEq)]
pub struct FoldedProgress {
    pub pass: i32,
    pub current_page: i32,
    pub progress_percentage: Option<f64>,
    pub completed: bool,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub r2_progression: Option<String>,
}

/// A completed pass that may need banking in `read_completions`.
///
/// The fold reports what the current pass warrants; the caller is responsible
/// for the duplicate guard, since answering "has this pass already been banked"
/// requires reading the log.
#[derive(Clone, Debug, PartialEq)]
pub struct FoldedCompletion {
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

/// The result of folding one `(user, book)` slice.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Fold {
    /// `None` means the `read_progress` row must **not exist**.
    ///
    /// Marking a book unread deletes that row today, and callers assert on its
    /// absence rather than on a zeroed row, so a pass consisting only of a
    /// reset has to project to nothing at all.
    pub progress: Option<FoldedProgress>,
    /// Set when the current pass has finished. Earlier passes are not
    /// re-reported: their completions were banked while they were current.
    pub completion: Option<FoldedCompletion>,
}

/// Sort key for the fold's ordering. Exposed so the repository can apply the
/// same ordering in SQL and so tests can assert the two agree.
pub fn sort_key(session: &Session) -> (i32, DateTime<Utc>, DateTime<Utc>) {
    (
        session.pass,
        session.client_ended_at,
        session.server_recorded_at,
    )
}

/// Fold a slice of sessions for one `(user, book)` into its projections.
///
/// The slice may arrive in any order and may span any number of passes; only
/// the highest pass contributes to the projection, since earlier passes are
/// history.
pub fn fold(sessions: &[Session]) -> Fold {
    let Some(current_pass) = sessions.iter().map(|s| s.pass).max() else {
        return Fold::default();
    };

    let mut current: Vec<&Session> = sessions.iter().filter(|s| s.pass == current_pass).collect();
    current.sort_by_key(|s| sort_key(s));

    // A pass whose only events are resets is a book marked unread and not read
    // since. That must project to no row, not to a zeroed one.
    let mut reading = current
        .iter()
        .filter(|s| s.session_kind() != SessionKind::Reset)
        .peekable();
    if reading.peek().is_none() {
        return Fold::default();
    }
    let reading: Vec<&&Session> = reading.collect();

    // The pass began when its first reading happened. This delimits the pass
    // for the completion guard, and it has to survive a back-tap (which is just
    // another event in the same pass) while not surviving a reset (which starts
    // a new pass entirely).
    let started_at = reading
        .iter()
        .map(|s| s.client_started_at)
        .min()
        .expect("non-empty by the peek above");
    let updated_at = current
        .iter()
        .map(|s| s.server_recorded_at)
        .max()
        .expect("non-empty because reading is non-empty");

    let mut current_page = 0;
    let mut progress_percentage = None;
    let mut r2_progression: Option<String> = None;
    let mut completed = false;
    let mut completed_at: Option<DateTime<Utc>> = None;

    for session in &reading {
        // Position takes the event's value outright. See the module docs for
        // why there is no max here.
        if let Some(page) = session.to_page {
            current_page = page;
        }
        if session.to_percentage.is_some() {
            progress_percentage = session.to_percentage;
        }
        // A session that carries no progression leaves the stored one alone,
        // matching the existing contract where `None` means "unchanged" rather
        // than "clear it".
        if session.r2_progression.is_some() {
            r2_progression = session.r2_progression.clone();
        }

        // `completed_at` tracks the *transition*, not the state: re-asserting a
        // completion keeps the original timestamp, and reading on past the end
        // clears it. Both match how the mutable row behaved.
        match session.session_kind() {
            SessionKind::Completed => {
                completed = true;
                if completed_at.is_none() {
                    completed_at = Some(session.client_ended_at);
                }
            }
            SessionKind::Progress => {
                completed = false;
                completed_at = None;
            }
            SessionKind::Reset => unreachable!("resets are filtered out above"),
        }
    }

    let completion = completed_at.map(|at| FoldedCompletion {
        started_at,
        completed_at: at,
    });

    Fold {
        progress: Some(FoldedProgress {
            pass: current_pass,
            current_page,
            progress_percentage,
            completed,
            started_at,
            updated_at,
            completed_at,
            r2_progression,
        }),
        completion,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::reading_sessions::DurationSource;
    use chrono::TimeZone;
    use uuid::Uuid;

    /// `minutes` after a fixed base instant. An offset rather than a
    /// wall-clock minute so tests can space events more than an hour apart.
    fn at(minutes: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 14, 10, 0, 0).unwrap() + chrono::Duration::minutes(minutes)
    }

    /// A session builder that keeps the tests readable: every test cares about
    /// two or three fields and should not have to spell out the other twelve.
    struct S {
        pass: i32,
        kind: SessionKind,
        page: Option<i32>,
        percentage: Option<f64>,
        r2: Option<String>,
        device: &'static str,
        ended: DateTime<Utc>,
        started: DateTime<Utc>,
        recorded: DateTime<Utc>,
    }

    impl S {
        fn new(kind: SessionKind, ended: DateTime<Utc>) -> Self {
            Self {
                pass: 1,
                kind,
                page: None,
                percentage: None,
                r2: None,
                device: "device-a",
                ended,
                started: ended,
                recorded: ended,
            }
        }

        fn progress(page: i32, ended: DateTime<Utc>) -> Self {
            Self {
                page: Some(page),
                ..Self::new(SessionKind::Progress, ended)
            }
        }

        fn completed(page: i32, ended: DateTime<Utc>) -> Self {
            Self {
                page: Some(page),
                ..Self::new(SessionKind::Completed, ended)
            }
        }

        fn reset(ended: DateTime<Utc>) -> Self {
            Self::new(SessionKind::Reset, ended)
        }

        fn pass(mut self, pass: i32) -> Self {
            self.pass = pass;
            self
        }

        fn device(mut self, device: &'static str) -> Self {
            self.device = device;
            self
        }

        fn started(mut self, started: DateTime<Utc>) -> Self {
            self.started = started;
            self
        }

        /// Arrival time, which for a late-syncing offline client is much later
        /// than when the reading happened.
        fn recorded(mut self, recorded: DateTime<Utc>) -> Self {
            self.recorded = recorded;
            self
        }

        fn percentage(mut self, percentage: f64) -> Self {
            self.percentage = Some(percentage);
            self
        }

        fn r2(mut self, r2: &str) -> Self {
            self.r2 = Some(r2.to_string());
            self
        }

        fn build(self) -> Session {
            Session {
                id: Uuid::new_v4(),
                user_id: Uuid::nil(),
                book_id: Uuid::nil(),
                device_id: self.device.to_string(),
                device_name: None,
                pass: self.pass,
                kind: self.kind.as_str().to_string(),
                to_page: self.page,
                to_percentage: self.percentage,
                r2_progression: self.r2,
                active_duration_ms: None,
                duration_source: DurationSource::Unknown.as_str().to_string(),
                pages_read: None,
                client_started_at: self.started,
                client_ended_at: self.ended,
                server_recorded_at: self.recorded,
            }
        }
    }

    fn fold_of(sessions: Vec<S>) -> Fold {
        let models: Vec<Session> = sessions.into_iter().map(S::build).collect();
        fold(&models)
    }

    fn progress_of(sessions: Vec<S>) -> FoldedProgress {
        fold_of(sessions).progress.expect("expected a progress row")
    }

    // ------------------------------------------------------------------
    // Empty and trivial slices
    // ------------------------------------------------------------------

    #[test]
    fn empty_slice_projects_nothing() {
        assert_eq!(fold(&[]), Fold::default());
    }

    #[test]
    fn a_single_progress_session_projects_its_position() {
        let progress = progress_of(vec![S::progress(10, at(0))]);

        assert_eq!(progress.current_page, 10);
        assert!(!progress.completed);
        assert_eq!(progress.completed_at, None);
        assert_eq!(progress.pass, 1);
    }

    // ------------------------------------------------------------------
    // Ordering: the defect this table exists to fix
    // ------------------------------------------------------------------

    /// The motivating failure. A device that read less but syncs later must not
    /// drag the reader backwards.
    #[test]
    fn a_late_arriving_stale_session_does_not_clobber_a_fresher_one() {
        let ipad = S::progress(12, at(30)).device("ipad").recorded(at(65));
        let iphone = S::progress(40, at(60)).device("iphone").recorded(at(60));

        assert_eq!(
            progress_of(vec![ipad, iphone]).current_page,
            40,
            "the session that read furthest in client time must win"
        );
    }

    /// And the same slice in the other arrival order folds identically, which is
    /// what makes replay order irrelevant.
    #[test]
    fn fold_is_independent_of_slice_order() {
        let sessions = || {
            vec![
                S::progress(12, at(30)).device("ipad").recorded(at(65)),
                S::progress(40, at(60)).device("iphone").recorded(at(60)),
            ]
        };
        let forward = fold_of(sessions());

        let mut reversed: Vec<Session> = sessions().into_iter().map(S::build).collect();
        reversed.reverse();

        assert_eq!(forward.progress.unwrap().current_page, 40);
        assert_eq!(fold(&reversed).progress.unwrap().current_page, 40);
    }

    /// Simultaneous client times fall back to arrival order rather than being
    /// resolved arbitrarily.
    #[test]
    fn identical_client_times_break_ties_on_arrival() {
        let first = S::progress(10, at(30)).recorded(at(40));
        let second = S::progress(20, at(30)).recorded(at(50));

        assert_eq!(progress_of(vec![second, first]).current_page, 20);
    }

    /// A deliberate rewind is a later session with a lower position. This is the
    /// case a "furthest wins" rule gets wrong.
    #[test]
    fn a_deliberate_rewind_moves_the_position_back() {
        let progress = progress_of(vec![S::completed(50, at(10)), S::progress(49, at(20))]);

        assert_eq!(
            progress.current_page, 49,
            "tapping back must not be discarded as a stale write"
        );
        assert!(!progress.completed);
    }

    // ------------------------------------------------------------------
    // Completion transitions
    // ------------------------------------------------------------------

    #[test]
    fn completing_sets_the_flag_and_the_timestamp() {
        let progress = progress_of(vec![S::progress(10, at(0)), S::completed(50, at(10))]);

        assert!(progress.completed);
        assert_eq!(progress.completed_at, Some(at(10)));
    }

    /// Re-asserting a completion keeps the first timestamp. `completed_at`
    /// records the transition, not the most recent claim.
    #[test]
    fn re_asserting_completion_keeps_the_original_timestamp() {
        let progress = progress_of(vec![
            S::completed(50, at(10)),
            S::completed(50, at(20)),
            S::completed(50, at(30)),
        ]);

        assert_eq!(progress.completed_at, Some(at(10)));
    }

    /// Reading on past a completion clears it, matching the mutable row's
    /// behaviour where `completed_at` is set iff `completed`.
    #[test]
    fn reading_after_completion_clears_the_completion() {
        let progress = progress_of(vec![S::completed(50, at(10)), S::progress(10, at(20))]);

        assert!(!progress.completed);
        assert_eq!(progress.completed_at, None);
    }

    /// The back-tap bounce: one read-through, however many times the client
    /// wobbles across the last page.
    #[test]
    fn a_bounce_across_the_last_page_reports_one_completion() {
        let progress = progress_of(vec![
            S::completed(50, at(10)),
            S::progress(49, at(20)),
            S::completed(50, at(30)),
        ]);

        assert!(progress.completed);
        assert_eq!(
            progress.completed_at,
            Some(at(30)),
            "the bounce cleared the first completion, so the second is the transition"
        );
    }

    // ------------------------------------------------------------------
    // Passes and resets
    // ------------------------------------------------------------------

    /// A reset with no reading since must project to no row at all. Callers
    /// assert `is_none()` on the progress row after marking a book unread.
    #[test]
    fn a_reset_alone_projects_no_progress_row() {
        let folded = fold_of(vec![
            S::completed(50, at(10)).pass(1),
            S::reset(at(20)).pass(2),
        ]);

        assert_eq!(folded.progress, None);
        assert_eq!(folded.completion, None);
    }

    #[test]
    fn consecutive_resets_still_project_nothing() {
        let folded = fold_of(vec![
            S::progress(10, at(0)).pass(1),
            S::reset(at(10)).pass(2),
            S::reset(at(20)).pass(3),
        ]);

        assert_eq!(folded.progress, None);
    }

    /// Reading after a reset starts fresh: the new pass's position, and a
    /// `started_at` that no longer covers the previous pass's completion.
    #[test]
    fn reading_after_a_reset_starts_a_new_pass() {
        let folded = fold_of(vec![
            S::completed(50, at(10)).pass(1),
            S::reset(at(20)).pass(2),
            S::progress(5, at(30)).pass(2),
        ]);

        let progress = folded.progress.expect("expected a progress row");
        assert_eq!(progress.pass, 2);
        assert_eq!(progress.current_page, 5);
        assert!(!progress.completed);
        assert_eq!(progress.started_at, at(30));
        assert_eq!(
            folded.completion, None,
            "the new pass has not finished, so nothing is banked"
        );
    }

    /// Completing a second pass reports a completion spanning that pass only.
    #[test]
    fn completing_a_second_pass_reports_its_own_span() {
        let folded = fold_of(vec![
            S::completed(50, at(0)).pass(1),
            S::reset(at(10)).pass(2),
            S::progress(5, at(20)).pass(2),
            S::completed(50, at(30)).pass(2),
        ]);

        assert_eq!(
            folded.completion,
            Some(FoldedCompletion {
                started_at: at(20),
                completed_at: at(30),
            })
        );
    }

    /// Only the highest pass contributes. Earlier passes were banked while they
    /// were current and must not be re-reported on every refold.
    #[test]
    fn earlier_passes_do_not_contribute_to_the_projection() {
        let folded = fold_of(vec![
            S::completed(99, at(0)).pass(1),
            S::reset(at(10)).pass(2),
            S::progress(3, at(20)).pass(2),
        ]);

        let progress = folded.progress.unwrap();
        assert_eq!(progress.current_page, 3);
        assert_eq!(folded.completion, None);
    }

    // ------------------------------------------------------------------
    // Pass span, used by the completion guard
    // ------------------------------------------------------------------

    /// `started_at` is when the pass's first reading happened, not when the
    /// last event arrived. The completion guard keys on it.
    #[test]
    fn started_at_is_the_first_reading_of_the_pass() {
        let progress = progress_of(vec![
            S::progress(1, at(10)).started(at(5)),
            S::progress(20, at(30)).started(at(25)),
            S::completed(50, at(50)).started(at(45)),
        ]);

        assert_eq!(progress.started_at, at(5));
    }

    /// A reset does not contribute its timestamp to the new pass's span; only
    /// reading does.
    #[test]
    fn a_reset_does_not_set_the_new_passes_start() {
        let progress = progress_of(vec![
            S::reset(at(10)).pass(2),
            S::progress(5, at(30)).pass(2).started(at(25)),
        ]);

        assert_eq!(progress.started_at, at(25));
    }

    #[test]
    fn updated_at_is_the_latest_arrival() {
        let progress = progress_of(vec![
            S::progress(10, at(10)).recorded(at(15)),
            S::progress(20, at(20)).recorded(at(90)),
        ]);

        assert_eq!(progress.updated_at, at(90));
    }

    // ------------------------------------------------------------------
    // EPUB percentage and Readium progression
    // ------------------------------------------------------------------

    #[test]
    fn percentage_follows_the_last_session_that_carries_one() {
        let progress = progress_of(vec![
            S::progress(0, at(10)).percentage(0.25),
            S::progress(0, at(20)).percentage(0.60),
        ]);

        assert_eq!(progress.progress_percentage, Some(0.60));
    }

    /// A session with no progression leaves the stored one alone. `None` has
    /// always meant "unchanged" on this field, not "clear it".
    #[test]
    fn a_session_without_progression_leaves_the_stored_one_intact() {
        let progress = progress_of(vec![
            S::progress(0, at(10)).r2(r#"{"locator":"a"}"#),
            S::progress(5, at(20)),
        ]);

        assert_eq!(
            progress.r2_progression.as_deref(),
            Some(r#"{"locator":"a"}"#)
        );
    }

    #[test]
    fn a_later_progression_replaces_an_earlier_one() {
        let progress = progress_of(vec![
            S::progress(0, at(10)).r2(r#"{"locator":"a"}"#),
            S::progress(0, at(20)).r2(r#"{"locator":"b"}"#),
        ]);

        assert_eq!(
            progress.r2_progression.as_deref(),
            Some(r#"{"locator":"b"}"#)
        );
    }

    /// Mixed-format writes: an EPUB client sending only a percentage must not
    /// wipe a page position written by another surface, and vice versa.
    #[test]
    fn page_and_percentage_are_tracked_independently() {
        let progress = progress_of(vec![
            S::progress(12, at(10)),
            S::progress(0, at(20)).percentage(0.5),
        ]);

        assert_eq!(progress.progress_percentage, Some(0.5));
        assert_eq!(
            progress.current_page, 0,
            "the later session carried an explicit page, so it wins"
        );
    }

    // ------------------------------------------------------------------
    // Multi-device
    // ------------------------------------------------------------------

    #[test]
    fn sessions_from_several_devices_fold_into_one_position() {
        let progress = progress_of(vec![
            S::progress(10, at(10)).device("phone"),
            S::progress(25, at(20)).device("ipad"),
            S::progress(30, at(30)).device("phone"),
        ]);

        assert_eq!(progress.current_page, 30);
    }

    /// A device completing while another device is mid-book: the later reading
    /// wins, and the completion is cleared, because that is what the reader
    /// actually did last.
    #[test]
    fn a_later_read_on_another_device_clears_an_earlier_completion() {
        let folded = fold_of(vec![
            S::completed(50, at(10)).device("phone"),
            S::progress(20, at(30)).device("ipad"),
        ]);

        let progress = folded.progress.unwrap();
        assert!(!progress.completed);
        assert_eq!(progress.current_page, 20);
        assert_eq!(folded.completion, None);
    }

    // ------------------------------------------------------------------
    // Unknown kinds
    // ------------------------------------------------------------------

    /// A row written by a newer server degrades to "some reading happened"
    /// rather than vanishing from the fold.
    #[test]
    fn an_unrecognised_kind_is_treated_as_progress() {
        let mut session = S::progress(10, at(10)).build();
        session.kind = "some-future-kind".to_string();

        let folded = fold(&[session]);
        let progress = folded.progress.expect("expected a progress row");
        assert_eq!(progress.current_page, 10);
        assert!(!progress.completed);
    }
}
