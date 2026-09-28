//! The reading-progress export/import document and the import request and
//! report shapes.
//!
//! These types are the wire format for `GET /api/v1/reading-progress/export`
//! and `POST /api/v1/reading-progress/import`, defined here (rather than in
//! `codex-api`'s DTO module, where every other request/response type lives)
//! because [`export`](super::export) produces them and
//! [`import`](super::import) both consumes and produces them directly; the
//! API layer only serializes what the service already built. `codex-api`
//! re-exports this module under its `dto` namespace for OpenAPI registration.
//!
//! The document is a portable file meant to survive a library reorganisation
//! or a move to a different Codex instance, but it is an ordinary API payload
//! too: the export returns it as a response body and the import nests it in
//! its request. So the structs here are `camelCase` on the wire like every
//! other DTO, and a single import body does not carry two naming conventions
//! with an invisible boundary between them. The enums stay `snake_case`,
//! matching how enum variants are spelled everywhere else in the API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// The only document shape this server currently writes, and the highest
/// `version` it knows how to read.
pub const READING_PROGRESS_FORMAT: &str = "codex-reading-progress";
pub const READING_PROGRESS_VERSION: i32 = 1;

/// One external identifier attached to a series (a plugin match, a ComicInfo
/// value, or a manual entry).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportExternalIdDto {
    /// `plugin:<name>`, `comicinfo`, `epub`, or `manual`.
    #[schema(example = "plugin:mangabaka")]
    pub source: String,
    #[schema(example = "12345")]
    pub id: String,
}

/// The live resume position for one book. Retains `r2_progression`: it is the
/// only place the EPUB locator survives, since sessions strip it.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgressDto {
    pub current_page: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress_percentage: Option<f64>,
    pub completed: bool,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r2_progression: Option<String>,
}

/// One finished read-through. Keeps its original id so re-importing the same
/// file is a no-op rather than a duplicate.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportCompletionDto {
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

/// One row from the reading-session log. `r2_progression` is deliberately
/// absent: nothing reads a session's historical locator, and it is the only
/// non-scalar column on the row.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportSessionDto {
    pub id: Uuid,
    pub device_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    pub pass: i32,
    /// `"progress"`, `"completed"`, or `"reset"`.
    #[schema(example = "progress")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_page: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_percentage: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_duration_ms: Option<i64>,
    /// `"measured"`, `"inferred"`, or `"unknown"`.
    #[schema(example = "measured")]
    pub duration_source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages_read: Option<i32>,
    pub client_started_at: DateTime<Utc>,
    pub client_ended_at: DateTime<Utc>,
    pub server_recorded_at: DateTime<Utc>,
}

/// One book inside a series, keyed for matching by path, name, and hash
/// rather than by id: the whole point of the file is that ids on the far side
/// are expected to be different.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportBookDto {
    /// Relative to the series folder, so a series move does not invalidate it.
    #[schema(example = "Vol 01/v01.cbz")]
    pub path: String,
    #[schema(example = "v01.cbz")]
    pub file_name: String,
    /// Empty when the book was never analyzed; never treated as a value to
    /// match on in that case.
    #[serde(default)]
    pub file_hash: String,
    #[serde(default)]
    pub partial_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<ExportProgressDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub completions: Vec<ExportCompletionDto>,
    /// Omitted entirely (not an empty array) when the export was taken with
    /// `includeSessions=false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sessions: Option<Vec<ExportSessionDto>>,
}

/// One series and everything the exporting user recorded against its books.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportSeriesDto {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_ids: Vec<ExportExternalIdDto>,
    /// The series path as stored, relative to the library root.
    #[schema(example = "shonen/Naruto")]
    pub library_relative_path: String,
    #[schema(example = "Naruto")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// When the rating was last changed. The `newest` conflict policy needs it
    /// to tell a stale rating from a fresh one; a file without it never
    /// overwrites an existing rating except under `overwrite`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating_updated_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub books: Vec<ExportBookDto>,
}

/// The whole export: one user's reading state, self-describing enough to be
/// matched back against a differently organised library.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadingProgressExportDocument {
    #[schema(example = "codex-reading-progress")]
    pub format: String,
    #[schema(example = 1)]
    pub version: i32,
    pub exported_at: DateTime<Utc>,
    pub includes_sessions: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub series: Vec<ExportSeriesDto>,
}

/// How aggressively hashes are used to match a book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum HashMode {
    /// Hashes are ignored entirely.
    Off,
    /// Match by path/name, then reject a pair whose `file_hash` disagrees.
    #[default]
    Verify,
    /// Additionally use `file_hash` / `partial_hash` as a matching key when
    /// the path and name steps find nothing, which rescues a bulk rename.
    Match,
}

