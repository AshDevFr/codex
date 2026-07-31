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
use super::collections::{
    MEMBER_COVER_CACHE_CONTROL, WANT_TO_READ_ID, WANT_TO_READ_NAME, queue_dates,
};
use crate::require_permission;
use crate::routes::v1::handlers::pages::generate_book_thumbnail;
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
    BookMetadataRepository, BookRepository, ReadListRepository, ReadProgressRepository,
    WantToReadRepository, visibility::SeriesVisibility,
};
use codex_models::sort::{BookSortField, BookSortParam, SortDirection, WantToReadSort};
use std::sync::Arc;
use uuid::Uuid;

fn parse_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(|_| ApiError::NotFound("Read list not found".to_string()))
}

/// Map a Komga `sort` parameter onto a book sort.
///
/// Komga clients send `field,direction` using the names on the book DTO, so the
/// ordering has to be a *book* ordering: `createdDate` means the book's own
/// `created`, which is when it entered the library, not when it was put on this
/// read list.
///
/// `None` means "nothing usable here" — an absent parameter, or a field Codex
/// has no equivalent for, including Komga's own `readListNumber`. Those keep
/// the read list's own order, which for a reading order is a better answer than
/// rejecting the request.
fn parse_komga_readlist_sort(sort: Option<&str>) -> Option<BookSortParam> {
    let mut parts = sort?.trim().split(',');
    let field = parts.next()?.trim();
    let direction = match parts.next().map(str::trim) {
        Some(d) if d.eq_ignore_ascii_case("desc") => SortDirection::Desc,
        _ => SortDirection::Asc,
    };

    let field = match field {
        "metadata.numberSort" | "numberSort" | "metadata.number" | "number" => {
            BookSortField::ChapterNumber
        }
        "metadata.titleSort" | "titleSort" | "metadata.title" | "name" => BookSortField::Title,
        "createdDate" | "created" | "dateAdded" => BookSortField::DateAdded,
        "metadata.releaseDate" | "releaseDate" => BookSortField::ReleaseDate,
        "readProgress.readDate" | "readDate" | "lastReadDate" => BookSortField::LastRead,
        "media.pagesCount" | "pagesCount" => BookSortField::PageCount,
        "fileSize" | "size" => BookSortField::FileSize,
        "fileName" | "url" => BookSortField::Filename,
        _ => return None,
    };

    Some(BookSortParam { field, direction })
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
    params(
        ("prefix" = String, Path, description = "Komga API prefix"),
        ("read_list_id" = String, Path),
        ("sort" = Option<String>, Query, description = "Sort as `field,direction` (e.g. `metadata.numberSort,asc`). Defaults to the read list's own order."),
    ),
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
    let start = (page as u64).saturating_mul(size as u64);

    // An explicit sort reorders the whole membership in the database and takes
    // the page from there; without one the books keep the read list's own order
    // (manual position, or release date) and the page is a slice of that.
    // Sorting a slice would only order the page.
    let page_members = match parse_komga_readlist_sort(query.sort.as_deref()) {
        Some(sort) => {
            let ids: Vec<Uuid> = members.iter().map(|b| b.id).collect();
            // `members` is already visibility-filtered, so no visibility is
            // passed here. Soft-deleted books are kept for the same reason:
            // the unsorted path returns whatever the membership holds, and a
            // sort must not quietly change which books are in the list.
            BookRepository::list_by_ids_sorted(
                &state.db,
                &ids,
                &sort,
                Some(auth.user_id),
                true,
                start,
                size as u64,
                None,
            )
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to sort read list books: {e}")))?
            .0
        }
        None => members
            .into_iter()
            .skip(start as usize)
            .take(size as usize)
            .collect(),
    };

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

/// Get a read list's thumbnail (the first visible member book's cover).
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/readlists/{read_list_id}/thumbnail",
    responses(
        (status = 200, description = "Read list thumbnail image", content_type = "image/jpeg"),
        (status = 304, description = "Not modified"),
        (status = 404, description = "Read list not found or has no visible books"),
    ),
    params(("prefix" = String, Path, description = "Komga API prefix"), ("read_list_id" = String, Path)),
    security(("jwt_bearer" = []), ("api_key" = [])),
    tag = "Komga"
)]
pub async fn get_readlist_thumbnail(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    headers: HeaderMap,
    Path(read_list_id): Path<String>,
) -> Result<Response, ApiError> {
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

    serve_member_cover(&state, first, &headers).await
}

