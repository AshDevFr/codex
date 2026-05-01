//! Komga-compatible series handlers
//!
//! Handlers for series-related endpoints in the Komga-compatible API.

use super::super::dto::book::{KomgaBookDto, extract_library_id_from_condition};
use super::super::dto::pagination::KomgaPage;
use super::super::dto::series::{
    KomgaAlternateTitleDto, KomgaAuthorDto, KomgaBooksMetadataAggregationDto, KomgaSeriesDto,
    KomgaSeriesMetadataDto, KomgaSeriesSearchRequestDto, KomgaWebLinkDto,
    codex_to_komga_reading_direction, codex_to_komga_status, extract_read_status_from_condition,
};
use super::libraries::{extract_page_image, generate_thumbnail};
use crate::api::{
    error::ApiError,
    extractors::{AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::repositories::{
    AlternateTitleRepository, BookMetadataRepository, BookQueryOptions, BookQuerySort,
    BookRepository, BookSortField, ExternalLinkRepository, GenreRepository, ReadProgressRepository,
    SeriesCoversRepository, SeriesMetadataRepository, SeriesQueryOptions, SeriesQuerySort,
    SeriesRepository, SeriesSortFieldRepo, TagRepository,
};
use crate::require_permission;
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::fs;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Query parameters for paginated series endpoints
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct SeriesPaginationQuery {
    /// Page number (0-indexed, Komga-style)
    #[serde(default)]
    pub page: i32,
    /// Page size (default: 20)
    #[serde(default = "default_page_size")]
    pub size: i32,
    /// Filter by library ID
    pub library_id: Option<Uuid>,
    /// Search query
    pub search: Option<String>,
    /// Sort parameter (e.g., "metadata.titleSort,asc", "createdDate,desc")
    pub sort: Option<String>,
}

/// Parse Komga sort parameter into SeriesQuerySort for database-level sorting
/// Format: "field,direction" e.g., "metadata.titleSort,asc" or "createdDate,desc"
fn parse_komga_series_sort_param(sort: Option<&str>) -> Option<SeriesQuerySort> {
    let sort = sort?;
    let parts: Vec<&str> = sort.split(',').collect();
    let field_str = parts.first().copied()?;
    let ascending = parts.get(1).is_none_or(|d| d.to_lowercase() != "desc");

    let field = match field_str {
        "metadata.titleSort" | "titleSort" | "name" | "metadata.title" => {
            SeriesSortFieldRepo::Title
        }
        "createdDate" | "created" | "dateAdded" => SeriesSortFieldRepo::DateAdded,
        "lastModifiedDate" | "lastModified" | "dateUpdated" => SeriesSortFieldRepo::DateUpdated,
        "metadata.releaseDate" | "releaseDate" | "year" => SeriesSortFieldRepo::ReleaseDate,
        "lastReadDate" | "readProgress.lastReadDate" | "dateRead" => SeriesSortFieldRepo::DateRead,
        _ => return None,
    };

    Some(SeriesQuerySort { field, ascending })
}

/// Parse Komga sort parameter into BookQuerySort for books within a series
/// Format: "field,direction" e.g., "metadata.numberSort,asc"
fn parse_komga_book_sort_param(sort: Option<&str>) -> Option<BookQuerySort> {
    let sort = sort?;
    let parts: Vec<&str> = sort.split(',').collect();
    let field_str = parts.first().copied()?;
    let ascending = parts.get(1).is_none_or(|d| d.to_lowercase() != "desc");

    let field = match field_str {
        "metadata.numberSort" | "numberSort" | "metadata.number" | "number" => {
            BookSortField::ChapterNumber
        }
        "createdDate" | "created" => BookSortField::DateAdded,
        "lastModifiedDate" | "lastModified" => BookSortField::DateAdded, // fallback
        "name" | "metadata.title" => BookSortField::Title,
        _ => return None,
    };

    Some(BookQuerySort { field, ascending })
}

fn default_page_size() -> i32 {
    20
}

/// List all series (paginated)
///
/// Returns all series in Komga-compatible format with pagination.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/series`
///
/// ## Query Parameters
/// - `page` - Page number (0-indexed, default: 0)
/// - `size` - Page size (default: 20)
/// - `library_id` - Optional filter by library UUID
/// - `search` - Optional search query
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/series",
    responses(
        (status = 200, description = "Paginated list of series", body = KomgaPage<KomgaSeriesDto>),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        SeriesPaginationQuery
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn list_series(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<SeriesPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaSeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let user_id = Some(auth.user_id);
    let page = query.page.max(0) as u64;
    let size = query.size.clamp(1, 500) as u64;

    // Build query options for database-level filtering, sorting, and pagination
    let options = SeriesQueryOptions {
        library_id: query.library_id,
        user_id: Some(auth.user_id),
        search: query.search.as_deref(),
        sort: parse_komga_series_sort_param(query.sort.as_deref()),
        page,
        page_size: size,
    };

    let (series_list, total) = SeriesRepository::query(&state.db, options)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    // Convert to DTOs
    let mut dtos = Vec::with_capacity(series_list.len());
    for series in series_list {
        let dto = build_series_dto(&state, &series, user_id).await?;
        dtos.push(dto);
    }

    Ok(Json(KomgaPage::new(
        dtos,
        query.page,
        query.size,
        total as i64,
    )))
}

/// Search/filter series
///
/// Returns series matching the filter criteria.
/// This uses POST to support complex filter bodies.
///
/// ## Endpoint
/// `POST /{prefix}/api/v1/series/list`
///
/// ## Query Parameters
/// - `page` - Page number (0-indexed, default: 0)
/// - `size` - Page size (default: 20)
/// - `sort` - Sort parameter (e.g., "createdDate,desc")
///
/// ## Request Body
/// JSON object with filter criteria (library_id, fullTextSearch, condition, etc.)
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    post,
    path = "/{prefix}/api/v1/series/list",
    request_body = KomgaSeriesSearchRequestDto,
    responses(
        (status = 200, description = "Paginated list of series matching filter", body = KomgaPage<KomgaSeriesDto>),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        SeriesPaginationQuery
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn search_series(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<SeriesPaginationQuery>,
    Json(body): Json<KomgaSeriesSearchRequestDto>,
) -> Result<Json<KomgaPage<KomgaSeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let user_id = Some(auth.user_id);
    let page = query.page.max(0) as u64;
    let size = query.size.clamp(1, 500) as u64;

    // Parse library_id from body - first try direct field, then from condition object
    let library_id = body
        .library_id
        .as_ref()
        .and_then(|ids| ids.first())
        .and_then(|id| Uuid::parse_str(id).ok())
        .or_else(|| {
            body.condition
                .as_ref()
                .and_then(extract_library_id_from_condition)
                .and_then(|id| Uuid::parse_str(id).ok())
        });

    // Use fullTextSearch from body as search term
    let search_term = body.full_text_search.clone();

    // Extract readStatus from condition if present
    let read_status = body
        .condition
        .as_ref()
        .and_then(extract_read_status_from_condition);

    // If readStatus filter is present, we need to fetch all results and filter in-memory
    // because readStatus depends on book counts which require DTO building
    if read_status.is_some() {
        return search_series_with_read_status_filter(
            &state,
            user_id,
            page,
            size,
            library_id,
            search_term.as_deref(),
            read_status,
            query.sort.as_deref(),
            query.page,
            query.size,
        )
        .await;
    }

    // No readStatus filter - use efficient database-level sorting and pagination
    let options = SeriesQueryOptions {
        library_id,
        user_id: Some(auth.user_id),
        search: search_term.as_deref(),
        sort: parse_komga_series_sort_param(query.sort.as_deref()),
        page,
        page_size: size,
    };

    let (series_list, total) = SeriesRepository::query(&state.db, options)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    // Convert to DTOs
    let mut dtos = Vec::with_capacity(series_list.len());
    for series in series_list {
        let dto = build_series_dto(&state, &series, user_id).await?;
        dtos.push(dto);
    }

    Ok(Json(KomgaPage::new(
        dtos,
        query.page,
        query.size,
        total as i64,
    )))
}

/// Helper for search_series when readStatus filter is present
/// This path requires fetching all results and filtering in-memory
#[allow(clippy::too_many_arguments)]
async fn search_series_with_read_status_filter(
    state: &Arc<AuthState>,
    user_id: Option<Uuid>,
    page: u64,
    size: u64,
    library_id: Option<Uuid>,
    search_term: Option<&str>,
    read_status: Option<&str>,
    sort_param: Option<&str>,
    query_page: i32,
    query_size: i32,
) -> Result<Json<KomgaPage<KomgaSeriesDto>>, ApiError> {
    // Fetch all series (no pagination) since we need to filter by readStatus
    let options = SeriesQueryOptions {
        library_id,
        user_id,
        search: search_term,
        sort: parse_komga_series_sort_param(sort_param),
        page: 0,
        page_size: i64::MAX as u64, // Fetch all for filtering (i64::MAX avoids SQLite/PG integer overflow)
    };

    let (series_list, _) = SeriesRepository::query(&state.db, options)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    // Convert all series to DTOs (needed for readStatus filtering)
    let mut all_dtos = Vec::with_capacity(series_list.len());
    for series in series_list {
        let dto = build_series_dto(state, &series, user_id).await?;
        all_dtos.push(dto);
    }

    // Apply readStatus filter
    let filtered_dtos: Vec<_> = match read_status {
        Some("IN_PROGRESS") => all_dtos
            .into_iter()
            .filter(|s| s.books_in_progress_count > 0)
            .collect(),
        Some("READ") => all_dtos
            .into_iter()
            .filter(|s| s.books_count > 0 && s.books_read_count == s.books_count)
            .collect(),
        Some("UNREAD") => all_dtos
            .into_iter()
            .filter(|s| s.books_unread_count > 0 && s.books_in_progress_count == 0)
            .collect(),
        _ => all_dtos,
    };

    let total = filtered_dtos.len() as i64;

    // Apply pagination after filtering (sorting already done at DB level)
    let offset = page * size;
    let paginated: Vec<_> = filtered_dtos
        .into_iter()
        .skip(offset as usize)
        .take(size as usize)
        .collect();

    Ok(Json(KomgaPage::new(
        paginated, query_page, query_size, total,
    )))
}

/// Get recently added series
///
/// Returns series sorted by created date descending (newest first).
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/series/new`
///
/// ## Query Parameters
/// - `page` - Page number (0-indexed, default: 0)
/// - `size` - Page size (default: 20)
/// - `library_id` - Optional filter by library UUID
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/series/new",
    responses(
        (status = 200, description = "Paginated list of recently added series", body = KomgaPage<KomgaSeriesDto>),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        SeriesPaginationQuery
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn get_series_new(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<SeriesPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaSeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let user_id = Some(auth.user_id);
    let page = query.page.max(0) as u64;
    let size = query.size.clamp(1, 500) as u64;

    // Use composable query with DateAdded sort descending
    let options = SeriesQueryOptions {
        library_id: query.library_id,
        user_id: Some(auth.user_id),
        search: None,
        sort: Some(SeriesQuerySort {
            field: SeriesSortFieldRepo::DateAdded,
            ascending: false, // newest first
        }),
        page,
        page_size: size,
    };

    let (series_list, total) = SeriesRepository::query(&state.db, options)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    // Convert to DTOs
    let mut dtos = Vec::with_capacity(series_list.len());
    for series in series_list {
        let dto = build_series_dto(&state, &series, user_id).await?;
        dtos.push(dto);
    }

    Ok(Json(KomgaPage::new(
        dtos,
        query.page,
        query.size,
        total as i64,
    )))
}

/// Get recently updated series
///
/// Returns series sorted by last modified date descending (most recently updated first).
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/series/updated`
///
/// ## Query Parameters
/// - `page` - Page number (0-indexed, default: 0)
/// - `size` - Page size (default: 20)
/// - `library_id` - Optional filter by library UUID
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/series/updated",
    responses(
        (status = 200, description = "Paginated list of recently updated series", body = KomgaPage<KomgaSeriesDto>),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        SeriesPaginationQuery
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn get_series_updated(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<SeriesPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaSeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let user_id = Some(auth.user_id);
    let page = query.page.max(0) as u64;
    let size = query.size.clamp(1, 500) as u64;

    // Use composable query with DateUpdated sort descending
    let options = SeriesQueryOptions {
        library_id: query.library_id,
        user_id: Some(auth.user_id),
        search: None,
        sort: Some(SeriesQuerySort {
            field: SeriesSortFieldRepo::DateUpdated,
            ascending: false, // most recently updated first
        }),
        page,
        page_size: size,
    };

    let (series_list, total) = SeriesRepository::query(&state.db, options)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    // Convert to DTOs
    let mut dtos = Vec::with_capacity(series_list.len());
    for series in series_list {
        let dto = build_series_dto(&state, &series, user_id).await?;
        dtos.push(dto);
    }

    Ok(Json(KomgaPage::new(
        dtos,
        query.page,
        query.size,
        total as i64,
    )))
}

/// Get series by ID
///
/// Returns a single series in Komga-compatible format.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/series/{seriesId}`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/series/{series_id}",
    responses(
        (status = 200, description = "Series details", body = KomgaSeriesDto),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn get_series(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<KomgaSeriesDto>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let user_id = Some(auth.user_id);

    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let dto = build_series_dto(&state, &series, user_id).await?;
    Ok(Json(dto))
}

/// Get series thumbnail
///
/// Returns a thumbnail image for the series.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/series/{seriesId}/thumbnail`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/series/{series_id}/thumbnail",
    responses(
        (status = 200, description = "Series thumbnail image", content_type = "image/jpeg"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn get_series_thumbnail(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    let _series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get the series cover - try selected cover first
    let image_data = if let Some(cover) = SeriesCoversRepository::get_selected(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch cover: {}", e)))?
    {
        fs::read(&cover.path).await.map_err(|e| {
            ApiError::Internal(format!("Failed to read cover from {}: {}", cover.path, e))
        })?
    } else {
        // Fall back to first book's first page
        get_default_series_cover(&state, series_id).await?
    };

    // Generate thumbnail (max 400px width or height)
    let thumbnail_data = generate_thumbnail(&image_data, 400)
        .map_err(|e| ApiError::Internal(format!("Failed to generate thumbnail: {}", e)))?;

    // Build response with caching headers
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .header(header::CONTENT_LENGTH, thumbnail_data.len())
        .body(Body::from(thumbnail_data))
        .unwrap())
}

/// Get books in a series
///
/// Returns all books in a series with pagination.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/series/{seriesId}/books`
///
/// ## Query Parameters
/// - `page` - Page number (0-indexed, default: 0)
/// - `size` - Page size (default: 20)
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/series/{series_id}/books",
    responses(
        (status = 200, description = "Paginated list of books in series", body = KomgaPage<KomgaBookDto>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        ("series_id" = Uuid, Path, description = "Series ID"),
        SeriesPaginationQuery
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn get_series_books(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(series_id): Path<Uuid>,
    Query(query): Query<SeriesPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaBookDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let user_id = Some(auth.user_id);
    let page = query.page.max(0) as u64;
    let size = query.size.clamp(1, 500) as u64;

    // Verify series exists and get its data
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get series metadata for the series title
    let metadata = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series metadata: {}", e)))?;

    let series_title = metadata
        .as_ref()
        .map(|m| m.title.clone())
        .unwrap_or_else(|| series.name.clone());

    // Use composable query with database-level sorting and pagination
    // Default sort is by chapter number (numberSort) ascending
    let sort = parse_komga_book_sort_param(query.sort.as_deref()).unwrap_or(BookQuerySort {
        field: BookSortField::ChapterNumber,
        ascending: true,
    });

    let options = BookQueryOptions {
        series_id: Some(series_id),
        user_id: Some(auth.user_id),
        sort: Some(sort),
        page,
        page_size: size,
        ..Default::default()
    };

    let (books, total) = BookRepository::query(&state.db, options)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    // Convert to DTOs
    let mut dtos = Vec::with_capacity(books.len());
    for (idx, book) in books.into_iter().enumerate() {
        // Get read progress for this book and user
        let read_progress = if let Some(uid) = user_id {
            ReadProgressRepository::get_by_user_and_book(&state.db, uid, book.id)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        let dto = KomgaBookDto::from_codex(
            &book,
            &series_title,
            ((page * size) as i32) + (idx as i32) + 1, // 1-indexed number with offset
            read_progress.as_ref(),
        );
        dtos.push(dto);
    }

    Ok(Json(KomgaPage::new(
        dtos,
        query.page,
        query.size,
        total as i64,
    )))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Build a KomgaSeriesDto from a series entity
async fn build_series_dto(
    state: &Arc<AuthState>,
    series: &crate::db::entities::series::Model,
    user_id: Option<Uuid>,
) -> Result<KomgaSeriesDto, ApiError> {
    // Get metadata
    let metadata = SeriesMetadataRepository::get_by_series_id(&state.db, series.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series metadata: {}", e)))?;

    // Get book counts
    let book_count = SeriesRepository::get_book_count(&state.db, series.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get book count: {}", e)))?
        as i32;

    // Get reading stats for user
    let (books_read_count, books_in_progress_count) = if let Some(uid) = user_id {
        get_series_read_stats(&state.db, series.id, uid).await?
    } else {
        (0, 0)
    };

    let books_unread_count = (book_count - books_read_count - books_in_progress_count).max(0);

    // Fetch related metadata: genres, tags, links, alternate titles
    let genres = GenreRepository::get_genres_for_series(&state.db, series.id)
        .await
        .unwrap_or_default();
    let tags = TagRepository::get_tags_for_series(&state.db, series.id)
        .await
        .unwrap_or_default();
    let external_links = ExternalLinkRepository::get_for_series(&state.db, series.id)
        .await
        .unwrap_or_default();
    let alternate_titles = AlternateTitleRepository::get_for_series(&state.db, series.id)
        .await
        .unwrap_or_default();

    let genre_names: Vec<String> = genres.into_iter().map(|g| g.name).collect();
    let tag_names: Vec<String> = tags.into_iter().map(|t| t.name).collect();
    let links: Vec<KomgaWebLinkDto> = external_links
        .into_iter()
        .map(|l| KomgaWebLinkDto {
            label: l.source_name,
            url: l.url,
        })
        .collect();
    let alt_titles: Vec<KomgaAlternateTitleDto> = alternate_titles
        .into_iter()
        .map(|at| KomgaAlternateTitleDto {
            label: at.label,
            title: at.title,
        })
        .collect();

    // Aggregate book authors from book metadata
    let (aggregated_authors, aggregated_tags) =
        aggregate_books_metadata(&state.db, series.id).await;

    // Build metadata DTO
    let now = chrono::Utc::now().to_rfc3339();
    let series_metadata = if let Some(ref m) = metadata {
        KomgaSeriesMetadataDto {
            status: codex_to_komga_status(m.status.as_deref()),
            status_lock: m.status_lock,
            title: m.title.clone(),
            title_lock: m.title_lock,
            title_sort: m.title_sort.clone().unwrap_or_else(|| m.title.clone()),
            title_sort_lock: m.title_sort_lock,
            summary: m.summary.clone().unwrap_or_default(),
            summary_lock: m.summary_lock,
            reading_direction: codex_to_komga_reading_direction(m.reading_direction.as_deref()),
            reading_direction_lock: m.reading_direction_lock,
            publisher: m.publisher.clone().unwrap_or_default(),
            publisher_lock: m.publisher_lock,
            age_rating: m.age_rating,
            age_rating_lock: m.age_rating_lock,
            language: m.language.clone().unwrap_or_default(),
            language_lock: m.language_lock,
            genres: genre_names,
            genres_lock: m.genres_lock,
            tags: tag_names,
            tags_lock: m.tags_lock,
            // Komga's `totalBookCount` is volume-shaped semantically, so we
            // map our `total_volume_count` (and its lock) to it. If/when
            // Komga adds a chapter-count field upstream, surface
            // `total_chapter_count` there too.
            total_book_count: m.total_volume_count,
            total_book_count_lock: m.total_volume_count_lock,
            sharing_labels: Vec::new(),
            sharing_labels_lock: false,
            links,
            links_lock: false,
            alternate_titles: alt_titles,
            alternate_titles_lock: m.alternate_titles_lock,
            created: m.created_at.to_rfc3339(),
            last_modified: m.updated_at.to_rfc3339(),
        }
    } else {
        KomgaSeriesMetadataDto {
            status: "ONGOING".to_string(),
            title: series.name.clone(),
            title_sort: series.name.clone(),
            created: now.clone(),
            last_modified: now.clone(),
            genres: genre_names,
            tags: tag_names,
            links,
            alternate_titles: alt_titles,
            ..Default::default()
        }
    };

    // Build aggregation DTO
    let books_metadata = KomgaBooksMetadataAggregationDto {
        authors: aggregated_authors,
        tags: aggregated_tags,
        release_date: None,
        summary: series_metadata.summary.clone(),
        summary_number: String::new(),
        created: series.created_at.to_rfc3339(),
        last_modified: series.updated_at.to_rfc3339(),
    };

    Ok(KomgaSeriesDto {
        id: series.id.to_string(),
        library_id: series.library_id.to_string(),
        name: metadata
            .as_ref()
            .map(|m| m.title.clone())
            .unwrap_or_else(|| series.name.clone()),
        url: series.path.clone(),
        created: series.created_at.to_rfc3339(),
        last_modified: series.updated_at.to_rfc3339(),
        file_last_modified: series.updated_at.to_rfc3339(), // Use updated_at as proxy
        books_count: book_count,
        books_read_count,
        books_unread_count,
        books_in_progress_count,
        metadata: series_metadata,
        books_metadata,
        deleted: false,
        oneshot: metadata
            .as_ref()
            .and_then(|m| m.total_volume_count)
            .map(|count| count == 1)
            .unwrap_or(book_count == 1),
    })
}

/// Get series reading stats for a user
async fn get_series_read_stats(
    db: &sea_orm::DatabaseConnection,
    series_id: Uuid,
    user_id: Uuid,
) -> Result<(i32, i32), ApiError> {
    // Get all books in series
    let books = BookRepository::list_by_series(db, series_id, false)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    let mut read_count = 0;
    let mut in_progress_count = 0;

    for book in books {
        if let Ok(Some(progress)) =
            ReadProgressRepository::get_by_user_and_book(db, user_id, book.id).await
        {
            if progress.completed {
                read_count += 1;
            } else if progress.current_page > 0 {
                in_progress_count += 1;
            }
        }
    }

    Ok((read_count, in_progress_count))
}

/// Aggregate authors and tags from all book metadata in a series
///
/// Collects authors from individual role fields (writer, penciller, etc.)
/// and deduplicates them. Also collects tags from book metadata genre fields.
async fn aggregate_books_metadata(
    db: &sea_orm::DatabaseConnection,
    series_id: Uuid,
) -> (Vec<KomgaAuthorDto>, Vec<String>) {
    let books = BookRepository::list_by_series(db, series_id, false)
        .await
        .unwrap_or_default();

    if books.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let book_ids: Vec<Uuid> = books.iter().map(|b| b.id).collect();
    let metadata_map = BookMetadataRepository::get_by_book_ids(db, &book_ids)
        .await
        .unwrap_or_default();

    let mut authors: Vec<KomgaAuthorDto> = Vec::new();
    let mut seen_authors: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    let mut tags_set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for meta in metadata_map.values() {
        // Collect authors from authors_json field
        if let Some(ref authors_json) = meta.authors_json
            && let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(authors_json)
        {
            for entry in &entries {
                if let Some(name) = entry.get("name").and_then(|n| n.as_str()) {
                    let name = name.trim().to_string();
                    let role = entry
                        .get("role")
                        .and_then(|r| r.as_str())
                        .unwrap_or("writer")
                        .to_string();
                    if !name.is_empty() {
                        let key = (name.clone(), role.clone());
                        if seen_authors.insert(key) {
                            authors.push(KomgaAuthorDto { name, role });
                        }
                    }
                }
            }
        }

        // Collect tags from book metadata genre field
        if let Some(ref genre) = meta.genre {
            for g in genre.split(',') {
                let g = g.trim().to_string();
                if !g.is_empty() {
                    tags_set.insert(g);
                }
            }
        }
    }

    // Sort authors by name then role for stable output
    authors.sort_by(|a, b| a.name.cmp(&b.name).then(a.role.cmp(&b.role)));

    let tags: Vec<String> = tags_set.into_iter().collect();
    (authors, tags)
}

/// Get default series cover from first book's first page
async fn get_default_series_cover(
    state: &Arc<AuthState>,
    series_id: Uuid,
) -> Result<Vec<u8>, ApiError> {
    // Get the first book in the series
    let books = BookRepository::list_by_series(&state.db, series_id, false)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    let first_book = books
        .first()
        .ok_or_else(|| ApiError::NotFound("Series has no books".to_string()))?;

    // Check if book has pages
    if first_book.page_count == 0 {
        return Err(ApiError::NotFound("First book has no pages".to_string()));
    }

    // Extract first page from the book
    extract_page_image(&first_book.file_path, &first_book.format, 1)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to extract cover image: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_page_size() {
        assert_eq!(default_page_size(), 20);
    }

    #[test]
    fn test_pagination_query_defaults() {
        let query: SeriesPaginationQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(query.page, 0);
        assert_eq!(query.size, 20);
        assert!(query.library_id.is_none());
        assert!(query.search.is_none());
    }

    #[test]
    fn test_parse_komga_series_sort_param_title() {
        let result = parse_komga_series_sort_param(Some("metadata.titleSort,asc"));
        assert!(result.is_some());
        let sort = result.unwrap();
        assert_eq!(sort.field, SeriesSortFieldRepo::Title);
        assert!(sort.ascending);

        let result = parse_komga_series_sort_param(Some("titleSort,desc"));
        assert!(result.is_some());
        let sort = result.unwrap();
        assert_eq!(sort.field, SeriesSortFieldRepo::Title);
        assert!(!sort.ascending);
    }

    #[test]
    fn test_parse_komga_series_sort_param_dates() {
        // createdDate
        let result = parse_komga_series_sort_param(Some("createdDate,desc"));
        assert!(result.is_some());
        let sort = result.unwrap();
        assert_eq!(sort.field, SeriesSortFieldRepo::DateAdded);
        assert!(!sort.ascending);

        // lastModifiedDate
        let result = parse_komga_series_sort_param(Some("lastModifiedDate,asc"));
        assert!(result.is_some());
        let sort = result.unwrap();
        assert_eq!(sort.field, SeriesSortFieldRepo::DateUpdated);
        assert!(sort.ascending);

        // lastReadDate
        let result = parse_komga_series_sort_param(Some("lastReadDate,desc"));
        assert!(result.is_some());
        let sort = result.unwrap();
        assert_eq!(sort.field, SeriesSortFieldRepo::DateRead);
        assert!(!sort.ascending);
    }

    #[test]
    fn test_parse_komga_series_sort_param_release_date() {
        let result = parse_komga_series_sort_param(Some("metadata.releaseDate,desc"));
        assert!(result.is_some());
        let sort = result.unwrap();
        assert_eq!(sort.field, SeriesSortFieldRepo::ReleaseDate);
        assert!(!sort.ascending);
    }

    #[test]
    fn test_parse_komga_series_sort_param_unknown() {
        let result = parse_komga_series_sort_param(Some("unknownField,asc"));
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_komga_series_sort_param_none() {
        let result = parse_komga_series_sort_param(None);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_komga_book_sort_param_number() {
        let result = parse_komga_book_sort_param(Some("metadata.numberSort,asc"));
        assert!(result.is_some());
        let sort = result.unwrap();
        assert_eq!(sort.field, BookSortField::ChapterNumber);
        assert!(sort.ascending);
    }

    #[test]
    fn test_parse_komga_book_sort_param_title() {
        let result = parse_komga_book_sort_param(Some("metadata.title,desc"));
        assert!(result.is_some());
        let sort = result.unwrap();
        assert_eq!(sort.field, BookSortField::Title);
        assert!(!sort.ascending);
    }

    #[test]
    fn test_parse_komga_book_sort_param_created() {
        let result = parse_komga_book_sort_param(Some("createdDate,desc"));
        assert!(result.is_some());
        let sort = result.unwrap();
        assert_eq!(sort.field, BookSortField::DateAdded);
        assert!(!sort.ascending);
    }

    #[test]
    fn test_parse_komga_book_sort_param_unknown() {
        let result = parse_komga_book_sort_param(Some("unknownField,asc"));
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_komga_book_sort_param_default_ascending() {
        // If direction is not specified, default to ascending
        let result = parse_komga_book_sort_param(Some("metadata.numberSort"));
        assert!(result.is_some());
        let sort = result.unwrap();
        assert!(sort.ascending);
    }
}
