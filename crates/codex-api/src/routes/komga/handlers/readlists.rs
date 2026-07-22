//! Komga-compatible read list endpoints (read-only).
//!
//! Backs the `KomgaReadListDto` shape third-party Komga apps expect, sourced
//! from real Codex read lists. Member books are filtered through the requesting
//! user's sharing-tag visibility.
//!
//! A virtual, per-user "Want to Read" read list (sentinel ID `want-to-read`)
//! is prepended to the list and exposes the book entries of the user's
//! want-to-read queue; the queue's series entries are exposed through the
//! matching virtual collection (see the collections handler).

use super::super::dto::book::KomgaBookDto;
use super::super::dto::pagination::KomgaPage;
use super::super::dto::stubs::{KomgaReadListDto, StubPaginationQuery};
use super::books::get_series_title;
use super::collections::{WANT_TO_READ_ID, WANT_TO_READ_NAME, queue_dates};
use crate::require_permission;
use crate::{
    error::ApiError,
    extractors::{AuthState, ContentFilter, FlexibleAuthContext},
    permissions::Permission,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    response::Redirect,
};
use codex_db::repositories::{
    BookMetadataRepository, ReadListRepository, ReadProgressRepository, WantToReadRepository,
    visibility::SeriesVisibility,
};
use codex_models::sort::{SortDirection, WantToReadSort};
use std::sync::Arc;
use uuid::Uuid;

fn parse_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(|_| ApiError::NotFound("Read list not found".to_string()))
}

async fn user_visibility(
    state: &AuthState,
    user_id: Uuid,
) -> Result<Option<SeriesVisibility>, ApiError> {
    let filter = ContentFilter::for_user(&state.db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {e}")))?;
    Ok(filter.to_visibility())
}

async fn build_want_to_read_dto(
    state: &AuthState,
    user_id: Uuid,
    vis: Option<&SeriesVisibility>,
) -> Result<KomgaReadListDto, ApiError> {
    let entries = WantToReadRepository::list(&state.db, user_id, WantToReadSort::Custom)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch want-to-read queue: {e}")))?;
    let members = WantToReadRepository::queued_books(&state.db, user_id, vis)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch want-to-read books: {e}")))?;
    let (created_date, last_modified_date) = queue_dates(&entries);
    Ok(KomgaReadListDto {
        id: WANT_TO_READ_ID.to_string(),
        name: WANT_TO_READ_NAME.to_string(),
        summary: "Books flagged as want to read".to_string(),
        ordered: true,
        book_ids: members.iter().map(|b| b.id.to_string()).collect(),
        created_date,
        last_modified_date,
        filtered: false,
    })
}

async fn build_readlist_dto(
    state: &AuthState,
    model: codex_db::entities::read_lists::Model,
    vis: Option<&SeriesVisibility>,
) -> Result<KomgaReadListDto, ApiError> {
    let members =
        ReadListRepository::get_books(&state.db, &model, vis, None, SortDirection::default())
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch read list books: {e}")))?;
    Ok(KomgaReadListDto {
        id: model.id.to_string(),
        name: model.name,
        summary: model.summary.unwrap_or_default(),
        ordered: model.ordered,
        book_ids: members.iter().map(|b| b.id.to_string()).collect(),
        created_date: model.created_at.to_rfc3339(),
        last_modified_date: model.updated_at.to_rfc3339(),
        filtered: false,
    })
}

/// List read lists (Komga-compatible).
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/readlists",
    responses((status = 200, body = KomgaPage<KomgaReadListDto>), (status = 401)),
    params(("prefix" = String, Path, description = "Komga API prefix")),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Komga"
)]
pub async fn list_readlists(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<StubPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaReadListDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;
    let vis = user_visibility(&state, auth.user_id).await?;

    let read_lists = ReadListRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list read lists: {e}")))?;
    // The virtual want-to-read read list is prepended, so it counts toward
    // the total and shifts the real read lists' page offsets by one.
    let total = read_lists.len() as i64 + 1;

    let page = query.page.max(0);
    let size = query.size.clamp(1, 500);
    let mut content = Vec::new();
    let (start, take) = if page == 0 {
        content.push(build_want_to_read_dto(&state, auth.user_id, vis.as_ref()).await?);
        (0, size as usize - 1)
    } else {
        (
            (page as usize).saturating_mul(size as usize) - 1,
            size as usize,
        )
    };
    let page_models: Vec<_> = read_lists.into_iter().skip(start).take(take).collect();

    for model in page_models {
        content.push(build_readlist_dto(&state, model, vis.as_ref()).await?);
    }
    Ok(Json(KomgaPage::new(content, page, size, total)))
}

/// Get a read list (Komga-compatible).
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/readlists/{read_list_id}",
    responses((status = 200, body = KomgaReadListDto), (status = 404)),
    params(("prefix" = String, Path, description = "Komga API prefix"), ("read_list_id" = String, Path)),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Komga"
)]
pub async fn get_readlist(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(read_list_id): Path<String>,
) -> Result<Json<KomgaReadListDto>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;
    let vis = user_visibility(&state, auth.user_id).await?;
    if read_list_id == WANT_TO_READ_ID {
        return Ok(Json(
            build_want_to_read_dto(&state, auth.user_id, vis.as_ref()).await?,
        ));
    }
    let id = parse_id(&read_list_id)?;
    let model = ReadListRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch read list: {e}")))?
        .ok_or_else(|| ApiError::NotFound("Read list not found".to_string()))?;
    Ok(Json(build_readlist_dto(&state, model, vis.as_ref()).await?))
}

