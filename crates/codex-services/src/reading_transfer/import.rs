//! Applying a matched export document under an explicit conflict policy.
//!
//! One transaction per series: a partial failure part-way through a large
//! import must not leave some of a series' books updated and others not, but
//! it also must not cost every other series in the file its own progress just
//! because one series' data was bad. The response reports which series
//! actually committed.
//!
//! A dry run walks the exact same decision logic as a real import (matching,
//! conflict resolution, orphan reattachment) and simply never opens a
//! transaction to apply it, which is what guarantees the response shape is
//! identical either way and that nothing is written.

use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set, TransactionTrait,
};
use std::collections::{HashMap, HashSet};
use std::fmt;
use uuid::Uuid;

use codex_db::entities::{
    books, read_completions, read_progress, reading_sessions, user_series_ratings,
};
use codex_db::repositories::{
    BookRepository, LibraryRepository, SeriesRepository, UserSeriesRatingRepository,
};

use crate::content_filter::ContentFilter;

use super::matching::{self, BookCandidate, BookMatch, SeriesMatch};
use super::model::{
    BookDisposition, ConflictPolicy, ExportBookDto, ExportCompletionDto, ExportSeriesDto,
    ExportSessionDto, FieldOutcome, HashMode, ImportBookReport, ImportReadingProgressResponse,
    ImportSeriesReport, ImportSummary, READING_PROGRESS_FORMAT, READING_PROGRESS_VERSION,
    ReadingProgressExportDocument, SeriesDisposition, WriteCounts,
};

/// Everything the caller controls about how an import behaves.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub dry_run: bool,
    pub hash_mode: HashMode,
    /// `None` means every source the exported series carries. See
    /// [`ImportReadingProgressRequest::source_preference`].
    pub source_preference: Option<Vec<String>>,
    pub conflict_policy: ConflictPolicy,
    pub reattach_sessions: bool,
    pub accept_stem_matches: bool,
    /// Which libraries a series may match into. `None` searches every library
    /// the reader can see; `Some` narrows the search, which is how an import
    /// can run while the old copy of a series is still present.
    pub library_ids: Option<Vec<Uuid>>,
}

