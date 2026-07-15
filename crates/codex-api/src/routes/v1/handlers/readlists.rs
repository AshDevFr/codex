//! Handlers for read lists (shared, ordered groupings of books across series).
//!
//! Reads require `ReadListsRead` (granted to every role); create/modify require
//! `ReadListsWrite`; delete requires `ReadListsDelete` (write/delete are in the
//! Maintainer bundle). Member lists and counts are filtered through the
//! requesting user's sharing-tag visibility.

use super::super::dto::{
    AddBooksToReadListRequest, BookDto, CreateReadListRequest, ReadListBooksQuery, ReadListDto,
    ReadListListResponse, ReorderReadListBooksRequest, UpdateReadListRequest,
};
use crate::require_permission;
use crate::{
    error::ApiError,
    extractors::{AuthContext, AuthState, ContentFilter, FlexibleAuthContext},
    permissions::Permission,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Redirect,
};
use codex_db::entities::read_lists;
use codex_db::repositories::{BookRepository, ReadListRepository, visibility::SeriesVisibility};
use codex_models::sort::SortDirection;
use std::sync::Arc;
use utoipa::OpenApi;
use uuid::Uuid;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_readlists,
        create_readlist,
        get_readlist,
        update_readlist,
        delete_readlist,
        get_readlist_books,
        add_readlist_books,
        remove_readlist_book,
        reorder_readlist_books,
        get_readlist_thumbnail,
        get_book_readlists,
    ),
    components(schemas(
        ReadListDto,
        ReadListListResponse,
        CreateReadListRequest,
        UpdateReadListRequest,
        AddBooksToReadListRequest,
        ReorderReadListBooksRequest,
    )),
    tags(
        (name = "Read Lists", description = "Shared, ordered groupings of books across series")
    )
)]
#[allow(dead_code)] // OpenAPI documentation struct - referenced by utoipa derive macros
pub struct ReadListsApi;

fn internal<E: std::fmt::Display>(context: &str) -> impl Fn(E) -> ApiError + '_ {
    move |e| ApiError::Internal(format!("{context}: {e}"))
}

async fn readlist_dto(
    db: &sea_orm::DatabaseConnection,
    model: codex_db::entities::read_lists::Model,
    vis: Option<&SeriesVisibility>,
) -> Result<ReadListDto, ApiError> {
    let count = ReadListRepository::count_books(db, model.id, vis)
        .await
        .map_err(internal("Failed to count read list books"))?;
    Ok(ReadListDto::from_model(model, count))
}

async fn user_visibility(
    state: &AuthState,
    user_id: Uuid,
) -> Result<Option<SeriesVisibility>, ApiError> {
    let filter = ContentFilter::for_user(&state.db, user_id)
        .await
        .map_err(internal("Failed to load content filter"))?;
    Ok(filter.to_visibility())
}

async fn ensure_readlist_exists(state: &AuthState, read_list_id: Uuid) -> Result<(), ApiError> {
    get_readlist_or_404(state, read_list_id).await.map(|_| ())
}

async fn get_readlist_or_404(
    state: &AuthState,
    read_list_id: Uuid,
) -> Result<read_lists::Model, ApiError> {
    ReadListRepository::get_by_id(&state.db, read_list_id)
        .await
        .map_err(internal("Failed to fetch read list"))?
        .ok_or_else(|| ApiError::NotFound("Read list not found".to_string()))
}