/// Get the books in a read list (Komga-compatible).
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/readlists/{read_list_id}/books",
    responses((status = 200, body = KomgaPage<KomgaBookDto>), (status = 404)),
    params(("prefix" = String, Path, description = "Komga API prefix"), ("read_list_id" = String, Path)),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Komga"
)]
pub async fn get_readlist_books(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(read_list_id): Path<String>,
    Query(query): Query<StubPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaBookDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;
    let vis = user_visibility(&state, auth.user_id).await?;
    let members = if read_list_id == WANT_TO_READ_ID {
        WantToReadRepository::queued_books(&state.db, auth.user_id, vis.as_ref())
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch want-to-read books: {e}")))?
    } else {
        let id = parse_id(&read_list_id)?;
        let model = ReadListRepository::get_by_id(&state.db, id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch read list: {e}")))?
            .ok_or_else(|| ApiError::NotFound("Read list not found".to_string()))?;
        ReadListRepository::get_books(
            &state.db,
            &model,
            vis.as_ref(),
            None,
            SortDirection::default(),
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch read list books: {e}")))?
    };
    let total = members.len() as i64;

    let page = query.page.max(0);
    let size = query.size.clamp(1, 500);
    let start = (page as usize).saturating_mul(size as usize);
    let page_members: Vec<_> = members
        .into_iter()
        .skip(start)
        .take(size as usize)
        .collect();

    let book_ids: Vec<Uuid> = page_members.iter().map(|b| b.id).collect();
    let metadata_map = BookMetadataRepository::get_by_book_ids(&state.db, &book_ids)
        .await
        .unwrap_or_default();
    let progress_map =
        ReadProgressRepository::get_for_user_books(&state.db, auth.user_id, &book_ids)
            .await
            .unwrap_or_default();

    let mut content = Vec::with_capacity(page_members.len());
    for book in page_members {
        let series_title = get_series_title(&state, book.series_id).await?;
        let meta = metadata_map.get(&book.id);
        let book_number = meta
            .and_then(|m| m.number)
            .map(|d| d.to_string().parse::<i32>().unwrap_or(1))
            .unwrap_or(1);
        let progress = progress_map.get(&book.id);
        content.push(KomgaBookDto::from_codex_with_metadata(
            &book,
            &series_title,
            book_number,
            progress,
            meta,
        ));
    }
    Ok(Json(KomgaPage::new(content, page, size, total)))
}

/// Get a read list's thumbnail (redirects to the first visible member book).
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/readlists/{read_list_id}/thumbnail",
    responses((status = 307), (status = 404)),
    params(("prefix" = String, Path, description = "Komga API prefix"), ("read_list_id" = String, Path)),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Komga"
)]
pub async fn get_readlist_thumbnail(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(read_list_id): Path<String>,
) -> Result<Redirect, ApiError> {
    auth.require_permission(&Permission::BooksRead)?;
    let vis = user_visibility(&state, auth.user_id).await?;
    let members = if read_list_id == WANT_TO_READ_ID {
        WantToReadRepository::queued_books(&state.db, auth.user_id, vis.as_ref())
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch want-to-read books: {e}")))?
    } else {
        let id = parse_id(&read_list_id)?;
        let model = ReadListRepository::get_by_id(&state.db, id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch read list: {e}")))?
            .ok_or_else(|| ApiError::NotFound("Read list not found".to_string()))?;
        ReadListRepository::get_books(
            &state.db,
            &model,
            vis.as_ref(),
            None,
            SortDirection::default(),
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch read list books: {e}")))?
    };
    let first = members
        .first()
        .ok_or_else(|| ApiError::NotFound("Read list has no visible books".to_string()))?;
    // Cache-bust with the member's update time so browsers refetch the image
    // after its cover is regenerated (the target URL is otherwise cached
    // indefinitely; the card grids bust their own image URLs the same way).
    Ok(Redirect::temporary(&format!(
        "/api/v1/books/{}/thumbnail?v={}",
        first.id,
        first.updated_at.timestamp_millis()
    )))
}

/// List the read lists that contain a book (Komga-compatible).
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/books/{book_id}/readlists",
    responses((status = 200, body = Vec<KomgaReadListDto>)),
    params(("prefix" = String, Path, description = "Komga API prefix"), ("book_id" = String, Path)),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Komga"
)]
pub async fn get_book_readlists(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(book_id): Path<String>,
) -> Result<Json<Vec<KomgaReadListDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;
    let bid =
        Uuid::parse_str(&book_id).map_err(|_| ApiError::NotFound("Book not found".to_string()))?;
    let vis = user_visibility(&state, auth.user_id).await?;

    let read_lists = ReadListRepository::get_read_lists_for_book(&state.db, bid)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch read lists: {e}")))?;
    let mut out = Vec::with_capacity(read_lists.len() + 1);
    // Queued books also belong to the virtual want-to-read read list, so
    // clients cross-referencing membership stay consistent with the list view.
    if WantToReadRepository::is_book_in_queue(&state.db, auth.user_id, bid)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check want-to-read queue: {e}")))?
    {
        out.push(build_want_to_read_dto(&state, auth.user_id, vis.as_ref()).await?);
    }
    for model in read_lists {
        out.push(build_readlist_dto(&state, model, vis.as_ref()).await?);
    }
    Ok(Json(out))
}