/// A rejection worth a 400, versus every other failure which is a 500.
#[derive(Debug)]
pub enum ImportError {
    UnknownFormat(String),
    UnsupportedVersion(i32),
    /// A value the normal write paths would never accept. Named precisely so
    /// the user can find it in the file.
    InvalidValue(String),
    Database(anyhow::Error),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::UnknownFormat(format) => write!(
                f,
                "unknown export format '{format}', expected '{READING_PROGRESS_FORMAT}'"
            ),
            ImportError::UnsupportedVersion(version) => write!(
                f,
                "export version {version} is newer than this server supports (max {READING_PROGRESS_VERSION})"
            ),
            ImportError::InvalidValue(message) => write!(f, "{message}"),
            ImportError::Database(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ImportError {}

fn validate_document(document: &ReadingProgressExportDocument) -> Result<(), ImportError> {
    if document.format != READING_PROGRESS_FORMAT {
        return Err(ImportError::UnknownFormat(document.format.clone()));
    }
    if document.version > READING_PROGRESS_VERSION {
        return Err(ImportError::UnsupportedVersion(document.version));
    }
    // Ratings feed a per-series average every user sees, so a file must not
    // be a way around the range the rating endpoint enforces: an out-of-range
    // value would skew that average for everyone, and a large one overflows
    // the sum behind it.
    for series in &document.series {
        if let Some(rating) = series.rating
            && !(1..=100).contains(&rating)
        {
            return Err(ImportError::InvalidValue(format!(
                "series '{}' has rating {rating}; ratings must be between 1 and 100",
                series.name
            )));
        }
    }
    Ok(())
}

/// A session's reading time can never exceed the span it covers. The normal
/// write path clamps to that span for the same reason: a larger figure is a
/// bug or a fabrication, and one huge value is enough to overflow the
/// statistics sums for the reader.
fn clamped_duration(doc: &ExportSessionDto) -> Option<i64> {
    let span = (doc.client_ended_at - doc.client_started_at).num_milliseconds();
    doc.active_duration_ms
        .map(|reported| reported.clamp(0, span.max(0)))
}

// ---------------------------------------------------------------------------
// Decisions: pure data describing what would happen, computed once and used
// both to build the report and (for a real import) to drive the writes.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ProgressValues {
    current_page: i32,
    progress_percentage: Option<f64>,
    completed: bool,
    started_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    r2_progression: Option<String>,
}

#[derive(Debug, Clone)]
enum ProgressDecision {
    /// The id is minted when planning, so a later entry for the same book can
    /// be planned as an update of this row before it exists.
    Insert(Uuid, ProgressValues),
    Update(Uuid, ProgressValues),
    Skip,
}

impl ProgressDecision {
    fn outcome(&self) -> FieldOutcome {
        match self {
            ProgressDecision::Insert(_, _) => FieldOutcome::Inserted,
            ProgressDecision::Update(_, _) => FieldOutcome::Updated,
            ProgressDecision::Skip => FieldOutcome::Skipped,
        }
    }
}

/// How far into a book a position is, for the `furthest` conflict policy. A
/// completed read always wins: it is further than any partial position could
/// be, regardless of which position field the format happens to use.
fn progress_score(completed: bool, percentage: Option<f64>, page: i32) -> f64 {
    if completed {
        f64::INFINITY
    } else if let Some(p) = percentage {
        p
    } else {
        page as f64
    }
}

fn decide_progress(
    existing: Option<&read_progress::Model>,
    imported: &super::model::ExportProgressDto,
    policy: ConflictPolicy,
) -> ProgressDecision {
    let values = ProgressValues {
        current_page: imported.current_page,
        progress_percentage: imported.progress_percentage,
        completed: imported.completed,
        started_at: imported.started_at,
        updated_at: imported.updated_at,
        completed_at: imported.completed_at,
        r2_progression: imported.r2_progression.clone(),
    };

    let Some(existing) = existing else {
        return ProgressDecision::Insert(Uuid::new_v4(), values);
    };

    match policy {
        ConflictPolicy::SkipExisting => ProgressDecision::Skip,
        ConflictPolicy::Overwrite => ProgressDecision::Update(existing.id, values),
        ConflictPolicy::Newest => {
            if imported.updated_at > existing.updated_at {
                ProgressDecision::Update(existing.id, values)
            } else {
                ProgressDecision::Skip
            }
        }
        ConflictPolicy::Furthest => {
            let existing_score = progress_score(
                existing.completed,
                existing.progress_percentage,
                existing.current_page,
            );
            let imported_score = progress_score(
                imported.completed,
                imported.progress_percentage,
                imported.current_page,
            );
            if imported_score > existing_score {
                ProgressDecision::Update(existing.id, values)
            } else {
                ProgressDecision::Skip
            }
        }
    }
}

#[derive(Debug, Clone)]
enum RatingDecision {
    NoOp,
    Insert {
        id: Uuid,
        rating: i32,
        notes: Option<String>,
        updated_at: DateTime<Utc>,
    },
    Update {
        existing: user_series_ratings::Model,
        rating: i32,
        notes: Option<String>,
        updated_at: DateTime<Utc>,
    },
    Skip,
}

impl RatingDecision {
    fn outcome(&self) -> Option<FieldOutcome> {
        match self {
            RatingDecision::NoOp => None,
            RatingDecision::Insert { .. } => Some(FieldOutcome::Inserted),
            RatingDecision::Update { .. } => Some(FieldOutcome::Updated),
            RatingDecision::Skip => Some(FieldOutcome::Skipped),
        }
    }
}

/// Decide a series rating under the conflict policy. `newest` compares the
/// file's `rating_updated_at` with the existing row's `updated_at`; see the
/// comments inside for `furthest` and for a file without the timestamp.
fn decide_rating(
    existing: Option<&user_series_ratings::Model>,
    series_doc: &ExportSeriesDto,
    policy: ConflictPolicy,
) -> RatingDecision {
    let Some(rating) = series_doc.rating else {
        return RatingDecision::NoOp;
    };
    let notes = series_doc.notes.clone();
    // The rating keeps the time it was actually set, so a second import of the
    // same file compares equal and is skipped rather than rewritten.
    let updated_at = series_doc.rating_updated_at.unwrap_or_else(Utc::now);

    match existing {
        None => RatingDecision::Insert {
            id: Uuid::new_v4(),
            rating,
            notes,
            updated_at,
        },
        Some(existing) => {
            let replace = match policy {
                ConflictPolicy::SkipExisting => false,
                ConflictPolicy::Overwrite => true,
                // A rating has no position to be "further" along, so furthest
                // reads as newest. A file without the timestamp cannot show it
                // is newer, and an old export must not silently undo a rating
                // changed since, so it loses.
                ConflictPolicy::Newest | ConflictPolicy::Furthest => series_doc
                    .rating_updated_at
                    .is_some_and(|imported| imported > existing.updated_at),
            };
            if replace {
                RatingDecision::Update {
                    existing: existing.clone(),
                    rating,
                    notes,
                    updated_at,
                }
            } else {
                RatingDecision::Skip
            }
        }
    }
}

/// What happens to one append-only row (`read_completions` / `reading_sessions`)
/// whose primary key is the id reused from the export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowDecision {
    /// No row with this id exists yet.
    Insert,
    /// The row exists and is the importing user's own, but it is not on a
    /// live book: its book was hard-deleted (`book_id IS NULL`), or it still
    /// points at a book the scanner has marked deleted because the file moved.
    /// Move it onto the matched book.
    Reattach,
    /// Already on a live book (re-importing the same file), listed twice in
    /// the file, or belongs to someone else (a UUID collision that should
    /// never happen, handled by leaving it alone).
    Skip,
    /// On a *different* live book, in a library that still has the file. The
    /// write is the same no-op as [`Self::Skip`], but it means something very
    /// different: the progress moved and the reading history did not. Counted
    /// apart so the report can say so.
    SkipStranded,
}

/// An existing history row, as much as a decision needs.
#[derive(Debug, Clone, Copy)]
struct ExistingRow {
    user_id: Uuid,
    book_id: Option<Uuid>,
}

/// What this import has already planned to write, so each entry is decided
/// against the state the earlier ones will leave rather than against the
/// database as it was before the import started.
///
/// Without it, two entries that land on the same book (the scanner keeps a
/// moved file's old row soft-deleted, and the export carries both) are each
/// planned against an empty table: both plan an insert, the second hits the
/// unique index, and the whole series rolls back. The dry run, deciding the
/// same way, would have promised both.
#[derive(Debug, Clone, Default)]
struct PlannedState {
    /// The progress row each target book will hold, by book id.
    progress: HashMap<Uuid, read_progress::Model>,
    /// The rating each target series will hold, by series id.
    ratings: HashMap<Uuid, user_series_ratings::Model>,
    /// Completion and session ids already claimed by an earlier entry.
    rows: HashSet<Uuid>,
}

/// Bind-parameter-safe batch size for `IN (...)` lookups.
const LOOKUP_BATCH: usize = 1_000;

