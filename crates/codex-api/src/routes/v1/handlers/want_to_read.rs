//! Handlers for the per-user want-to-read queue.
//!
//! The queue is personal: every handler scopes to `auth.user_id`. Being
//! authenticated is sufficient (no extra permission) — a user only ever manages
//! their own queue.

use super::super::dto::{
    AddWantToReadRequest, WantToReadEntryDto, WantToReadItemType, WantToReadListQuery,
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
        remove_want_to_read_series,
        remove_want_to_read_book,
    ),
    components(schemas(
        WantToReadEntryDto,
        WantToReadListResponse,
        AddWantToReadRequest,
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
    let entries = WantToReadRepository::list(&state.db, auth.user_id, query.ascending())
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
