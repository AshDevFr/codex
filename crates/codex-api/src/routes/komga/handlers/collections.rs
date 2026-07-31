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
use super::series::{build_series_dto, generate_series_thumbnail};
use crate::require_permission;
use crate::{
    error::ApiError,
    extractors::{AuthState, ContentFilter, FlexibleAuthContext},
    permissions::Permission,
};
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use codex_db::repositories::{
    CollectionRepository, SeriesRepository, WantToReadRepository, visibility::SeriesVisibility,
};
use codex_models::sort::{SeriesSortField, SeriesSortParam, SortDirection, WantToReadSort};
use codex_services::CollectionMembershipService;
use std::sync::Arc;
use uuid::Uuid;

/// Sentinel ID of the virtual per-user want-to-read collection. Komga clients
/// treat collection IDs as opaque strings, so a non-UUID value is safe.
pub(crate) const WANT_TO_READ_ID: &str = "want-to-read";
pub(crate) const WANT_TO_READ_NAME: &str = "Want to Read";

/// `Cache-Control` for a collection's or read list's cover.
///
/// Deliberately `private` and short-lived, unlike the series and book covers
/// these bytes are copied from. *Which* member represents the container depends
/// on the caller: sharing-tag visibility hides members per user, and an
/// automatic collection's rule can reference the viewer's own ratings or read
/// state, so two users legitimately resolve to different covers. A shared cache
/// holding this response would serve one user's cover to another, so `private`
/// keeps proxies out.
///
/// The short `max-age` lets the caller's own client skip the round-trip while
/// browsing, which matters because resolving a cover is a rule evaluation
/// rather than a lookup. The cost is that a cover can lag a membership change
/// by up to a minute: a slightly old picture, where a stale member list would
/// be a wrong answer. The accompanying `ETag` makes the revalidation after that
/// minute a 304 rather than a re-download.
pub(crate) const MEMBER_COVER_CACHE_CONTROL: &str = "private, max-age=60";

fn parse_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(|_| ApiError::NotFound("Collection not found".to_string()))
}