async fn existing_completions(
    db: &DatabaseConnection,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, ExistingRow>> {
    let mut found = HashMap::new();
    for chunk in ids.chunks(LOOKUP_BATCH) {
        for row in read_completions::Entity::find()
            .filter(read_completions::Column::Id.is_in(chunk.to_vec()))
            .all(db)
            .await?
        {
            found.insert(
                row.id,
                ExistingRow {
                    user_id: row.user_id,
                    book_id: row.book_id,
                },
            );
        }
    }
    Ok(found)
}

async fn existing_sessions(
    db: &DatabaseConnection,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, ExistingRow>> {
    let mut found = HashMap::new();
    for chunk in ids.chunks(LOOKUP_BATCH) {
        for row in reading_sessions::Entity::find()
            .filter(reading_sessions::Column::Id.is_in(chunk.to_vec()))
            .all(db)
            .await?
        {
            found.insert(
                row.id,
                ExistingRow {
                    user_id: row.user_id,
                    book_id: row.book_id,
                },
            );
        }
    }
    Ok(found)
}

/// Which of these books the scanner has marked deleted.
async fn soft_deleted_books(db: &DatabaseConnection, ids: &[Uuid]) -> Result<HashSet<Uuid>> {
    let mut found = HashSet::new();
    for chunk in ids.chunks(LOOKUP_BATCH) {
        for book in books::Entity::find()
            .filter(books::Column::Id.is_in(chunk.to_vec()))
            .filter(books::Column::Deleted.eq(true))
            .all(db)
            .await?
        {
            found.insert(book.id);
        }
    }
    Ok(found)
}

async fn progress_for_books(
    db: &DatabaseConnection,
    user_id: Uuid,
    book_ids: &[Uuid],
) -> Result<HashMap<Uuid, read_progress::Model>> {
    let mut found = HashMap::new();
    for chunk in book_ids.chunks(LOOKUP_BATCH) {
        for row in read_progress::Entity::find()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::BookId.is_in(chunk.to_vec()))
            .all(db)
            .await?
        {
            found.insert(row.book_id, row);
        }
    }
    Ok(found)
}

/// Everything already in the database that a series' decisions depend on,
/// fetched in a handful of batched queries rather than one per row.
struct SeriesLookups {
    progress: HashMap<Uuid, read_progress::Model>,
    completions: HashMap<Uuid, ExistingRow>,
    sessions: HashMap<Uuid, ExistingRow>,
    soft_deleted: HashSet<Uuid>,
}

fn decide_row(
    id: Uuid,
    existing: Option<&ExistingRow>,
    target_book: Uuid,
    user_id: Uuid,
    soft_deleted: &HashSet<Uuid>,
    reattach: bool,
    planned: &mut PlannedState,
) -> RowDecision {
    if !planned.rows.insert(id) {
        return RowDecision::Skip;
    }
    match existing {
        None => RowDecision::Insert,
        Some(row) if row.user_id == user_id => {
            let on_another_live_book = matches!(row.book_id, Some(book) if book != target_book && !soft_deleted.contains(&book));
            let off_a_live_book = match row.book_id {
                None => true,
                Some(book) => book != target_book && soft_deleted.contains(&book),
            };
            if off_a_live_book && reattach {
                RowDecision::Reattach
            } else if on_another_live_book {
                RowDecision::SkipStranded
            } else {
                RowDecision::Skip
            }
        }
        Some(_) => RowDecision::Skip,
    }
}

/// Everything decided for one exported book, before any write happens.
struct BookPlan<'a> {
    book_doc: &'a ExportBookDto,
    disposition: BookDisposition,
    matched_book_id: Option<Uuid>,
    applied: bool,
    progress: Option<ProgressDecision>,
    completions: Vec<(&'a ExportCompletionDto, RowDecision)>,
    sessions: Vec<(&'a ExportSessionDto, RowDecision)>,
}

fn build_book_plan<'a>(
    user_id: Uuid,
    book_doc: &'a ExportBookDto,
    candidates: &[BookCandidate],
    lookups: &SeriesLookups,
    planned: &mut PlannedState,
    options: &ImportOptions,
) -> BookPlan<'a> {
    let book_match = matching::resolve_book(candidates, book_doc, options.hash_mode);

    let (disposition, matched_book_id, applied) = match book_match {
        BookMatch::Matched(id) => (BookDisposition::Matched, Some(id), true),
        BookMatch::StemMatch(id) => (
            BookDisposition::StemMatch,
            Some(id),
            options.accept_stem_matches,
        ),
        BookMatch::Ambiguous => (BookDisposition::Ambiguous, None, false),
        BookMatch::Unmatched => (BookDisposition::Unmatched, None, false),
        BookMatch::HashMismatch => (BookDisposition::HashMismatch, None, false),
    };

    let Some(book_id) = matched_book_id.filter(|_| applied) else {
        return BookPlan {
            book_doc,
            disposition,
            matched_book_id,
            applied: false,
            progress: None,
            completions: vec![],
            sessions: vec![],
        };
    };

    let progress = book_doc.progress.as_ref().map(|imported| {
        let existing = planned
            .progress
            .get(&book_id)
            .or_else(|| lookups.progress.get(&book_id))
            .cloned();
        let decision = decide_progress(existing.as_ref(), imported, options.conflict_policy);
        let planned_row = match &decision {
            ProgressDecision::Insert(id, values) | ProgressDecision::Update(id, values) => {
                Some(progress_model(*id, user_id, book_id, values))
            }
            ProgressDecision::Skip => existing,
        };
        if let Some(row) = planned_row {
            planned.progress.insert(book_id, row);
        }
        decision
    });

    let completions = book_doc
        .completions
        .iter()
        .map(|completion| {
            let decision = decide_row(
                completion.id,
                lookups.completions.get(&completion.id),
                book_id,
                user_id,
                &lookups.soft_deleted,
                options.reattach_sessions,
                planned,
            );
            (completion, decision)
        })
        .collect();

    let sessions = book_doc
        .sessions
        .iter()
        .flatten()
        .map(|session| {
            let decision = decide_row(
                session.id,
                lookups.sessions.get(&session.id),
                book_id,
                user_id,
                &lookups.soft_deleted,
                options.reattach_sessions,
                planned,
            );
            (session, decision)
        })
        .collect();

    BookPlan {
        book_doc,
        disposition,
        matched_book_id,
        applied: true,
        progress,
        completions,
        sessions,
    }
}

fn progress_model(
    id: Uuid,
    user_id: Uuid,
    book_id: Uuid,
    values: &ProgressValues,
) -> read_progress::Model {
    read_progress::Model {
        id,
        user_id,
        book_id,
        current_page: values.current_page,
        progress_percentage: values.progress_percentage,
        completed: values.completed,
        started_at: values.started_at,
        updated_at: values.updated_at,
        completed_at: values.completed_at,
        r2_progression: values.r2_progression.clone(),
    }
}

