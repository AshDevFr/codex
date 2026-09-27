//! Export and import of one user's reading state, for carrying it across a
//! library reorganisation or a move to a different Codex instance.
//!
//! Export streams a JSON document naming the user's `read_progress`,
//! `read_completions`, `reading_sessions`, and `user_series_ratings` rows
//! against stable keys (external ids, a series-relative path, file name,
//! hashes) rather than database ids, which a rescan under a new root always
//! replaces. Import resolves those keys back to real ids in the current
//! library and writes the state under an explicit conflict policy, one
//! transaction per series, with a dry run that reports the identical shape
//! without writing anything.

use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use std::sync::Arc;

use super::super::dto::{
    ExportReadingProgressQuery, ImportReadingProgressRequest, ImportReadingProgressResponse,
};
use crate::{AppState, error::ApiError, extractors::AuthContext, permissions::Permission};
use codex_services::reading_transfer::import::{ImportError, ImportOptions};

/// Export the authenticated user's reading progress
///
/// Produces the whole `read_progress` / `read_completions` / `reading_sessions`
/// / `user_series_ratings` state for the caller, keyed by external ids and
/// series-relative paths instead of database ids so it can be matched back
/// against a differently organised library (or a different Codex instance
/// entirely) by `POST /api/v1/reading-progress/import`.
///
/// Take this export **before** deleting an old library during a split: the
/// underlying rows survive a hard delete as orphans, but nothing except this
/// file can say which book an orphan used to belong to.
#[utoipa::path(
    get,
    path = "/api/v1/reading-progress/export",
    params(
        ("include_sessions" = Option<bool>, Query, description = "Include the reading-session log (default: true). Sessions are the only source of every reading statistic, so this is opt-out rather than opt-in.")
    ),
    responses(
        (status = 200, description = "The export document", body = codex_services::reading_transfer::model::ReadingProgressExportDocument),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress Transfer"
)]
pub async fn export_reading_progress(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Query(query): Query<ExportReadingProgressQuery>,
) -> Result<Response, ApiError> {
    auth.require_permission(&Permission::ProgressRead)?;

    let document =
        codex_services::export_reading_progress(&state.db, auth.user_id, query.include_sessions)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to export reading progress: {e}")))?;

    let filename = format!(
        "codex-reading-progress-{}.json",
        Utc::now().format("%Y-%m-%d")
    );
    let disposition = crate::ranged_file::content_disposition_attachment(&filename);

    let mut response = Json(document).into_response();
    if let Ok(value) = HeaderValue::from_str(&disposition) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    Ok(response)
}

/// Import reading progress from an export document
///
/// Resolves the file's series and books against the current library through
/// the same access-group / sharing-tag visibility that an ordinary read
/// respects: a book the importing user cannot see resolves as `unmatched`,
/// never as a permission error that would confirm it exists, and nothing is
/// ever written against it.
///
/// Matching never guesses: any step (external id, path, file name, or, under
/// `hash_mode = "match"`, hash) that finds more than one candidate reports
/// `ambiguous` and writes nothing for that series or book. `dry_run: true`
/// returns the identical response shape without writing anything, which is
/// what makes it safe to preview before committing.
///
/// Each series is applied in its own transaction; the response's per-series
/// `committed` field says which ones actually landed. `read_completions` and
/// `reading_sessions` reuse their exported ids on insert, so importing the
/// same file twice leaves row counts unchanged rather than duplicating
/// history.
#[utoipa::path(
    post,
    path = "/api/v1/reading-progress/import",
    request_body = ImportReadingProgressRequest,
    responses(
        (status = 200, description = "Import processed (or, for a dry run, previewed)", body = ImportReadingProgressResponse),
        (status = 400, description = "Unknown export format, a version newer than this server supports, or a value the normal write paths reject"),
        (status = 413, description = "The file is larger than the import limit"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Reading Progress Transfer"
)]
pub async fn import_reading_progress(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<ImportReadingProgressRequest>,
) -> Result<Json<ImportReadingProgressResponse>, ApiError> {
    auth.require_permission(&Permission::ProgressWrite)?;

    let options = ImportOptions {
        dry_run: request.dry_run,
        hash_mode: request.hash_mode,
        source_preference: request.source_preference,
        conflict_policy: request.conflict_policy,
        reattach_sessions: request.reattach_sessions,
        accept_stem_matches: request.accept_stem_matches,
    };

    let response =
        codex_services::import_reading_progress(&state.db, auth.user_id, &request.file, &options)
            .await
            .map_err(|err| match err {
                ImportError::UnknownFormat(_)
                | ImportError::UnsupportedVersion(_)
                | ImportError::InvalidValue(_) => ApiError::BadRequest(err.to_string()),
                ImportError::Database(source) => {
                    ApiError::Internal(format!("Failed to import reading progress: {source}"))
                }
            })?;

    Ok(Json(response))
}