/// Map a Komga `sort` parameter onto a series sort.
///
/// Komga clients send `field,direction`, where the field names are the ones on
/// the series DTO. The ordering therefore has to be a *series* ordering, not the
/// collection's: a client asking for `createdDate` is asking for the order of
/// the `created` values it can see in the response, which for a manual
/// collection is not the order the series were added to it. Field names match
/// the top-level series list so both endpoints answer the same parameter the
/// same way.
///
/// `None` means "nothing usable here" — an absent parameter, or a field Codex
/// has no equivalent for — and leaves the collection's own order in place
/// rather than failing the request.
fn parse_komga_collection_sort(sort: Option<&str>) -> Option<SeriesSortParam> {
    let mut parts = sort?.trim().split(',');
    let field = parts.next()?.trim();
    let direction = match parts.next().map(str::trim) {
        Some(d) if d.eq_ignore_ascii_case("desc") => SortDirection::Desc,
        _ => SortDirection::Asc,
    };

    let field = match field {
        "metadata.titleSort" | "titleSort" | "metadata.title" | "name" => SeriesSortField::Name,
        "createdDate" | "created" | "dateAdded" => SeriesSortField::DateAdded,
        "lastModifiedDate" | "lastModified" => SeriesSortField::DateUpdated,
        "metadata.releaseDate" | "releaseDate" | "year" => SeriesSortField::ReleaseDate,
        "readProgress.lastReadDate" | "lastReadDate" => SeriesSortField::DateRead,
        "booksCount" => SeriesSortField::BookCount,
        _ => return None,
    };

    Some(SeriesSortParam { field, direction })
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

/// Build the Komga DTO for a collection.
///
/// `seriesIds` is part of the Komga contract, so this is the one place a rule
/// has to resolve during a *list* request. Both list endpoints paginate before
/// building DTOs, so the cost is bounded by page size rather than by how many
/// collections exist.
///
/// `user_id` is threaded through because a rule may reference the viewer's own
/// ratings or read state, in which case `seriesIds` differs per caller.
async fn build_collection_dto(
    state: &AuthState,
    model: codex_db::entities::collections::Model,
    vis: Option<&SeriesVisibility>,
    user_id: Option<Uuid>,
) -> Result<KomgaCollectionDto, ApiError> {
    // `seriesIds` is metadata on a list row rather than a list someone is
    // reading, so this reads through the short-lived resolution cache. The
    // members endpoint below stays live.
    let member_ids =
        CollectionMembershipService::member_ids_for_listing(&state.db, &model, vis, user_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch collection series: {e}")))?;
    Ok(KomgaCollectionDto {
        id: model.id.to_string(),
        name: model.name,
        ordered: model.ordered,
        series_ids: member_ids.iter().map(|id| id.to_string()).collect(),
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
        content.push(build_collection_dto(&state, model, vis.as_ref(), Some(auth.user_id)).await?);
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
        build_collection_dto(&state, model, vis.as_ref(), Some(auth.user_id)).await?,
    ))
}

/// Get the series in a collection (Komga-compatible).
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/collections/{collection_id}/series",
    responses((status = 200, body = KomgaPage<KomgaSeriesDto>), (status = 404)),
    params(
        ("prefix" = String, Path, description = "Komga API prefix"),
        ("collection_id" = String, Path),
        ("sort" = Option<String>, Query, description = "Sort as `field,direction` (e.g. `createdDate,asc`). Defaults to the collection's own order."),
    ),
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
        CollectionMembershipService::members(
            &state.db,
            &model,
            vis.as_ref(),
            None,
            SortDirection::default(),
            Some(auth.user_id),
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch collection series: {e}")))?
    };
    let total = members.len() as i64;

    let page = query.page.max(0);
    let size = query.size.clamp(1, 500);
    let start = (page as u64).saturating_mul(size as u64);

    // An explicit sort reorders the whole membership in the database and takes
    // the page from there; without one the members keep the order the
    // collection defines (manual position, or title) and the page is a slice of
    // that. Sorting a slice would only order the page.
    let page_members = match parse_komga_collection_sort(query.sort.as_deref()) {
        Some(sort) => {
            let ids: Vec<Uuid> = members.iter().map(|s| s.id).collect();
            // `members` is already visibility-filtered, so no visibility is
            // passed here — it would only re-apply the same predicate.
            SeriesRepository::list_by_ids_sorted(
                &state.db,
                &ids,
                &sort,
                Some(auth.user_id),
                start,
                size as u64,
                None,
            )
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to sort collection series: {e}")))?
            .0
        }
        None => members
            .into_iter()
            .skip(start as usize)
            .take(size as usize)
            .collect(),
    };

    let mut content = Vec::with_capacity(page_members.len());
    for series in page_members {
        content.push(build_series_dto(&state, &series, Some(auth.user_id)).await?);
    }
    Ok(Json(KomgaPage::new(content, page, size, total)))
}

/// Get a collection's thumbnail (the first visible member series' cover).
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/collections/{collection_id}/thumbnail",
    responses(
        (status = 200, description = "Collection thumbnail image", content_type = "image/jpeg"),
        (status = 304, description = "Not modified"),
        (status = 404, description = "Collection not found or has no visible series"),
    ),
    params(("prefix" = String, Path, description = "Komga API prefix"), ("collection_id" = String, Path)),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Komga"
)]
pub async fn get_collection_thumbnail(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    headers: HeaderMap,
    Path(collection_id): Path<String>,
) -> Result<Response, ApiError> {
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
        CollectionMembershipService::cover(&state.db, &model, vis.as_ref(), Some(auth.user_id))
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch collection cover: {e}")))?
            .into_iter()
            .collect()
    };
    let first = members
        .first()
        .ok_or_else(|| ApiError::NotFound("Collection has no visible series".to_string()))?;

    serve_member_cover(&state, first.id, &headers).await
}