fn book_report_from_plan(plan: &BookPlan<'_>) -> ImportBookReport {
    let mut completions = WriteCounts::default();
    for (_, decision) in &plan.completions {
        match decision {
            RowDecision::Insert => completions.inserted += 1,
            RowDecision::Reattach => completions.reattached += 1,
            RowDecision::Skip => completions.skipped += 1,
            RowDecision::SkipStranded => {
                completions.skipped += 1;
                completions.stranded += 1;
            }
        }
    }
    let mut sessions = WriteCounts::default();
    for (_, decision) in &plan.sessions {
        match decision {
            RowDecision::Insert => sessions.inserted += 1,
            RowDecision::Reattach => sessions.reattached += 1,
            RowDecision::Skip => sessions.skipped += 1,
            RowDecision::SkipStranded => {
                sessions.skipped += 1;
                sessions.stranded += 1;
            }
        }
    }

    ImportBookReport {
        path: plan.book_doc.path.clone(),
        file_name: plan.book_doc.file_name.clone(),
        disposition: plan.disposition,
        matched_book_id: plan.matched_book_id,
        applied: plan.applied,
        progress: plan.progress.as_ref().map(ProgressDecision::outcome),
        completions,
        sessions,
    }
}

/// Whether the series rating write counts as a real write for the summary,
/// following the same `count_writes` gate as [`tally_book_report`].
fn tally_rating_write(summary: &mut ImportSummary, rating_decision: &RatingDecision) {
    if matches!(
        rating_decision.outcome(),
        Some(FieldOutcome::Inserted) | Some(FieldOutcome::Updated)
    ) {
        summary.ratings_written += 1;
    }
}

/// Fold one book's report into the running summary. `count_writes` is false
/// for a series that was matched but whose transaction failed to commit: the
/// disposition counts (how many books matched, were ambiguous, etc.) are
/// still meaningful, but nothing was actually written.
fn tally_book_report(summary: &mut ImportSummary, report: &ImportBookReport, count_writes: bool) {
    summary.books_total += 1;
    match report.disposition {
        BookDisposition::Matched => summary.books_matched += 1,
        BookDisposition::StemMatch => summary.books_stem_matched += 1,
        BookDisposition::Ambiguous => summary.books_ambiguous += 1,
        BookDisposition::Unmatched => summary.books_unmatched += 1,
        BookDisposition::HashMismatch => summary.books_hash_mismatch += 1,
    }
    if !count_writes {
        return;
    }
    if matches!(
        report.progress,
        Some(FieldOutcome::Inserted) | Some(FieldOutcome::Updated)
    ) {
        summary.progress_written += 1;
    }
    summary.completions_inserted += report.completions.inserted;
    summary.completions_reattached += report.completions.reattached;
    summary.sessions_inserted += report.sessions.inserted;
    summary.sessions_reattached += report.sessions.reattached;
    summary.rows_stranded += report.completions.stranded + report.sessions.stranded;
}

// ---------------------------------------------------------------------------
// Execution: only reached for a real (non-dry-run) import.
// ---------------------------------------------------------------------------

async fn apply_progress(
    txn: &DatabaseTransaction,
    user_id: Uuid,
    book_id: Uuid,
    decision: &ProgressDecision,
) -> Result<()> {
    match decision {
        ProgressDecision::Skip => Ok(()),
        ProgressDecision::Insert(id, values) => {
            read_progress::ActiveModel {
                id: Set(*id),
                user_id: Set(user_id),
                book_id: Set(book_id),
                current_page: Set(values.current_page),
                progress_percentage: Set(values.progress_percentage),
                completed: Set(values.completed),
                started_at: Set(values.started_at),
                updated_at: Set(values.updated_at),
                completed_at: Set(values.completed_at),
                r2_progression: Set(values.r2_progression.clone()),
            }
            .insert(txn)
            .await?;
            Ok(())
        }
        ProgressDecision::Update(id, values) => {
            read_progress::ActiveModel {
                id: Set(*id),
                user_id: Set(user_id),
                book_id: Set(book_id),
                current_page: Set(values.current_page),
                progress_percentage: Set(values.progress_percentage),
                completed: Set(values.completed),
                started_at: Set(values.started_at),
                updated_at: Set(values.updated_at),
                completed_at: Set(values.completed_at),
                r2_progression: Set(values.r2_progression.clone()),
            }
            .update(txn)
            .await?;
            Ok(())
        }
    }
}

async fn apply_completion(
    txn: &DatabaseTransaction,
    user_id: Uuid,
    book_id: Uuid,
    doc: &ExportCompletionDto,
    decision: &RowDecision,
) -> Result<()> {
    match decision {
        RowDecision::Skip | RowDecision::SkipStranded => Ok(()),
        RowDecision::Insert => {
            read_completions::ActiveModel {
                id: Set(doc.id),
                user_id: Set(user_id),
                book_id: Set(Some(book_id)),
                started_at: Set(doc.started_at),
                completed_at: Set(doc.completed_at),
            }
            .insert(txn)
            .await?;
            Ok(())
        }
        RowDecision::Reattach => {
            read_completions::Entity::update_many()
                .col_expr(read_completions::Column::BookId, Expr::value(book_id))
                .filter(read_completions::Column::Id.eq(doc.id))
                .filter(read_completions::Column::UserId.eq(user_id))
                .exec(txn)
                .await?;
            Ok(())
        }
    }
}

