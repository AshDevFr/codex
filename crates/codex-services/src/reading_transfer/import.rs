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
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, Set, TransactionTrait,
};
use std::fmt;
use uuid::Uuid;

use codex_db::entities::{read_completions, read_progress, reading_sessions, user_series_ratings};
use codex_db::repositories::{
    BookRepository, LibraryRepository, ReadProgressRepository, SeriesRepository,
    UserSeriesRatingRepository,
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
    pub source_preference: Vec<String>,
    pub conflict_policy: ConflictPolicy,
    pub reattach_sessions: bool,
    pub accept_stem_matches: bool,
}

/// A rejection worth a 400, versus every other failure which is a 500.
#[derive(Debug)]
pub enum ImportError {
    UnknownFormat(String),
    UnsupportedVersion(i32),
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
    Ok(())
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
    Insert(ProgressValues),
    Update(Uuid, ProgressValues),
    Skip,
}

impl ProgressDecision {
    fn outcome(&self) -> FieldOutcome {
        match self {
            ProgressDecision::Insert(_) => FieldOutcome::Inserted,
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
        return ProgressDecision::Insert(values);
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
        rating: i32,
        notes: Option<String>,
    },
    Update {
        existing: user_series_ratings::Model,
        rating: i32,
        notes: Option<String>,
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

/// The export document does not carry a timestamp for a series rating (only
/// `rating` and `notes`), so `newest` and `furthest` have nothing to compare
/// against and both fall back to `overwrite`. There is no meaningful "further
/// into a rating" either way, so this loses nothing `furthest` could have
/// expressed.
fn decide_rating(
    existing: Option<&user_series_ratings::Model>,
    series_doc: &ExportSeriesDto,
    policy: ConflictPolicy,
) -> RatingDecision {
    let Some(rating) = series_doc.rating else {
        return RatingDecision::NoOp;
    };
    let notes = series_doc.notes.clone();

    match existing {
        None => RatingDecision::Insert { rating, notes },
        Some(existing) => match policy {
            ConflictPolicy::SkipExisting => RatingDecision::Skip,
            ConflictPolicy::Overwrite | ConflictPolicy::Newest | ConflictPolicy::Furthest => {
                RatingDecision::Update {
                    existing: existing.clone(),
                    rating,
                    notes,
                }
            }
        },
    }
}

/// What happens to one append-only row (`read_completions` / `reading_sessions`)
/// whose primary key is the id reused from the export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowDecision {
    /// No row with this id exists yet.
    Insert,
    /// A row exists, belongs to this user, and its `book_id` is `NULL`: the
    /// book it was recorded against was hard-deleted after export. Adopt it.
    Reattach,
    /// A row already exists with a book attached (re-importing the same
    /// file), or belongs to someone else (a UUID collision that should never
    /// happen in practice, handled defensively by leaving it alone).
    Skip,
}

async fn decide_completion_row(
    db: &DatabaseConnection,
    user_id: Uuid,
    id: Uuid,
    reattach_sessions: bool,
) -> Result<RowDecision> {
    let existing = read_completions::Entity::find_by_id(id).one(db).await?;
    Ok(match existing {
        None => RowDecision::Insert,
        Some(row) if row.user_id == user_id && row.book_id.is_none() => {
            if reattach_sessions {
                RowDecision::Reattach
            } else {
                RowDecision::Skip
            }
        }
        Some(_) => RowDecision::Skip,
    })
}

async fn decide_session_row(
    db: &DatabaseConnection,
    user_id: Uuid,
    id: Uuid,
    reattach_sessions: bool,
) -> Result<RowDecision> {
    let existing = reading_sessions::Entity::find_by_id(id).one(db).await?;
    Ok(match existing {
        None => RowDecision::Insert,
        Some(row) if row.user_id == user_id && row.book_id.is_none() => {
            if reattach_sessions {
                RowDecision::Reattach
            } else {
                RowDecision::Skip
            }
        }
        Some(_) => RowDecision::Skip,
    })
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

async fn build_book_plan<'a>(
    db: &DatabaseConnection,
    user_id: Uuid,
    book_doc: &'a ExportBookDto,
    candidates: &[BookCandidate],
    options: &ImportOptions,
) -> Result<BookPlan<'a>> {
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

    if !applied {
        return Ok(BookPlan {
            book_doc,
            disposition,
            matched_book_id,
            applied: false,
            progress: None,
            completions: vec![],
            sessions: vec![],
        });
    }

    let book_id = matched_book_id.expect("applied implies a matched book id");

    let progress = match &book_doc.progress {
        None => None,
        Some(imported) => {
            let existing =
                ReadProgressRepository::get_by_user_and_book(db, user_id, book_id).await?;
            Some(decide_progress(
                existing.as_ref(),
                imported,
                options.conflict_policy,
            ))
        }
    };

    let mut completions = Vec::with_capacity(book_doc.completions.len());
    for completion in &book_doc.completions {
        let decision =
            decide_completion_row(db, user_id, completion.id, options.reattach_sessions).await?;
        completions.push((completion, decision));
    }

    let mut sessions = Vec::new();
    if let Some(session_docs) = &book_doc.sessions {
        sessions.reserve(session_docs.len());
        for session in session_docs {
            let decision =
                decide_session_row(db, user_id, session.id, options.reattach_sessions).await?;
            sessions.push((session, decision));
        }
    }

    Ok(BookPlan {
        book_doc,
        disposition,
        matched_book_id,
        applied: true,
        progress,
        completions,
        sessions,
    })
}

fn book_report_from_plan(plan: &BookPlan<'_>) -> ImportBookReport {
    let mut completions = WriteCounts::default();
    for (_, decision) in &plan.completions {
        match decision {
            RowDecision::Insert => completions.inserted += 1,
            RowDecision::Reattach => completions.reattached += 1,
            RowDecision::Skip => completions.skipped += 1,
        }
    }
    let mut sessions = WriteCounts::default();
    for (_, decision) in &plan.sessions {
        match decision {
            RowDecision::Insert => sessions.inserted += 1,
            RowDecision::Reattach => sessions.reattached += 1,
            RowDecision::Skip => sessions.skipped += 1,
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
        ProgressDecision::Insert(values) => {
            read_progress::ActiveModel {
                id: Set(Uuid::new_v4()),
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
        RowDecision::Skip => Ok(()),
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
            if let Some(row) = read_completions::Entity::find_by_id(doc.id)
                .one(txn)
                .await?
            {
                let mut active: read_completions::ActiveModel = row.into();
                active.book_id = Set(Some(book_id));
                active.update(txn).await?;
            }
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
        RowDecision::Skip => Ok(()),
        RowDecision::Insert => {
            reading_sessions::ActiveModel {
                id: Set(doc.id),
                user_id: Set(user_id),
                book_id: Set(Some(book_id)),
                device_id: Set(doc.device_id.clone()),
                device_name: Set(doc.device_name.clone()),
                pass: Set(doc.pass),
                kind: Set(doc.kind.clone()),
                to_page: Set(doc.to_page),
                to_percentage: Set(doc.to_percentage),
                // The export never carries a historical locator; only the
                // live `read_progress` row keeps one.
                r2_progression: Set(None),
                active_duration_ms: Set(doc.active_duration_ms),
                duration_source: Set(doc.duration_source.clone()),
                pages_read: Set(doc.pages_read),
                client_started_at: Set(doc.client_started_at),
                client_ended_at: Set(doc.client_ended_at),
                server_recorded_at: Set(doc.server_recorded_at),
            }
            .insert(txn)
            .await?;
            Ok(())
        }
        RowDecision::Reattach => {
            if let Some(row) = reading_sessions::Entity::find_by_id(doc.id)
                .one(txn)
                .await?
            {
                let mut active: reading_sessions::ActiveModel = row.into();
                active.book_id = Set(Some(book_id));
                active.update(txn).await?;
            }
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
        RatingDecision::Insert { rating, notes } => {
            let now = Utc::now();
            user_series_ratings::ActiveModel {
                id: Set(Uuid::new_v4()),
                user_id: Set(user_id),
                series_id: Set(series_id),
                rating: Set(*rating),
                notes: Set(notes.clone()),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(txn)
            .await?;
            Ok(())
        }
        RatingDecision::Update {
            existing,
            rating,
            notes,
        } => {
            let mut active: user_series_ratings::ActiveModel = existing.clone().into();
            active.rating = Set(*rating);
            active.notes = Set(notes.clone());
            active.updated_at = Set(Utc::now());
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

    let mut plans = Vec::with_capacity(series_doc.books.len());
    for book_doc in &series_doc.books {
        plans.push(build_book_plan(db, user_id, book_doc, &candidates, options).await?);
    }

    let existing_rating =
        UserSeriesRatingRepository::get_by_user_and_series(db, user_id, series_id).await?;
    let rating_decision = decide_rating(
        existing_rating.as_ref(),
        series_doc,
        options.conflict_policy,
    );

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

    for series_doc in &document.series {
        summary.series_total += 1;

        let series_match =
            matching::resolve_series(db, &content_filter, series_doc, &options.source_preference)
                .await
                .map_err(ImportError::Database)?;

        match series_match {
            SeriesMatch::Ambiguous | SeriesMatch::Unmatched => {
                let disposition = if series_match == SeriesMatch::Ambiguous {
                    summary.series_ambiguous += 1;
                    SeriesDisposition::Ambiguous
                } else {
                    summary.series_unmatched += 1;
                    SeriesDisposition::Unmatched
                };
                let book_disposition = if disposition == SeriesDisposition::Ambiguous {
                    BookDisposition::Ambiguous
                } else {
                    BookDisposition::Unmatched
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
            }
            SeriesMatch::Matched(series_id) => {
                summary.series_matched += 1;

                let (plans, rating_decision) =
                    plan_series(db, user_id, series_id, series_doc, options)
                        .await
                        .map_err(ImportError::Database)?;

                if options.dry_run {
                    let books: Vec<ImportBookReport> =
                        plans.iter().map(book_report_from_plan).collect();
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
                        committed: false,
                        error: None,
                        rating: rating_decision.outcome(),
                        books,
                    });
                } else {
                    match apply_series(db, user_id, series_id, &plans, &rating_decision).await {
                        Ok(()) => {
                            summary.series_committed += 1;
                            let books: Vec<ImportBookReport> =
                                plans.iter().map(book_report_from_plan).collect();
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
                                committed: true,
                                error: None,
                                rating: rating_decision.outcome(),
                                books,
                            });
                        }
                        Err(err) => {
                            let books: Vec<ImportBookReport> =
                                plans.iter().map(book_report_from_plan).collect();
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
            }
        }
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
        assert!(matches!(decision, ProgressDecision::Insert(_)));
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
        ExportSeriesDto {
            external_ids: vec![],
            library_relative_path: "s".to_string(),
            name: "S".to_string(),
            rating,
            notes: None,
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
    fn rating_overwrite_and_newest_and_furthest_all_replace() {
        let existing = rating_row(40);
        let doc = series_with_rating(Some(90));
        for policy in [
            ConflictPolicy::Overwrite,
            ConflictPolicy::Newest,
            ConflictPolicy::Furthest,
        ] {
            assert!(matches!(
                decide_rating(Some(&existing), &doc, policy),
                RatingDecision::Update { .. }
            ));
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

    use codex_db::ScanningStrategy;
    use codex_db::repositories::{
        LibraryRepository, ReadCompletionRepository, SeriesRepository, UserRepository,
    };
    use codex_db::test_helpers::create_test_db;

    async fn make_user(db: &DatabaseConnection) -> Uuid {
        use codex_db::entities::users;
        let now = Utc::now();
        let model = users::Model {
            id: Uuid::new_v4(),
            username: format!("u-{}", Uuid::new_v4()),
            email: format!("{}@test.com", Uuid::new_v4()),
            password_hash: "hash".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: true,
            permissions: serde_json::json!([]),
            created_at: now,
            updated_at: now,
            last_login_at: None,
        };
        UserRepository::create(db, &model).await.unwrap().id
    }

    #[tokio::test]
    async fn completion_row_inserts_when_absent() {
        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn).await;
        let decision = decide_completion_row(conn, user, Uuid::new_v4(), true)
            .await
            .unwrap();
        assert_eq!(decision, RowDecision::Insert);
    }

    #[tokio::test]
    async fn completion_row_reattaches_an_orphan_when_flag_is_on() {
        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn).await;
        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(conn, library.id, "S", None)
            .await
            .unwrap();
        let book = codex_db::entities::books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            path: "/lib/b.cbz".to_string(),
            file_name: "b.cbz".to_string(),
            file_size: 1,
            file_hash: String::new(),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 1,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            analysis_errors: None,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
            koreader_hash: None,
            epub_positions: None,
            epub_spine_items: None,
        };
        let book = BookRepository::create(conn, &book, None).await.unwrap();
        let completion =
            ReadCompletionRepository::record(conn, user, book.id, Utc::now(), Utc::now())
                .await
                .unwrap();

        // Hard-delete the book: the FK's ON DELETE SET NULL orphans the row.
        codex_db::entities::books::Entity::delete_by_id(book.id)
            .exec(conn)
            .await
            .unwrap();

        let decision = decide_completion_row(conn, user, completion.id, true)
            .await
            .unwrap();
        assert_eq!(decision, RowDecision::Reattach);

        let decision_off = decide_completion_row(conn, user, completion.id, false)
            .await
            .unwrap();
        assert_eq!(decision_off, RowDecision::Skip);
    }

    #[tokio::test]
    async fn session_row_skips_when_already_attached() {
        use codex_db::entities::reading_sessions::SessionKind;
        use codex_db::repositories::{NewSession, ReadProgressRepository};

        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = make_user(conn).await;
        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(conn, library.id, "S", None)
            .await
            .unwrap();
        let book = codex_db::entities::books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            path: "/lib/b.cbz".to_string(),
            file_name: "b.cbz".to_string(),
            file_size: 1,
            file_hash: String::new(),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 1,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            analysis_errors: None,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
            koreader_hash: None,
            epub_positions: None,
            epub_spine_items: None,
        };
        let book = BookRepository::create(conn, &book, None).await.unwrap();

        let session_id = Uuid::new_v4();
        let now = Utc::now();
        let session = NewSession::from_client(
            session_id,
            user,
            book.id,
            "device",
            None,
            SessionKind::Progress,
            Some(1000),
            Some(1),
            now,
            now,
        )
        .with_page(1);
        ReadProgressRepository::record_session(conn, session)
            .await
            .unwrap();

        // Still attached: importing the same id again must skip, not reattach.
        let decision = decide_session_row(conn, user, session_id, true)
            .await
            .unwrap();
        assert_eq!(decision, RowDecision::Skip);
    }
}