/// Serve a member series' cover as the collection's own thumbnail.
///
/// Komga answers this URL with the image itself (`image/jpeg`), so clients
/// treat it as the picture rather than as a pointer to one. This used to reply
/// with a 307 to the native series thumbnail; Komic does not follow that, so
/// every collection rendered with a blank cover while the list behind it was
/// perfectly healthy.
async fn serve_member_cover(
    state: &Arc<AuthState>,
    series_id: Uuid,
    headers: &HeaderMap,
) -> Result<Response, ApiError> {
    if let Some(meta) = state
        .thumbnail_service
        .get_series_thumbnail_metadata(series_id)
        .await
    {
        if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH)
            && let Ok(client_etag) = if_none_match.to_str()
        {
            let client_etag = client_etag.trim().trim_start_matches("W/");
            if client_etag == meta.etag
                || client_etag.trim_matches('"') == meta.etag.trim_matches('"')
            {
                return Ok(Response::builder()
                    .status(StatusCode::NOT_MODIFIED)
                    .header(header::ETAG, &meta.etag)
                    .header(header::CACHE_CONTROL, MEMBER_COVER_CACHE_CONTROL)
                    .body(Body::empty())
                    .unwrap());
            }
        }

        if let Some(stream) = state
            .thumbnail_service
            .get_series_thumbnail_stream(series_id)
            .await
        {
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/jpeg")
                .header(header::CACHE_CONTROL, MEMBER_COVER_CACHE_CONTROL)
                .header(header::CONTENT_LENGTH, meta.size)
                .header(header::ETAG, &meta.etag)
                .body(Body::from_stream(stream))
                .unwrap());
        }
    }

    // A member whose file is missing or unreadable means this collection has no
    // cover to show, which is a 404 rather than a server fault: the collection
    // itself is fine, and a client asking for a picture wants "there isn't one",
    // not a 500. The underlying error is logged so it stays diagnosable.
    let thumbnail_data = match generate_series_thumbnail(state, series_id).await {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!("Failed to build collection cover from series {series_id}: {e:?}");
            return Err(ApiError::NotFound(
                "Collection has no cover available".to_string(),
            ));
        }
    };

    // ETag read back off the file just written, so it matches what the cache
    // branch above will send and the next revalidation is a 304.
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(header::CACHE_CONTROL, MEMBER_COVER_CACHE_CONTROL)
        .header(header::CONTENT_LENGTH, thumbnail_data.len());
    if let Some(meta) = state
        .thumbnail_service
        .get_series_thumbnail_metadata(series_id)
        .await
    {
        response = response.header(header::ETAG, meta.etag);
    }

    Ok(response.body(Body::from(thumbnail_data)).unwrap())
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
        out.push(build_collection_dto(&state, model, vis.as_ref(), Some(auth.user_id)).await?);
    }
    Ok(Json(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_title_sort_spellings() {
        for field in ["metadata.titleSort", "titleSort", "metadata.title", "name"] {
            let parsed = parse_komga_collection_sort(Some(&format!("{field},asc")))
                .unwrap_or_else(|| panic!("{field} should parse"));
            assert_eq!(parsed.field, SeriesSortField::Name);
            assert_eq!(parsed.direction, SortDirection::Asc);
        }
    }

    #[test]
    fn parses_date_fields() {
        let parsed = parse_komga_collection_sort(Some("createdDate,desc")).unwrap();
        assert_eq!(parsed.field, SeriesSortField::DateAdded);
        assert_eq!(parsed.direction, SortDirection::Desc);

        let parsed = parse_komga_collection_sort(Some("lastModifiedDate,asc")).unwrap();
        assert_eq!(parsed.field, SeriesSortField::DateUpdated);

        let parsed = parse_komga_collection_sort(Some("readProgress.lastReadDate,desc")).unwrap();
        assert_eq!(parsed.field, SeriesSortField::DateRead);
    }

    #[test]
    fn direction_defaults_to_ascending() {
        // Komga clients sometimes send a bare field, and anything that is not
        // "desc" is ascending.
        let parsed = parse_komga_collection_sort(Some("createdDate")).unwrap();
        assert_eq!(parsed.direction, SortDirection::Asc);

        let parsed = parse_komga_collection_sort(Some("createdDate,DESC")).unwrap();
        assert_eq!(parsed.direction, SortDirection::Desc);

        let parsed = parse_komga_collection_sort(Some("createdDate,sideways")).unwrap();
        assert_eq!(parsed.direction, SortDirection::Asc);
    }

    #[test]
    fn unknown_and_absent_sorts_yield_none() {
        // Both leave the collection's own order in place.
        assert!(parse_komga_collection_sort(None).is_none());
        assert!(parse_komga_collection_sort(Some("nonsense,asc")).is_none());
        assert!(parse_komga_collection_sort(Some("")).is_none());
        // Komga's own collection-position sort has no series-level equivalent,
        // and falling through to the collection order is exactly right for it.
        assert!(parse_komga_collection_sort(Some("collection.number,asc")).is_none());
    }
}