/// How a conflict between an imported value and an existing row is resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    /// Whichever side has the later `updated_at` wins.
    #[default]
    Newest,
    /// Whichever side is further into the book wins; a completed read beats
    /// any partial one.
    Furthest,
    /// An existing row is left untouched; only a missing row is written.
    SkipExisting,
    /// The imported value always wins.
    Overwrite,
}

fn default_reattach_sessions() -> bool {
    true
}

/// `POST /api/v1/reading-progress/import` request body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportReadingProgressRequest {
    /// Compute and report the outcome without writing anything.
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub hash_mode: HashMode,
    /// External-id sources to try, in order, before falling back to path and
    /// then normalized name. An empty list skips straight to path matching.
    #[serde(default)]
    pub source_preference: Vec<String>,
    #[serde(default)]
    pub conflict_policy: ConflictPolicy,
    /// When a session or completion in the file already exists as the
    /// importer's own row but is not on a live book (its book was hard-deleted,
    /// leaving `book_id` null, or the scanner marked it deleted after the file
    /// moved), move it onto the matched book instead of skipping it.
    #[serde(default = "default_reattach_sessions")]
    pub reattach_sessions: bool,
    /// A stem match (`v01.cbr` renamed to `v01.cbz`) is reported either way,
    /// but only written when this is set: two files can share a stem, and
    /// applying it silently risks writing progress onto the wrong one.
    #[serde(default)]
    pub accept_stem_matches: bool,
    pub file: ReadingProgressExportDocument,
}

/// Why a series in the import file could not be resolved to exactly one
/// series in the current library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SeriesDisposition {
    Matched,
    /// More than one series matched at the same step. Never guessed.
    Ambiguous,
    /// No series matched, or the only ones that did are not visible to the
    /// importing user.
    Unmatched,
}

/// Why a book in the import file could not be resolved to exactly one book
/// in its matched series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BookDisposition {
    Matched,
    /// Matched only by filename stem (e.g. a `.cbr` repacked to `.cbz`).
    /// Applied only when `acceptStemMatches` is set.
    StemMatch,
    Ambiguous,
    Unmatched,
    /// A single candidate was found by path or name, but its `file_hash`
    /// disagreed with the export under `hashMode = "verify"` or `"match"`.
    HashMismatch,
}

/// What happened to one scalar field write (progress or rating) under the
/// active conflict policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FieldOutcome {
    Inserted,
    Updated,
    /// An existing row won the conflict policy and was left as-is.
    Skipped,
}

/// Insert/reattach/skip counts for an append-only table (completions or
/// sessions) within one book.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WriteCounts {
    pub inserted: u32,
    /// Adopted an orphaned row (`book_id IS NULL`) rather than inserting a
    /// new one.
    pub reattached: u32,
    /// Already present with the same book attached; re-importing is a no-op.
    pub skipped: u32,
}

/// The outcome for one book in the import file.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportBookReport {
    pub path: String,
    pub file_name: String,
    pub disposition: BookDisposition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_book_id: Option<Uuid>,
    /// Whether any writes were attempted for this book. False for every
    /// disposition except `matched`, and except `stem_match` when
    /// `acceptStemMatches` is off.
    pub applied: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<FieldOutcome>,
    pub completions: WriteCounts,
    pub sessions: WriteCounts,
}

/// The outcome for one series in the import file.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportSeriesReport {
    pub library_relative_path: String,
    pub name: String,
    pub disposition: SeriesDisposition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_series_id: Option<Uuid>,
    /// Whether book/rating processing was attempted at all (only when
    /// `disposition == matched`).
    pub attempted: bool,
    /// Whether this series' transaction was committed. Always `false` in a
    /// dry run, and `false` if `attempted` but a write failed.
    pub committed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<FieldOutcome>,
    pub books: Vec<ImportBookReport>,
}

/// Totals across every series in the file, for a one-line summary.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub series_total: u32,
    pub series_matched: u32,
    pub series_ambiguous: u32,
    pub series_unmatched: u32,
    pub series_committed: u32,
    pub books_total: u32,
    pub books_matched: u32,
    pub books_stem_matched: u32,
    pub books_ambiguous: u32,
    pub books_unmatched: u32,
    pub books_hash_mismatch: u32,
    pub progress_written: u32,
    pub ratings_written: u32,
    pub completions_inserted: u32,
    pub completions_reattached: u32,
    pub sessions_inserted: u32,
    pub sessions_reattached: u32,
}

/// The response for both a real import and a dry run: the shape is identical
/// either way, so a client cannot tell from the response alone whether
/// anything was written. Only `dryRun` (and the DB) says that.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportReadingProgressResponse {
    pub dry_run: bool,
    pub sessions_in_file: bool,
    /// Informational notes about the request, e.g. `reattachSessions` having
    /// no effect because the file carries no sessions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    pub summary: ImportSummary,
    pub series: Vec<ImportSeriesReport>,
}