/// Serve a member book's cover as the read list's own thumbnail.
///
/// Komga answers this URL with the image itself, so, exactly as for a
/// collection, redirecting here leaves clients that do not follow it with a
/// blank cover. See [`MEMBER_COVER_CACHE_CONTROL`] for why these bytes are
/// cached privately and briefly rather than inheriting the book cover's own
/// long-lived caching.
async fn serve_member_cover(
    state: &Arc<AuthState>,
    book: &codex_db::entities::books::Model,
    headers: &HeaderMap,
) -> Result<Response, ApiError> {
    if let Some(meta) = state
        .thumbnail_service
        .get_thumbnail_metadata(book.id)
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

        if let Some(stream) = state.thumbnail_service.get_thumbnail_stream(book.id).await {
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

    // A member whose file is missing or unreadable means this read list has no
    // cover to show, which is a 404 rather than a server fault: the list itself
    // is fine, and a client asking for a picture wants "there isn't one", not a
    // 500. The underlying error is logged so it stays diagnosable.
    let thumbnail_data = if book.page_count == 0 {
        return Err(ApiError::NotFound(
            "Read list has no cover available".to_string(),
        ));
    } else {
        match generate_book_thumbnail(state, book).await {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!(
                    "Failed to build read list cover from book {}: {e:?}",
                    book.id
                );
                return Err(ApiError::NotFound(
                    "Read list has no cover available".to_string(),
                ));
            }
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
        .get_thumbnail_metadata(book.id)
        .await
    {
        response = response.header(header::ETAG, meta.etag);
    }

    Ok(response.body(Body::from(thumbnail_data)).unwrap())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_book_number_spellings() {
        for field in [
            "metadata.numberSort",
            "numberSort",
            "metadata.number",
            "number",
        ] {
            let parsed = parse_komga_readlist_sort(Some(&format!("{field},asc")))
                .unwrap_or_else(|| panic!("{field} should parse"));
            assert_eq!(parsed.field, BookSortField::ChapterNumber);
            assert_eq!(parsed.direction, SortDirection::Asc);
        }
    }

    #[test]
    fn parses_the_remaining_fields() {
        for (field, expected) in [
            ("metadata.title", BookSortField::Title),
            ("createdDate", BookSortField::DateAdded),
            ("metadata.releaseDate", BookSortField::ReleaseDate),
            ("readProgress.readDate", BookSortField::LastRead),
            ("media.pagesCount", BookSortField::PageCount),
            ("fileSize", BookSortField::FileSize),
            ("fileName", BookSortField::Filename),
        ] {
            let parsed = parse_komga_readlist_sort(Some(&format!("{field},desc")))
                .unwrap_or_else(|| panic!("{field} should parse"));
            assert_eq!(parsed.field, expected, "field {field}");
            assert_eq!(parsed.direction, SortDirection::Desc);
        }
    }

    #[test]
    fn direction_defaults_to_ascending() {
        // A bare field, or anything that is not "desc", is ascending.
        assert_eq!(
            parse_komga_readlist_sort(Some("createdDate"))
                .unwrap()
                .direction,
            SortDirection::Asc
        );
        assert_eq!(
            parse_komga_readlist_sort(Some("createdDate,DESC"))
                .unwrap()
                .direction,
            SortDirection::Desc
        );
        assert_eq!(
            parse_komga_readlist_sort(Some("createdDate,sideways"))
                .unwrap()
                .direction,
            SortDirection::Asc
        );
    }

    #[test]
    fn unknown_and_absent_sorts_yield_none() {
        // Each leaves the read list's own reading order in place.
        assert!(parse_komga_readlist_sort(None).is_none());
        assert!(parse_komga_readlist_sort(Some("")).is_none());
        assert!(parse_komga_readlist_sort(Some("nonsense,asc")).is_none());
        // Komga's own read-list position sort has no book-level equivalent, and
        // falling through to the read list's order is exactly right for it.
        assert!(parse_komga_readlist_sort(Some("readListNumber,asc")).is_none());
    }
}
