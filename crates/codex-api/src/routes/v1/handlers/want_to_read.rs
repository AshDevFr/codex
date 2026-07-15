//! Handlers for the per-user want-to-read queue.
//!
//! The queue is personal: every handler scopes to `auth.user_id`. Being
//! authenticated is sufficient (no extra permission) — a user only ever manages
//! their own queue.

use super::super::dto::{
    AddWantToReadRequest, BulkAddWantToReadRequest, BulkAddWantToReadResponse,
    ReorderWantToReadRequest, WantToReadEntryDto, WantToReadItemType, WantToReadListQuery,
    WantToReadListResponse,
};
use crate::{AppState, error::ApiError, extractors::AuthContext};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use codex_db::repositories::{BookRepository, SeriesRepository, WantToReadRepository};
use std::sync::Arc;
use utoipa::OpenApi;
use uuid::Uuid;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_want_to_read,
        add_want_to_read,
        bulk_add_want_to_read,
        reorder_want_to_read,
        remove_want_to_read_series,
        remove_want_to_read_book,
    ),
    components(schemas(
        WantToReadEntryDto,
        WantToReadListResponse,
        AddWantToReadRequest,
        BulkAddWantToReadRequest,
        BulkAddWantToReadResponse,
        ReorderWantToReadRequest,
        WantToReadItemType,
    )),
    tags(
        (name = "Want to Read", description = "Per-user want-to-read queue endpoints")
    )
)]
#[allow(dead_code)] // OpenAPI documentation struct - referenced by utoipa derive macros
pub struct WantToReadApi;

/// List the authenticated user's want-to-read queue.
#[utoipa::path(
    get,
    path = "/api/v1/want-to-read",
    params(WantToReadListQuery),
    responses(
        (status = 200, description = "Queue retrieved", body = WantToReadListResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Want to Read"
)]
pub async fn list_want_to_read(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Query(query): Query<WantToReadListQuery>,
) -> Result<Json<WantToReadListResponse>, ApiError> {
    let entries = WantToReadRepository::list(&state.db, auth.user_id, query.order())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list want-to-read queue: {e}")))?;

    let total = entries.len();
    let items = entries.into_iter().map(WantToReadEntryDto::from).collect();
    Ok(Json(WantToReadListResponse { items, total }))
}

/// Add a series or book to the authenticated user's queue.
///
/// Exactly one of `seriesId` / `bookId` must be provided. Idempotent: flagging
/// something already queued returns the existing entry.
#[utoipa::path(
    post,
    path = "/api/v1/want-to-read",
    request_body = AddWantToReadRequest,
    responses(
        (status = 201, description = "Entry added (or already present)", body = WantToReadEntryDto),
        (status = 400, description = "Must provide exactly one of seriesId / bookId"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series or book not found"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Want to Read"
)]
pub async fn add_want_to_read(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<AddWantToReadRequest>,
) -> Result<(StatusCode, Json<WantToReadEntryDto>), ApiError> {
    let entry = match (request.series_id, request.book_id) {
        (Some(series_id), None) => {
            SeriesRepository::get_by_id(&state.db, series_id)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to look up series: {e}")))?
                .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;
            WantToReadRepository::add_series(&state.db, auth.user_id, series_id)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to add series: {e}")))?
        }
        (None, Some(book_id)) => {
            BookRepository::get_by_id(&state.db, book_id)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to look up book: {e}")))?
                .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;
            WantToReadRepository::add_book(&state.db, auth.user_id, book_id)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to add book: {e}")))?
        }
        _ => {
            return Err(ApiError::BadRequest(
                "Exactly one of seriesId or bookId must be provided".to_string(),
            ));
        }
    };

    Ok((StatusCode::CREATED, Json(entry.into())))
}

/// Add many series and/or books to the authenticated user's queue in one call.
///
/// Idempotent and tolerant: items already queued are reported as
/// `alreadyPresent`, and unknown IDs are silently skipped (a bulk grid
/// selection shouldn't fail wholesale because one item was deleted
/// concurrently). Returns the counts so the caller can surface an accurate
/// toast.
#[utoipa::path(
    post,
    path = "/api/v1/want-to-read/bulk",
    request_body = BulkAddWantToReadRequest,
    responses(
        (status = 200, description = "Entries added", body = BulkAddWantToReadResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Want to Read"
)]
pub async fn bulk_add_want_to_read(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<BulkAddWantToReadRequest>,
) -> Result<Json<BulkAddWantToReadResponse>, ApiError> {
    let mut added = 0;
    let mut already_present = 0;

    if !request.series_ids.is_empty() {
        // Filter to series that actually exist so phantom IDs don't violate the
        // foreign key and fail the batch. Dedup so the "already present" count
        // is honest.
        let existing = SeriesRepository::get_existing_ids(&state.db, &request.series_ids)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to look up series: {e}")))?;
        let mut seen = std::collections::HashSet::new();
        let valid: Vec<Uuid> = request
            .series_ids
            .iter()
            .copied()
            .filter(|id| existing.contains(id) && seen.insert(*id))
            .collect();
        let inserted = WantToReadRepository::add_series_bulk(&state.db, auth.user_id, &valid)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to add series: {e}")))?;
        added += inserted;
        already_present += valid.len() - inserted;
    }

    if !request.book_ids.is_empty() {
        let existing = BookRepository::get_existing_ids(&state.db, &request.book_ids)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to look up books: {e}")))?;
        let mut seen = std::collections::HashSet::new();
        let valid: Vec<Uuid> = request
            .book_ids
            .iter()
            .copied()
            .filter(|id| existing.contains(id) && seen.insert(*id))
            .collect();
        let inserted = WantToReadRepository::add_books_bulk(&state.db, auth.user_id, &valid)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to add books: {e}")))?;
        added += inserted;
        already_present += valid.len() - inserted;
    }

    Ok(Json(BulkAddWantToReadResponse {
        added,
        already_present,
    }))
}

/// Set the manual (`custom`) order of the authenticated user's queue.
///
/// Positions are assigned by index of `entryIds`; entries not listed keep
/// their old positions and unknown IDs are ignored. The order is visible via
/// `GET /want-to-read?sort=custom`.
#[utoipa::path(
    put,
    path = "/api/v1/want-to-read/order",
    request_body = ReorderWantToReadRequest,
    responses(
        (status = 204, description = "Order updated"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Want to Read"
)]
pub async fn reorder_want_to_read(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Json(request): Json<ReorderWantToReadRequest>,
) -> Result<StatusCode, ApiError> {
    WantToReadRepository::reorder(&state.db, auth.user_id, &request.entry_ids)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to reorder want-to-read queue: {e}")))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Remove a series from the authenticated user's queue.
#[utoipa::path(
    delete,
    path = "/api/v1/want-to-read/series/{series_id}",
    responses(
        (status = 204, description = "Removed (or was not present)"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Want to Read"
)]
pub async fn remove_want_to_read_series(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    WantToReadRepository::remove_series(&state.db, auth.user_id, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to remove series: {e}")))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Remove a book from the authenticated user's queue.
#[utoipa::path(
    delete,
    path = "/api/v1/want-to-read/books/{book_id}",
    responses(
        (status = 204, description = "Removed (or was not present)"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("bearer_auth" = []),
        ("api_key" = [])
    ),
    tag = "Want to Read"
)]
pub async fn remove_want_to_read_book(
    State(state): State<Arc<AppState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    WantToReadRepository::remove_book(&state.db, auth.user_id, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to remove book: {e}")))?;
    Ok(StatusCode::NO_CONTENT)
}
