//! Komga-compatible collection endpoints (read-only).
//!
//! Backs the `KomgaCollectionDto` shape third-party Komga apps expect, sourced
//! from real Codex collections. Member series are filtered through the
//! requesting user's sharing-tag visibility.
//!
//! A virtual, per-user "Want to Read" collection (sentinel ID `want-to-read`)
//! is prepended to the list and exposes the series entries of the user's
//! want-to-read queue, so Komga clients can browse the queue even though
//! Komga itself has no such feature. Book entries are exposed through the
//! matching virtual read list instead (see the readlists handler).

use super::super::dto::pagination::KomgaPage;
use super::super::dto::series::KomgaSeriesDto;
use super::super::dto::stubs::{KomgaCollectionDto, StubPaginationQuery};
use super::series::build_series_dto;
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
    CollectionRepository, WantToReadRepository, visibility::SeriesVisibility,
};
use codex_models::sort::{SortDirection, WantToReadSort};
use std::sync::Arc;
use uuid::Uuid;

/// Sentinel ID of the virtual per-user want-to-read collection. Komga clients
/// treat collection IDs as opaque strings, so a non-UUID value is safe.
pub(crate) const WANT_TO_READ_ID: &str = "want-to-read";
pub(crate) const WANT_TO_READ_NAME: &str = "Want to Read";

fn parse_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(|_| ApiError::NotFound("Collection not found".to_string()))
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

/// Created/modified dates for a virtual queue-backed DTO: first and last
/// `added_at` of the user's queue entries, or "now" for an empty queue.
pub(crate) fn queue_dates(entries: &[codex_db::entities::want_to_read::Model]) -> (String, String) {
    let created = entries.iter().map(|e| e.added_at).min();
    let modified = entries.iter().map(|e| e.added_at).max();
    let now = chrono::Utc::now();
    (
        created.unwrap_or(now).to_rfc3339(),
        modified.unwrap_or(now).to_rfc3339(),
    )
}

async fn build_want_to_read_dto(
    state: &AuthState,
    user_id: Uuid,
    vis: Option<&SeriesVisibility>,
) -> Result<KomgaCollectionDto, ApiError> {
    let entries = WantToReadRepository::list(&state.db, user_id, WantToReadSort::Custom)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch want-to-read queue: {e}")))?;
    let members = WantToReadRepository::queued_series(&state.db, user_id, vis)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch want-to-read series: {e}")))?;
    let (created_date, last_modified_date) = queue_dates(&entries);
    Ok(KomgaCollectionDto {
        id: WANT_TO_READ_ID.to_string(),
        name: WANT_TO_READ_NAME.to_string(),
        ordered: true,
        series_ids: members.iter().map(|s| s.id.to_string()).collect(),
        created_date,
        last_modified_date,
        filtered: false,
    })
}

async fn build_collection_dto(
    state: &AuthState,
    model: codex_db::entities::collections::Model,
    vis: Option<&SeriesVisibility>,
) -> Result<KomgaCollectionDto, ApiError> {
    let members =
        CollectionRepository::get_series(&state.db, &model, vis, None, SortDirection::default())
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch collection series: {e}")))?;
    Ok(KomgaCollectionDto {
        id: model.id.to_string(),
        name: model.name,
        ordered: model.ordered,
        series_ids: members.iter().map(|s| s.id.to_string()).collect(),
        created_date: model.created_at.to_rfc3339(),
        last_modified_date: model.updated_at.to_rfc3339(),
        filtered: false,
    })
}

/// List collections (Komga-compatible).
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/collections",
    responses((status = 200, body = KomgaPage<KomgaCollectionDto>), (status = 401)),
    params(("prefix" = String, Path, description = "Komga API prefix")),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Komga"
)]
pub async fn list_collections(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<StubPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaCollectionDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;
    let vis = user_visibility(&state, auth.user_id).await?;

    let collections = CollectionRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list collections: {e}")))?;
    // The virtual want-to-read collection is prepended, so it counts toward
    // the total and shifts the real collections' page offsets by one.
    let total = collections.len() as i64 + 1;

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
    let page_models: Vec<_> = collections.into_iter().skip(start).take(take).collect();

    for model in page_models {
        content.push(build_collection_dto(&state, model, vis.as_ref()).await?);
    }
    Ok(Json(KomgaPage::new(content, page, size, total)))
}