/// List all read lists.
#[utoipa::path(
    get,
    path = "/api/v1/readlists",
    responses(
        (status = 200, description = "Read lists", body = ReadListListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Read Lists"
)]
pub async fn list_readlists(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<ReadListListResponse>, ApiError> {
    require_permission!(auth, Permission::ReadListsRead)?;
    let vis = user_visibility(&state, auth.user_id).await?;

    let read_lists = ReadListRepository::list_all(&state.db)
        .await
        .map_err(internal("Failed to list read lists"))?;

    let mut items = Vec::with_capacity(read_lists.len());
    for model in read_lists {
        items.push(readlist_dto(&state.db, model, vis.as_ref()).await?);
    }
    let total = items.len();
    Ok(Json(ReadListListResponse { items, total }))
}

/// Create a read list.
#[utoipa::path(
    post,
    path = "/api/v1/readlists",
    request_body = CreateReadListRequest,
    responses(
        (status = 201, description = "Created", body = ReadListDto),
        (status = 400, description = "Invalid name"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "A read list with that name already exists"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Read Lists"
)]
pub async fn create_readlist(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<CreateReadListRequest>,
) -> Result<(StatusCode, Json<ReadListDto>), ApiError> {
    require_permission!(auth, Permission::ReadListsWrite)?;

    let name = request.name.trim();
    if name.is_empty() {
        return Err(ApiError::BadRequest(
            "Read list name cannot be empty".to_string(),
        ));
    }
    if ReadListRepository::get_by_name(&state.db, name)
        .await
        .map_err(internal("Failed to check read list name"))?
        .is_some()
    {
        return Err(ApiError::Conflict(format!(
            "A read list named '{name}' already exists"
        )));
    }

    let model =
        ReadListRepository::create(&state.db, name, request.summary.as_deref(), request.ordered)
            .await
            .map_err(internal("Failed to create read list"))?;
    Ok((StatusCode::CREATED, Json(ReadListDto::from_model(model, 0))))
}

/// Get a read list.
#[utoipa::path(
    get,
    path = "/api/v1/readlists/{read_list_id}",
    responses(
        (status = 200, description = "Read list", body = ReadListDto),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Read Lists"
)]
pub async fn get_readlist(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(read_list_id): Path<Uuid>,
) -> Result<Json<ReadListDto>, ApiError> {
    require_permission!(auth, Permission::ReadListsRead)?;
    let model = ReadListRepository::get_by_id(&state.db, read_list_id)
        .await
        .map_err(internal("Failed to fetch read list"))?
        .ok_or_else(|| ApiError::NotFound("Read list not found".to_string()))?;
    let vis = user_visibility(&state, auth.user_id).await?;
    Ok(Json(readlist_dto(&state.db, model, vis.as_ref()).await?))
}

/// Update a read list (rename / edit summary / toggle ordered).
#[utoipa::path(
    patch,
    path = "/api/v1/readlists/{read_list_id}",
    request_body = UpdateReadListRequest,
    responses(
        (status = 200, description = "Updated", body = ReadListDto),
        (status = 400, description = "Invalid name"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
        (status = 409, description = "Name already in use"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Read Lists"
)]
pub async fn update_readlist(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(read_list_id): Path<Uuid>,
    Json(request): Json<UpdateReadListRequest>,
) -> Result<Json<ReadListDto>, ApiError> {
    require_permission!(auth, Permission::ReadListsWrite)?;

    if let Some(ref new_name) = request.name {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err(ApiError::BadRequest(
                "Read list name cannot be empty".to_string(),
            ));
        }
        if let Some(existing) = ReadListRepository::get_by_name(&state.db, trimmed)
            .await
            .map_err(internal("Failed to check read list name"))?
            && existing.id != read_list_id
        {
            return Err(ApiError::Conflict(format!(
                "A read list named '{trimmed}' already exists"
            )));
        }
    }

    let summary = request.summary.as_ref().map(|inner| inner.as_deref());
    let model = ReadListRepository::update(
        &state.db,
        read_list_id,
        request.name.as_deref().map(str::trim),
        summary,
        request.ordered,
    )
    .await
    .map_err(internal("Failed to update read list"))?
    .ok_or_else(|| ApiError::NotFound("Read list not found".to_string()))?;

    let vis = user_visibility(&state, auth.user_id).await?;
    Ok(Json(readlist_dto(&state.db, model, vis.as_ref()).await?))
}