async fn apply_session(
    txn: &DatabaseTransaction,
    user_id: Uuid,
    book_id: Uuid,
    doc: &ExportSessionDto,
    decision: &RowDecision,
) -> Result<()> {
    match decision {
        RowDecision::Skip | RowDecision::SkipStranded => Ok(()),
        RowDecision::Insert => {
            reading_sessions::ActiveModel {
                id: Set(doc.id),
                user_id: Set(user_id),
                book_id: Set(Some(book_id)),
                device_id: Set(doc.device_id.clone()),
                device_name: Set(doc.device_name.clone()),
                pass: Set(doc.pass.max(1)),
                kind: Set(doc.kind.clone()),
                to_page: Set(doc.to_page),
                to_percentage: Set(doc.to_percentage),
                // The export never carries a historical locator; only the
                // live `read_progress` row keeps one.
                r2_progression: Set(None),
                active_duration_ms: Set(clamped_duration(doc)),
                duration_source: Set(doc.duration_source.clone()),
                pages_read: Set(doc.pages_read.map(|pages| pages.max(0))),
                client_started_at: Set(doc.client_started_at),
                client_ended_at: Set(doc.client_ended_at),
                server_recorded_at: Set(doc.server_recorded_at),
            }
            .insert(txn)
            .await?;
            Ok(())
        }
        RowDecision::Reattach => {
            reading_sessions::Entity::update_many()
                .col_expr(reading_sessions::Column::BookId, Expr::value(book_id))
                .filter(reading_sessions::Column::Id.eq(doc.id))
                .filter(reading_sessions::Column::UserId.eq(user_id))
                .exec(txn)
                .await?;
            Ok(())
        }
    }
}

async fn apply_rating(
    txn: &DatabaseTransaction,
    user_id: Uuid,
    series_id: Uuid,
    decision: &RatingDecision,
) -> Result<()> {
    match decision {
        RatingDecision::NoOp | RatingDecision::Skip => Ok(()),
        RatingDecision::Insert {
            id,
            rating,
            notes,
            updated_at,
        } => {
            user_series_ratings::ActiveModel {
                id: Set(*id),
                user_id: Set(user_id),
                series_id: Set(series_id),
                rating: Set(*rating),
                notes: Set(notes.clone()),
                created_at: Set(Utc::now()),
                updated_at: Set(*updated_at),
            }
            .insert(txn)
            .await?;
            Ok(())
        }
        RatingDecision::Update {
            existing,
            rating,
            notes,
            updated_at,
        } => {
            let mut active: user_series_ratings::ActiveModel = existing.clone().into();
            active.rating = Set(*rating);
            active.notes = Set(notes.clone());
            active.updated_at = Set(*updated_at);
            active.update(txn).await?;
            Ok(())
        }
    }
}

async fn apply_series(
    db: &DatabaseConnection,
    user_id: Uuid,
    series_id: Uuid,
    plans: &[BookPlan<'_>],
    rating_decision: &RatingDecision,
) -> Result<()> {
    let txn = db.begin().await?;

    for plan in plans {
        if !plan.applied {
            continue;
        }
        let book_id = plan
            .matched_book_id
            .expect("applied implies a matched book id");

        if let Some(progress_decision) = &plan.progress {
            apply_progress(&txn, user_id, book_id, progress_decision).await?;
        }
        for (completion_doc, decision) in &plan.completions {
            apply_completion(&txn, user_id, book_id, completion_doc, decision).await?;
        }
        for (session_doc, decision) in &plan.sessions {
            apply_session(&txn, user_id, book_id, session_doc, decision).await?;
        }
    }

    apply_rating(&txn, user_id, series_id, rating_decision).await?;

    txn.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Series-level orchestration
// ---------------------------------------------------------------------------

async fn plan_series<'a>(
    db: &DatabaseConnection,
    user_id: Uuid,
    series_id: Uuid,
    series_doc: &'a ExportSeriesDto,
    planned: &mut PlannedState,
    options: &ImportOptions,
) -> Result<(Vec<BookPlan<'a>>, RatingDecision)> {
    let series_row = SeriesRepository::get_by_id(db, series_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("matched series {series_id} vanished during import"))?;
    let library_row = LibraryRepository::get_by_id(db, series_row.library_id)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!("library {} vanished during import", series_row.library_id)
        })?;

    let book_models = BookRepository::list_by_series(db, series_id, false).await?;
    let candidates: Vec<BookCandidate> = book_models
        .iter()
        .map(|b| BookCandidate::from_model(b, &library_row.path, &series_row.path))
        .collect();

    let book_ids: Vec<Uuid> = candidates.iter().map(|c| c.id).collect();
    let completion_ids: Vec<Uuid> = series_doc
        .books
        .iter()
        .flat_map(|b| b.completions.iter().map(|c| c.id))
        .collect();
    let session_ids: Vec<Uuid> = series_doc
        .books
        .iter()
        .flat_map(|b| b.sessions.iter().flatten().map(|s| s.id))
        .collect();
    let completions = existing_completions(db, &completion_ids).await?;
    let sessions = existing_sessions(db, &session_ids).await?;
    let referenced: Vec<Uuid> = completions
        .values()
        .chain(sessions.values())
        .filter_map(|row| row.book_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let lookups = SeriesLookups {
        progress: progress_for_books(db, user_id, &book_ids).await?,
        soft_deleted: soft_deleted_books(db, &referenced).await?,
        completions,
        sessions,
    };

    let plans = series_doc
        .books
        .iter()
        .map(|book_doc| build_book_plan(user_id, book_doc, &candidates, &lookups, planned, options))
        .collect();

    let existing_rating = match planned.ratings.get(&series_id) {
        Some(row) => Some(row.clone()),
        None => UserSeriesRatingRepository::get_by_user_and_series(db, user_id, series_id).await?,
    };
    let rating_decision = decide_rating(
        existing_rating.as_ref(),
        series_doc,
        options.conflict_policy,
    );
    let planned_rating = match &rating_decision {
        RatingDecision::Insert {
            id,
            rating,
            notes,
            updated_at,
        } => Some(user_series_ratings::Model {
            id: *id,
            user_id,
            series_id,
            rating: *rating,
            notes: notes.clone(),
            created_at: *updated_at,
            updated_at: *updated_at,
        }),
        RatingDecision::Update {
            existing,
            rating,
            notes,
            updated_at,
        } => Some(user_series_ratings::Model {
            rating: *rating,
            notes: notes.clone(),
            updated_at: *updated_at,
            ..existing.clone()
        }),
        RatingDecision::NoOp | RatingDecision::Skip => existing_rating,
    };
    if let Some(row) = planned_rating {
        planned.ratings.insert(series_id, row);
    }

    Ok((plans, rating_decision))
}

