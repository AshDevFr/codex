//! Batched reading-session recording.
//!
//! The endpoint a client replays its offline outbox against. One request rather
//! than one per session, because a day spent reading on a plane produces a queue
//! rather than a single write.

use super::super::dto::{
    ReadProgressResponse, ReadingSessionDto, ReadingSessionRejectionReason,
    RecordReadingSessionsRequest, RecordReadingSessionsResponse, RejectedReadingSessionDto,
};
use crate::{AppState, error::ApiError, extractors::AuthContext, permissions::Permission};
use axum::{Json, extract::State};
use codex_db::repositories::{BookRepository, NewSession, ReadProgressRepository};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use utoipa::OpenApi;
use uuid::Uuid;

/// Most sessions one request may carry.
///
/// A client with a genuinely enormous backlog chunks rather than sending it all
/// at once, which keeps one request from holding a transaction open for an
/// unbounded time.
const MAX_BATCH_SIZE: usize = 500;

#[derive(OpenApi)]
#[openapi(
    paths(record_reading_sessions),
    components(schemas(
        RecordReadingSessionsRequest,
        RecordReadingSessionsResponse,
        ReadingSessionDto,
        RejectedReadingSessionDto,
        ReadingSessionRejectionReason,
    )),
    tags(
        (name = "Reading Sessions", description = "Append-only reading activity log")
    )
)]
#[allow(dead_code)] // OpenAPI documentation struct - referenced by utoipa derive macros
pub struct ReadingSessionsApi;

/// Record a batch of reading sessions
///
/// Sessions are the source of truth behind reading progress and reading time.
/// A client measures them locally, queues them while offline, and replays the
/// queue here.
///
/// Two properties make replay safe. Submitting an id that is already present
/// changes nothing and still reports as accepted, so a batch whose response was
/// lost can be sent again. And sessions are ordered by `clientEndedAt` rather
/// than by arrival, so a session read earlier but synced later does not
/// overwrite one that was read more recently.
///
/// Sessions are always recorded against the authenticated user; there is no way
/// to write another user's reading.
#[utoipa::path(
    post,
    path = "/api/v1/reading-sessions",
    request_body = RecordReadingSessionsRequest,
    responses(
        (status = 200, description = "Batch processed; check `rejected` for entries that were not recorded", body = RecordReadingSessionsResponse),
        (status = 400, description = "Batch exceeds the maximum size"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Reading Sessions"
)]
pub async fn record_reading_sessions(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<RecordReadingSessionsRequest>,
) -> Result<Json<RecordReadingSessionsResponse>, ApiError> {
    auth.require_permission(&Permission::BooksRead)?;

    if request.sessions.len() > MAX_BATCH_SIZE {
        return Err(ApiError::BadRequest(format!(
            "batch of {} sessions exceeds the maximum of {MAX_BATCH_SIZE}; send it in chunks",
            request.sessions.len()
        )));
    }

    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let mut touched_books = Vec::new();
    let mut seen_ids = HashSet::new();
    // Book lookups are cached across the batch: a queued day of reading is
    // usually a handful of books and many sessions.
    let mut known_books: HashMap<Uuid, bool> = HashMap::new();

    for session in request.sessions {
        if !seen_ids.insert(session.id) {
            rejected.push(RejectedReadingSessionDto {
                id: session.id,
                reason: ReadingSessionRejectionReason::DuplicateInBatch,
            });
            continue;
        }

        if let Some(reason) = validate(&session) {
            rejected.push(RejectedReadingSessionDto {
                id: session.id,
                reason,
            });
            continue;
        }

        let book_exists = match known_books.get(&session.book_id) {
            Some(exists) => *exists,
            None => {
                let exists = BookRepository::get_by_id(&state.db, session.book_id)
                    .await
                    .map_err(|e| ApiError::Internal(format!("Failed to get book: {}", e)))?
                    .is_some();
                known_books.insert(session.book_id, exists);
                exists
            }
        };

        if !book_exists {
            rejected.push(RejectedReadingSessionDto {
                id: session.id,
                reason: ReadingSessionRejectionReason::BookNotFound,
            });
            continue;
        }

        let book_id = session.book_id;
        let record = NewSession::from_client(
            session.id,
            auth.user_id,
            book_id,
            session.device_id,
            session.device_name,
            session.kind.into(),
            session.active_duration_ms,
            session.pages_read,
            session.client_started_at,
            session.client_ended_at,
        )
        .with_percentage(session.to_percentage)
        .with_progression(session.r2_progression);

        let record = match session.to_page {
            Some(page) => record.with_page(page),
            None => record,
        };

        let id = session.id;
        ReadProgressRepository::record_session(&state.db, record)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to record reading session: {}", e)))?;

        accepted.push(id);
        if !touched_books.contains(&book_id) {
            touched_books.push(book_id);
        }
    }

    // Read the projections back once at the end rather than per session: a
    // batch commonly holds several sessions for the same book, and only the
    // final state is useful to the client.
    let progress_rows =
        ReadProgressRepository::get_by_user_books(&state.db, auth.user_id, &touched_books)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to load reading progress: {}", e)))?;

    // Ordered to match the books as they were first touched, so the response is
    // stable rather than dependent on hash iteration order.
    let progress = touched_books
        .iter()
        .filter_map(|book_id| progress_rows.get(book_id).cloned())
        .map(ReadProgressResponse::from)
        .collect();

    Ok(Json(RecordReadingSessionsResponse {
        accepted,
        rejected,
        progress,
    }))
}

/// Reject entries a client could not have measured honestly.
///
/// Deliberately not exhaustive validation: the point is to catch values that
/// would corrupt reading statistics, not to police every field.
fn validate(session: &ReadingSessionDto) -> Option<ReadingSessionRejectionReason> {
    if session.client_ended_at < session.client_started_at {
        return Some(ReadingSessionRejectionReason::InvalidTimeRange);
    }

    if session
        .to_percentage
        .is_some_and(|p| !(0.0..=1.0).contains(&p) || p.is_nan())
    {
        return Some(ReadingSessionRejectionReason::InvalidPercentage);
    }

    if session.active_duration_ms.is_some_and(|ms| ms < 0)
        || session.pages_read.is_some_and(|pages| pages < 0)
    {
        return Some(ReadingSessionRejectionReason::InvalidMeasurement);
    }

    None
}