/// Delete a read list.
#[utoipa::path(
    delete,
    path = "/api/v1/readlists/{read_list_id}",
    responses(
        (status = 204, description = "Deleted"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Read Lists"
)]
pub async fn delete_readlist(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(read_list_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::ReadListsDelete)?;
    let deleted = ReadListRepository::delete(&state.db, read_list_id)
        .await
        .map_err(internal("Failed to delete read list"))?;
    if !deleted {
        return Err(ApiError::NotFound("Read list not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Get the books in a read list (visibility-filtered).
///
/// An explicit `sort` always wins; otherwise the read list's `ordered` flag
/// picks the default (manual reading order when set, release date otherwise).
#[utoipa::path(
    get,
    path = "/api/v1/readlists/{read_list_id}/books",
    params(ReadListBooksQuery),
    responses(
        (status = 200, description = "Member books", body = [BookDto]),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Read Lists"
)]
pub async fn get_readlist_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(read_list_id): Path<Uuid>,
    Query(query): Query<ReadListBooksQuery>,
) -> Result<Json<Vec<BookDto>>, ApiError> {
    require_permission!(auth, Permission::ReadListsRead)?;
    let read_list = get_readlist_or_404(&state, read_list_id).await?;

    let vis = user_visibility(&state, auth.user_id).await?;
    let members = ReadListRepository::get_books(
        &state.db,
        &read_list,
        vis.as_ref(),
        query.sort,
        query.direction.unwrap_or_default(),
    )
    .await
    .map_err(internal("Failed to fetch read list books"))?;

    let dtos = super::books::books_to_dtos(&state.db, auth.user_id, members).await?;
    Ok(Json(dtos))
}

/// Add one or more books to a read list.
#[utoipa::path(
    post,
    path = "/api/v1/readlists/{read_list_id}/books",
    request_body = AddBooksToReadListRequest,
    responses(
        (status = 200, description = "Updated read list", body = ReadListDto),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Read list or book not found"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Read Lists"
)]
pub async fn add_readlist_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(read_list_id): Path<Uuid>,
    Json(request): Json<AddBooksToReadListRequest>,
) -> Result<Json<ReadListDto>, ApiError> {
    require_permission!(auth, Permission::ReadListsWrite)?;
    let model = ReadListRepository::get_by_id(&state.db, read_list_id)
        .await
        .map_err(internal("Failed to fetch read list"))?
        .ok_or_else(|| ApiError::NotFound("Read list not found".to_string()))?;

    for book_id in &request.book_ids {
        if BookRepository::get_by_id(&state.db, *book_id)
            .await
            .map_err(internal("Failed to look up book"))?
            .is_none()
        {
            return Err(ApiError::NotFound(format!("Book {book_id} not found")));
        }
        ReadListRepository::add_book(&state.db, read_list_id, *book_id)
            .await
            .map_err(internal("Failed to add book to read list"))?;
    }

    let vis = user_visibility(&state, auth.user_id).await?;
    Ok(Json(readlist_dto(&state.db, model, vis.as_ref()).await?))
}

/// Remove a book from a read list.
#[utoipa::path(
    delete,
    path = "/api/v1/readlists/{read_list_id}/books/{book_id}",
    responses(
        (status = 204, description = "Removed (or was not a member)"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Read Lists"
)]
pub async fn remove_readlist_book(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((read_list_id, book_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::ReadListsWrite)?;
    ReadListRepository::remove_book(&state.db, read_list_id, book_id)
        .await
        .map_err(internal("Failed to remove book from read list"))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Set the manual order of a read list's books.
#[utoipa::path(
    put,
    path = "/api/v1/readlists/{read_list_id}/books",
    request_body = ReorderReadListBooksRequest,
    responses(
        (status = 204, description = "Reordered"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Read Lists"
)]
pub async fn reorder_readlist_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(read_list_id): Path<Uuid>,
    Json(request): Json<ReorderReadListBooksRequest>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::ReadListsWrite)?;
    ensure_readlist_exists(&state, read_list_id).await?;
    ReadListRepository::reorder(&state.db, read_list_id, &request.book_ids)
        .await
        .map_err(internal("Failed to reorder read list books"))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Get a read list's thumbnail (the first visible member book's cover).
#[utoipa::path(
    get,
    path = "/api/v1/readlists/{read_list_id}/thumbnail",
    responses(
        (status = 307, description = "Redirect to the first member book thumbnail"),
        (status = 404, description = "No visible member books"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Read Lists"
)]
pub async fn get_readlist_thumbnail(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(read_list_id): Path<Uuid>,
) -> Result<Redirect, ApiError> {
    auth.require_permission(&Permission::ReadListsRead)?;
    let read_list = get_readlist_or_404(&state, read_list_id).await?;
    let vis = user_visibility(&state, auth.user_id).await?;
    let members = ReadListRepository::get_books(
        &state.db,
        &read_list,
        vis.as_ref(),
        None,
        SortDirection::default(),
    )
    .await
    .map_err(internal("Failed to fetch read list books"))?;
    let first = members
        .first()
        .ok_or_else(|| ApiError::NotFound("Read list has no visible books".to_string()))?;
    Ok(Redirect::temporary(&format!(
        "/api/v1/books/{}/thumbnail",
        first.id
    )))
}

/// List the read lists that contain a given book.
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/readlists",
    responses(
        (status = 200, description = "Read lists containing the book", body = ReadListListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer_auth" = []), ("api_key" = [])),
    tag = "Read Lists"
)]
pub async fn get_book_readlists(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<ReadListListResponse>, ApiError> {
    require_permission!(auth, Permission::ReadListsRead)?;
    let vis = user_visibility(&state, auth.user_id).await?;

    let read_lists = ReadListRepository::get_read_lists_for_book(&state.db, book_id)
        .await
        .map_err(internal("Failed to fetch read lists for book"))?;

    let mut items = Vec::with_capacity(read_lists.len());
    for model in read_lists {
        items.push(readlist_dto(&state.db, model, vis.as_ref()).await?);
    }
    let total = items.len();
    Ok(Json(ReadListListResponse { items, total }))
}