fn unresolved_book_reports(
    series_doc: &ExportSeriesDto,
    disposition: BookDisposition,
) -> Vec<ImportBookReport> {
    series_doc
        .books
        .iter()
        .map(|b| ImportBookReport {
            path: b.path.clone(),
            file_name: b.file_name.clone(),
            disposition,
            matched_book_id: None,
            applied: false,
            progress: None,
            completions: WriteCounts::default(),
            sessions: WriteCounts::default(),
        })
        .collect()
}

/// The report for a series that could not be planned at all.
fn failed_series_report(
    summary: &mut ImportSummary,
    series_doc: &ExportSeriesDto,
    disposition: SeriesDisposition,
    matched_series_id: Option<Uuid>,
    err: anyhow::Error,
) -> ImportSeriesReport {
    let books = unresolved_book_reports(series_doc, BookDisposition::Unmatched);
    for report in &books {
        tally_book_report(summary, report, false);
    }
    ImportSeriesReport {
        library_relative_path: series_doc.library_relative_path.clone(),
        name: series_doc.name.clone(),
        disposition,
        matched_series_id,
        attempted: matched_series_id.is_some(),
        committed: false,
        error: Some(err.to_string()),
        rating: None,
        books,
    }
}

/// Apply (or dry-run) a matched export document for one user.
pub async fn import_reading_progress(
    db: &DatabaseConnection,
    user_id: Uuid,
    document: &ReadingProgressExportDocument,
    options: &ImportOptions,
) -> Result<ImportReadingProgressResponse, ImportError> {
    validate_document(document)?;

    let content_filter = ContentFilter::for_user(db, user_id)
        .await
        .map_err(ImportError::Database)?;

    let mut notices = Vec::new();
    if options.reattach_sessions && !document.includes_sessions {
        notices.push(
            "reattach_sessions has no effect: the imported file does not include sessions."
                .to_string(),
        );
    }

    let mut summary = ImportSummary::default();
    let mut series_reports = Vec::with_capacity(document.series.len());
    let mut planned = PlannedState::default();

    for series_doc in &document.series {
        summary.series_total += 1;

        // A failure in one series is reported on that series and the import
        // carries on: earlier series may already have committed, and the
        // report is the only place the reader learns which did.
        let series_match = match matching::resolve_series(
            db,
            &content_filter,
            series_doc,
            options.source_preference.as_deref(),
            options.library_ids.as_deref(),
        )
        .await
        {
            Ok(found) => found,
            Err(err) => {
                summary.series_unmatched += 1;
                series_reports.push(failed_series_report(
                    &mut summary,
                    series_doc,
                    SeriesDisposition::Unmatched,
                    None,
                    err,
                ));
                continue;
            }
        };

        let series_id = match series_match {
            SeriesMatch::Matched(series_id) => series_id,
            SeriesMatch::Ambiguous | SeriesMatch::Unmatched => {
                let (disposition, book_disposition) = if series_match == SeriesMatch::Ambiguous {
                    summary.series_ambiguous += 1;
                    (SeriesDisposition::Ambiguous, BookDisposition::Ambiguous)
                } else {
                    summary.series_unmatched += 1;
                    (SeriesDisposition::Unmatched, BookDisposition::Unmatched)
                };
                let books = unresolved_book_reports(series_doc, book_disposition);
                for report in &books {
                    tally_book_report(&mut summary, report, false);
                }
                series_reports.push(ImportSeriesReport {
                    library_relative_path: series_doc.library_relative_path.clone(),
                    name: series_doc.name.clone(),
                    disposition,
                    matched_series_id: None,
                    attempted: false,
                    committed: false,
                    error: None,
                    rating: None,
                    books,
                });
                continue;
            }
        };
        summary.series_matched += 1;

        // Restored if this series does not land, so later series are not
        // planned against writes that never happened.
        let before_series = planned.clone();

        let (plans, rating_decision) =
            match plan_series(db, user_id, series_id, series_doc, &mut planned, options).await {
                Ok(planned_series) => planned_series,
                Err(err) => {
                    planned = before_series;
                    series_reports.push(failed_series_report(
                        &mut summary,
                        series_doc,
                        SeriesDisposition::Matched,
                        Some(series_id),
                        err,
                    ));
                    continue;
                }
            };

        let outcome = if options.dry_run {
            Ok(false)
        } else {
            apply_series(db, user_id, series_id, &plans, &rating_decision)
                .await
                .map(|()| true)
        };

        let books: Vec<ImportBookReport> = plans.iter().map(book_report_from_plan).collect();
        match outcome {
            Ok(committed) => {
                if committed {
                    summary.series_committed += 1;
                }
                for report in &books {
                    tally_book_report(&mut summary, report, true);
                }
                tally_rating_write(&mut summary, &rating_decision);
                series_reports.push(ImportSeriesReport {
                    library_relative_path: series_doc.library_relative_path.clone(),
                    name: series_doc.name.clone(),
                    disposition: SeriesDisposition::Matched,
                    matched_series_id: Some(series_id),
                    attempted: true,
                    committed,
                    error: None,
                    rating: rating_decision.outcome(),
                    books,
                });
            }
            Err(err) => {
                planned = before_series;
                for report in &books {
                    tally_book_report(&mut summary, report, false);
                }
                series_reports.push(ImportSeriesReport {
                    library_relative_path: series_doc.library_relative_path.clone(),
                    name: series_doc.name.clone(),
                    disposition: SeriesDisposition::Matched,
                    matched_series_id: Some(series_id),
                    attempted: true,
                    committed: false,
                    error: Some(err.to_string()),
                    rating: None,
                    books,
                });
            }
        }
    }

    // A stranded row means the progress moved and the reading history did
    // not, which the per-book counts show but the summary otherwise reads as
    // success. Say it plainly: the reader can still fix it by removing or
    // rescanning the other library and importing again.
    if summary.rows_stranded > 0 {
        notices.push(format!(
            "{} session/completion rows were left behind: their books are still \
             live in another library. Progress moved but reading history did not. \
             Delete or rescan that library so its books are no longer on disk, \
             then import again to bring the history across.",
            summary.rows_stranded
        ));
    }

    Ok(ImportReadingProgressResponse {
        dry_run: options.dry_run,
        sessions_in_file: document.includes_sessions,
        notices,
        summary,
        series: series_reports,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reading_transfer::model::ExportProgressDto;

    fn progress_row(
        current_page: i32,
        completed: bool,
        updated_at: DateTime<Utc>,
    ) -> read_progress::Model {
        read_progress::Model {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            book_id: Uuid::new_v4(),
            current_page,
            progress_percentage: None,
            completed,
            started_at: updated_at,
            updated_at,
            completed_at: if completed { Some(updated_at) } else { None },
            r2_progression: None,
        }
    }

    fn imported_progress(
        current_page: i32,
        completed: bool,
        updated_at: DateTime<Utc>,
    ) -> ExportProgressDto {
        ExportProgressDto {
            current_page,
            progress_percentage: None,
            completed,
            started_at: updated_at,
            updated_at,
            completed_at: if completed { Some(updated_at) } else { None },
            r2_progression: None,
        }
    }

    fn t(minutes: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap() + chrono::Duration::minutes(minutes)
    }

    use chrono::TimeZone;

    #[test]
    fn progress_inserts_when_nothing_exists() {
        let imported = imported_progress(10, false, t(0));
        let decision = decide_progress(None, &imported, ConflictPolicy::Newest);
        assert!(matches!(decision, ProgressDecision::Insert(_, _)));
    }

    #[test]
    fn progress_skip_existing_leaves_existing_row_alone() {
        let existing = progress_row(5, false, t(0));
        let imported = imported_progress(50, false, t(10));
        let decision = decide_progress(Some(&existing), &imported, ConflictPolicy::SkipExisting);
        assert!(matches!(decision, ProgressDecision::Skip));
    }

    #[test]
    fn progress_overwrite_always_replaces() {
        let existing = progress_row(50, false, t(10));
        let imported = imported_progress(1, false, t(0));
        let decision = decide_progress(Some(&existing), &imported, ConflictPolicy::Overwrite);
        assert!(matches!(decision, ProgressDecision::Update(_, _)));
    }

    #[test]
    fn progress_newest_picks_the_later_updated_at() {
        let existing = progress_row(5, false, t(0));
        let newer_import = imported_progress(3, false, t(10));
        assert!(matches!(
            decide_progress(Some(&existing), &newer_import, ConflictPolicy::Newest),
            ProgressDecision::Update(_, _)
        ));

        let older_import = imported_progress(999, false, t(-10));
        assert!(matches!(
            decide_progress(Some(&existing), &older_import, ConflictPolicy::Newest),
            ProgressDecision::Skip
        ));
    }

    #[test]
    fn progress_furthest_prefers_the_higher_position() {
        let existing = progress_row(10, false, t(0));
        let further_import = imported_progress(50, false, t(-100)); // older, but further
        assert!(matches!(
            decide_progress(Some(&existing), &further_import, ConflictPolicy::Furthest),
            ProgressDecision::Update(_, _)
        ));

        let behind_import = imported_progress(2, false, t(100)); // newer, but behind
        assert!(matches!(
            decide_progress(Some(&existing), &behind_import, ConflictPolicy::Furthest),
            ProgressDecision::Skip
        ));
    }

    #[test]
    fn progress_furthest_treats_completed_as_further_than_any_partial_position() {
        let existing = progress_row(999, false, t(0));
        let completed_import = imported_progress(1, true, t(-100));
        assert!(matches!(
            decide_progress(Some(&existing), &completed_import, ConflictPolicy::Furthest),
            ProgressDecision::Update(_, _)
        ));
    }

    fn rating_row(rating: i32) -> user_series_ratings::Model {
        user_series_ratings::Model {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            series_id: Uuid::new_v4(),
            rating,
            notes: None,
            created_at: t(0),
            updated_at: t(0),
        }
    }

    fn series_with_rating(rating: Option<i32>) -> ExportSeriesDto {
        rated_at(rating, None)
    }

    fn rated_at(rating: Option<i32>, when: Option<DateTime<Utc>>) -> ExportSeriesDto {
        ExportSeriesDto {
            external_ids: vec![],
            library_relative_path: "s".to_string(),
            name: "S".to_string(),
            rating,
            notes: None,
            rating_updated_at: when,
            books: vec![],
        }
    }

    #[test]
    fn rating_no_op_when_file_carries_none() {
        let doc = series_with_rating(None);
        assert!(matches!(
            decide_rating(None, &doc, ConflictPolicy::Overwrite),
            RatingDecision::NoOp
        ));
    }

    #[test]
    fn rating_inserts_when_nothing_exists() {
        let doc = series_with_rating(Some(80));
        assert!(matches!(
            decide_rating(None, &doc, ConflictPolicy::Newest),
            RatingDecision::Insert { .. }
        ));
    }

    #[test]
    fn rating_skip_existing_leaves_existing_alone() {
        let existing = rating_row(40);
        let doc = series_with_rating(Some(90));
        assert!(matches!(
            decide_rating(Some(&existing), &doc, ConflictPolicy::SkipExisting),
            RatingDecision::Skip
        ));
    }

    #[test]
    fn rating_overwrite_always_replaces() {
        let existing = rating_row(40);
        let doc = series_with_rating(Some(90));
        assert!(matches!(
            decide_rating(Some(&existing), &doc, ConflictPolicy::Overwrite),
            RatingDecision::Update { .. }
        ));
    }

    /// The default policy must not let an old export undo a rating the reader
    /// changed after taking it.
    #[test]
    fn rating_newest_replaces_only_a_strictly_older_rating() {
        let existing = rating_row(40); // updated at t(0)
        for policy in [ConflictPolicy::Newest, ConflictPolicy::Furthest] {
            assert!(matches!(
                decide_rating(Some(&existing), &rated_at(Some(90), Some(t(10))), policy),
                RatingDecision::Update { .. }
            ));
            assert!(matches!(
                decide_rating(Some(&existing), &rated_at(Some(90), Some(t(-10))), policy),
                RatingDecision::Skip
            ));
            assert!(
                matches!(
                    decide_rating(Some(&existing), &rated_at(Some(90), Some(t(0))), policy),
                    RatingDecision::Skip
                ),
                "an equal timestamp is the same rating re-imported"
            );
            assert!(
                matches!(
                    decide_rating(Some(&existing), &series_with_rating(Some(90)), policy),
                    RatingDecision::Skip
                ),
                "without a timestamp the file cannot show it is newer"
            );
        }
    }

    #[test]
    fn rejects_unknown_format() {
        let doc = ReadingProgressExportDocument {
            format: "something-else".to_string(),
            version: 1,
            exported_at: t(0),
            includes_sessions: true,
            series: vec![],
        };
        assert!(matches!(
            validate_document(&doc),
            Err(ImportError::UnknownFormat(_))
        ));
    }

    #[test]
    fn rejects_a_version_newer_than_supported() {
        let doc = ReadingProgressExportDocument {
            format: READING_PROGRESS_FORMAT.to_string(),
            version: READING_PROGRESS_VERSION + 1,
            exported_at: t(0),
            includes_sessions: true,
            series: vec![],
        };
        assert!(matches!(
            validate_document(&doc),
            Err(ImportError::UnsupportedVersion(_))
        ));
    }

    #[test]
    fn accepts_the_current_format_and_version() {
        let doc = ReadingProgressExportDocument {
            format: READING_PROGRESS_FORMAT.to_string(),
            version: READING_PROGRESS_VERSION,
            exported_at: t(0),
            includes_sessions: true,
            series: vec![],
        };
        assert!(validate_document(&doc).is_ok());
    }

    // ------------------------------------------------------------------
    // Row decisions: insert / reattach / skip for completions and sessions.
    // ------------------------------------------------------------------

    fn row(user_id: Uuid, book_id: Option<Uuid>) -> ExistingRow {
        ExistingRow { user_id, book_id }
    }

    #[test]
    fn row_inserts_when_absent() {
        let mut planned = PlannedState::default();
        let decision = decide_row(
            Uuid::new_v4(),
            None,
            Uuid::new_v4(),
            Uuid::new_v4(),
            &HashSet::new(),
            true,
            &mut planned,
        );
        assert_eq!(decision, RowDecision::Insert);
    }

    #[test]
    fn row_reattaches_an_orphan_only_when_the_flag_is_on() {
        let user = Uuid::new_v4();
        let target = Uuid::new_v4();
        let orphan = row(user, None);
        for (flag, expected) in [(true, RowDecision::Reattach), (false, RowDecision::Skip)] {
            let mut planned = PlannedState::default();
            let decision = decide_row(
                Uuid::new_v4(),
                Some(&orphan),
                target,
                user,
                &HashSet::new(),
                flag,
                &mut planned,
            );
            assert_eq!(decision, expected);
        }
    }

    /// A moved file leaves its old book soft-deleted with the history still
    /// on it; importing onto the new book moves that history across.
    #[test]
    fn row_on_a_soft_deleted_book_moves_to_the_matched_book() {
        let user = Uuid::new_v4();
        let old_book = Uuid::new_v4();
        let mut planned = PlannedState::default();
        let decision = decide_row(
            Uuid::new_v4(),
            Some(&row(user, Some(old_book))),
            Uuid::new_v4(),
            user,
            &HashSet::from([old_book]),
            true,
            &mut planned,
        );
        assert_eq!(decision, RowDecision::Reattach);
    }

    /// Left alone, but not for the harmless reason a plain `Skip` means.
    /// The row is on a live book in another library: reattaching would strip
    /// history from a library the reader may still be using, and skipping it
    /// silently would hide that the progress moved without it. Hence its own
    /// variant, which the report turns into a notice.
    #[test]
    fn row_on_a_live_book_elsewhere_is_left_alone_but_flagged() {
        let user = Uuid::new_v4();
        let live = Uuid::new_v4();
        let mut planned = PlannedState::default();
        let decision = decide_row(
            Uuid::new_v4(),
            Some(&row(user, Some(live))),
            Uuid::new_v4(),
            user,
            &HashSet::new(),
            true,
            &mut planned,
        );
        assert_eq!(decision, RowDecision::SkipStranded);
    }

    /// The genuinely harmless skip: the row is already on the book being
    /// imported onto, which is what makes re-importing the same file a no-op.
    #[test]
    fn row_already_on_the_target_book_is_a_plain_skip() {
        let user = Uuid::new_v4();
        let target = Uuid::new_v4();
        let mut planned = PlannedState::default();
        let decision = decide_row(
            Uuid::new_v4(),
            Some(&row(user, Some(target))),
            target,
            user,
            &HashSet::new(),
            true,
            &mut planned,
        );
        assert_eq!(decision, RowDecision::Skip);
    }

    #[test]
    fn another_users_row_is_never_touched() {
        let mut planned = PlannedState::default();
        let decision = decide_row(
            Uuid::new_v4(),
            Some(&row(Uuid::new_v4(), None)),
            Uuid::new_v4(),
            Uuid::new_v4(),
            &HashSet::new(),
            true,
            &mut planned,
        );
        assert_eq!(decision, RowDecision::Skip);
    }

    #[test]
    fn an_id_listed_twice_is_written_once() {
        let id = Uuid::new_v4();
        let mut planned = PlannedState::default();
        let args = |planned: &mut PlannedState| {
            decide_row(
                id,
                None,
                Uuid::new_v4(),
                Uuid::new_v4(),
                &HashSet::new(),
                true,
                planned,
            )
        };
        assert_eq!(args(&mut planned), RowDecision::Insert);
        assert_eq!(args(&mut planned), RowDecision::Skip);
    }
}