/// Get a collection (Komga-compatible).
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/collections/{collection_id}",
    responses((status = 200, body = KomgaCollectionDto), (status = 404)),
    params(("prefix" = String, Path, description = "Komga API prefix"), ("collection_id" = String, Path)),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Komga"
)]
pub async fn get_collection(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(collection_id): Path<String>,
) -> Result<Json<KomgaCollectionDto>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;
    let vis = user_visibility(&state, auth.user_id).await?;
    if collection_id == WANT_TO_READ_ID {
        return Ok(Json(
            build_want_to_read_dto(&state, auth.user_id, vis.as_ref()).await?,
        ));
    }
    let id = parse_id(&collection_id)?;
    let model = CollectionRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch collection: {e}")))?
        .ok_or_else(|| ApiError::NotFound("Collection not found".to_string()))?;
    Ok(Json(
        build_collection_dto(&state, model, vis.as_ref()).await?,
    ))
}

/// Get the series in a collection (Komga-compatible).
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/collections/{collection_id}/series",
    responses((status = 200, body = KomgaPage<KomgaSeriesDto>), (status = 404)),
    params(("prefix" = String, Path, description = "Komga API prefix"), ("collection_id" = String, Path)),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Komga"
)]
pub async fn get_collection_series(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(collection_id): Path<String>,
    Query(query): Query<StubPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaSeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;
    let vis = user_visibility(&state, auth.user_id).await?;
    let members = if collection_id == WANT_TO_READ_ID {
        WantToReadRepository::queued_series(&state.db, auth.user_id, vis.as_ref())
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch want-to-read series: {e}")))?
    } else {
        let id = parse_id(&collection_id)?;
        let model = CollectionRepository::get_by_id(&state.db, id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch collection: {e}")))?
            .ok_or_else(|| ApiError::NotFound("Collection not found".to_string()))?;
        CollectionRepository::get_series(
            &state.db,
            &model,
            vis.as_ref(),
            None,
            SortDirection::default(),
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch collection series: {e}")))?
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

    let mut content = Vec::with_capacity(page_members.len());
    for series in page_members {
        content.push(build_series_dto(&state, &series, Some(auth.user_id)).await?);
    }
    Ok(Json(KomgaPage::new(content, page, size, total)))
}

/// Get a collection's thumbnail (redirects to the first visible member series).
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/collections/{collection_id}/thumbnail",
    responses((status = 307), (status = 404)),
    params(("prefix" = String, Path, description = "Komga API prefix"), ("collection_id" = String, Path)),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Komga"
)]
pub async fn get_collection_thumbnail(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(collection_id): Path<String>,
) -> Result<Redirect, ApiError> {
    auth.require_permission(&Permission::SeriesRead)?;
    let vis = user_visibility(&state, auth.user_id).await?;
    let members = if collection_id == WANT_TO_READ_ID {
        WantToReadRepository::queued_series(&state.db, auth.user_id, vis.as_ref())
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch want-to-read series: {e}")))?
    } else {
        let id = parse_id(&collection_id)?;
        let model = CollectionRepository::get_by_id(&state.db, id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch collection: {e}")))?
            .ok_or_else(|| ApiError::NotFound("Collection not found".to_string()))?;
        CollectionRepository::get_series(
            &state.db,
            &model,
            vis.as_ref(),
            None,
            SortDirection::default(),
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch collection series: {e}")))?
    };
    let first = members
        .first()
        .ok_or_else(|| ApiError::NotFound("Collection has no visible series".to_string()))?;
    // Cache-bust with the member's update time so browsers refetch the image
    // after its cover is regenerated (the target URL is otherwise cached
    // indefinitely; the card grids bust their own image URLs the same way).
    Ok(Redirect::temporary(&format!(
        "/api/v1/series/{}/thumbnail?v={}",
        first.id,
        first.updated_at.timestamp_millis()
    )))
}

/// List the collections that contain a series (Komga-compatible).
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/series/{series_id}/collections",
    responses((status = 200, body = Vec<KomgaCollectionDto>)),
    params(("prefix" = String, Path, description = "Komga API prefix"), ("series_id" = String, Path)),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Komga"
)]
pub async fn get_series_collections(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(series_id): Path<String>,
) -> Result<Json<Vec<KomgaCollectionDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;
    let sid = Uuid::parse_str(&series_id)
        .map_err(|_| ApiError::NotFound("Series not found".to_string()))?;
    let vis = user_visibility(&state, auth.user_id).await?;

    let collections = CollectionRepository::get_collections_for_series(&state.db, sid)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch collections: {e}")))?;
    let mut out = Vec::with_capacity(collections.len() + 1);
    // Queued series also belong to the virtual want-to-read collection, so
    // clients cross-referencing membership stay consistent with the list view.
    if WantToReadRepository::is_series_in_queue(&state.db, auth.user_id, sid)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check want-to-read queue: {e}")))?
    {
        out.push(build_want_to_read_dto(&state, auth.user_id, vis.as_ref()).await?);
    }
    for model in collections {
        out.push(build_collection_dto(&state, model, vis.as_ref()).await?);
    }
    Ok(Json(out))
}
