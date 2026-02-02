use super::super::dto::{
    common::{
        ListPaginationParams, PaginatedResponse, PaginationLinkBuilder, DEFAULT_PAGE,
        DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE,
    },
    series::{
        AddSeriesGenreRequest, AddSeriesTagRequest, AlphabeticalGroupDto, AlternateTitleDto,
        AlternateTitleListResponse, CreateAlternateTitleRequest, CreateExternalLinkRequest,
        CreateExternalRatingRequest, ExternalLinkDto, ExternalLinkListResponse, ExternalRatingDto,
        ExternalRatingListResponse, FullSeriesListResponse, FullSeriesMetadataResponse,
        FullSeriesResponse, GenreDto, GenreListResponse, MetadataLocks, PatchSeriesMetadataRequest,
        PatchSeriesRequest, ReplaceSeriesMetadataRequest, SeriesAverageRatingResponse,
        SeriesCoverDto, SeriesCoverListResponse, SeriesExternalIdDto, SeriesFullMetadata,
        SeriesMetadataResponse, SeriesSortParam, SeriesUpdateResponse, SetSeriesGenresRequest,
        SetSeriesTagsRequest, SetUserRatingRequest, TagDto, TagListResponse,
        TaxonomyCleanupResponse, UpdateAlternateTitleRequest, UpdateMetadataLocksRequest,
        UserRatingsListResponse, UserSeriesRatingDto,
    },
    BookDto, MarkReadResponse, SearchSeriesRequest, SeriesDto, SeriesListRequest,
    SeriesListResponse,
};
use super::paginated_response;
use crate::api::{
    error::ApiError,
    extractors::{AuthContext, AuthState, ContentFilter, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::entities::{series, series_metadata};
use crate::db::repositories::{
    AlternateTitleRepository, BookRepository, ExternalLinkRepository, ExternalRatingRepository,
    GenreRepository, LibraryRepository, ReadProgressRepository, SeriesCoversRepository,
    SeriesExternalIdRepository, SeriesMetadataRepository, SeriesRepository, TagRepository,
    UserSeriesRatingRepository,
};
use crate::events::{EntityChangeEvent, EntityEvent, EntityType};
use crate::require_permission;
use crate::utils::{
    parse_custom_metadata, serialize_custom_metadata, validate_custom_metadata_size,
};
use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use httpdate::fmt_http_date;
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use std::io::{Cursor, Write};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

/// Placeholder SVG for series thumbnails that are being generated or don't exist
/// This is a simple gray rectangle with a book icon, loaded from assets at compile time
const PLACEHOLDER_SVG: &[u8] = include_bytes!("../../../../../assets/placeholder-cover.svg");

/// Query parameters for listing books in a series
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct ListBooksQuery {
    /// Include deleted books in the result
    #[serde(default)]
    pub include_deleted: bool,

    /// Return full data including metadata and locks.
    /// Default is false for backward compatibility.
    #[serde(default)]
    pub full: bool,
}

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_page_size() -> u64 {
    DEFAULT_PAGE_SIZE
}

/// Query parameters for listing series
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct SeriesListQuery {
    /// Page number (1-indexed, default 1)
    #[serde(default = "default_page")]
    pub page: u64,

    /// Number of items per page (max 100, default 50)
    #[serde(default = "default_page_size")]
    pub page_size: u64,

    /// Sort parameter (format: "field,direction" e.g. "name,asc")
    #[serde(default)]
    pub sort: Option<String>,

    /// Filter by genres (comma-separated, AND logic - series must have ALL specified genres)
    #[serde(default)]
    pub genres: Option<String>,

    /// Filter by tags (comma-separated, AND logic - series must have ALL specified tags)
    #[serde(default)]
    pub tags: Option<String>,

    /// Filter by library ID
    #[serde(default)]
    pub library_id: Option<Uuid>,

    /// Return full series data including metadata, locks, genres, tags, alternate titles,
    /// external ratings, and external links. Default is false for backward compatibility.
    #[serde(default)]
    pub full: bool,
}

/// Helper function to convert series model to DTO with unread count
/// Fetches metadata, cover info, and book count from related tables
async fn series_to_dto(
    db: &DatabaseConnection,
    series: series::Model,
    user_id: Option<Uuid>,
) -> Result<SeriesDto, ApiError> {
    let unread_count = if let Some(uid) = user_id {
        BookRepository::count_unread_in_series(db, series.id, uid)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to count unread books: {:?}", e)))
            .map(Some)?
    } else {
        None
    };

    // Fetch metadata from series_metadata table (contains title, title_sort, summary, etc.)
    let metadata = SeriesMetadataRepository::get_by_series_id(db, series.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series metadata: {:?}", e)))?;

    // Compute book count dynamically (no longer stored on series table)
    let book_count = SeriesRepository::get_book_count(db, series.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get book count: {:?}", e)))?;

    // Fetch cover info from series_covers table
    let selected_cover = SeriesCoversRepository::get_selected(db, series.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series cover: {:?}", e)))?;

    let has_custom_cover = SeriesCoversRepository::has_custom_cover(db, series.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check custom cover: {:?}", e)))?;

    // Fetch library name
    let library = LibraryRepository::get_by_id(db, series.library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch library: {:?}", e)))?;
    let library_name = library
        .map(|l| l.name)
        .unwrap_or_else(|| "Unknown Library".to_string());

    // Series name now comes from series_metadata.title (fall back to "Unknown Series" if not found)
    let name = metadata
        .as_ref()
        .map(|m| m.title.clone())
        .unwrap_or_else(|| "Unknown Series".to_string());

    Ok(SeriesDto {
        id: series.id,
        library_id: series.library_id,
        library_name,
        title: name,
        title_sort: metadata.as_ref().and_then(|m| m.title_sort.clone()),
        summary: metadata.as_ref().and_then(|m| m.summary.clone()),
        publisher: metadata.as_ref().and_then(|m| m.publisher.clone()),
        year: metadata.as_ref().and_then(|m| m.year),
        book_count,
        path: Some(series.path),
        selected_cover_source: selected_cover.map(|c| c.source),
        has_custom_cover: Some(has_custom_cover),
        unread_count,
        created_at: series.created_at,
        updated_at: series.updated_at,
    })
}

/// Convert multiple series models to FullSeriesResponse DTOs using batched queries
///
/// This is much more efficient than calling series_to_dto + full data fetching N times
/// because it uses IN clauses to batch all related data fetches.
async fn series_to_full_dtos_batched(
    db: &DatabaseConnection,
    series_list: Vec<series::Model>,
    user_id: Option<Uuid>,
) -> Result<Vec<FullSeriesResponse>, ApiError> {
    use std::collections::HashMap;

    if series_list.is_empty() {
        return Ok(vec![]);
    }

    // Collect all series IDs and unique library IDs
    let series_ids: Vec<Uuid> = series_list.iter().map(|s| s.id).collect();
    let library_ids: Vec<Uuid> = series_list
        .iter()
        .map(|s| s.library_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Fetch all related data in parallel using batched queries
    let (
        metadata_map,
        book_counts_map,
        unread_counts_map,
        selected_covers_map,
        custom_covers_map,
        libraries_map,
        genres_map,
        tags_map,
        alt_titles_map,
        ext_ratings_map,
        ext_links_map,
        ext_ids_map,
    ) = tokio::join!(
        SeriesMetadataRepository::get_by_series_ids(db, &series_ids),
        SeriesRepository::get_book_counts_for_series_ids(db, &series_ids),
        async {
            if let Some(uid) = user_id {
                BookRepository::count_unread_in_series_ids(db, &series_ids, uid).await
            } else {
                Ok(HashMap::new())
            }
        },
        SeriesCoversRepository::get_selected_for_series_ids(db, &series_ids),
        SeriesCoversRepository::has_custom_cover_for_series_ids(db, &series_ids),
        LibraryRepository::get_by_ids(db, &library_ids),
        GenreRepository::get_genres_for_series_ids(db, &series_ids),
        TagRepository::get_tags_for_series_ids(db, &series_ids),
        AlternateTitleRepository::get_for_series_ids(db, &series_ids),
        ExternalRatingRepository::get_for_series_ids(db, &series_ids),
        ExternalLinkRepository::get_for_series_ids(db, &series_ids),
        SeriesExternalIdRepository::get_for_series_ids(db, &series_ids),
    );

    // Handle errors
    let metadata_map =
        metadata_map.map_err(|e| ApiError::Internal(format!("Failed to fetch metadata: {}", e)))?;
    let book_counts_map = book_counts_map
        .map_err(|e| ApiError::Internal(format!("Failed to get book counts: {}", e)))?;
    let unread_counts_map = unread_counts_map
        .map_err(|e| ApiError::Internal(format!("Failed to count unread: {}", e)))?;
    let selected_covers_map = selected_covers_map
        .map_err(|e| ApiError::Internal(format!("Failed to fetch covers: {}", e)))?;
    let custom_covers_map = custom_covers_map
        .map_err(|e| ApiError::Internal(format!("Failed to check custom covers: {}", e)))?;
    let libraries_map = libraries_map
        .map_err(|e| ApiError::Internal(format!("Failed to fetch libraries: {}", e)))?;
    let genres_map =
        genres_map.map_err(|e| ApiError::Internal(format!("Failed to fetch genres: {}", e)))?;
    let tags_map =
        tags_map.map_err(|e| ApiError::Internal(format!("Failed to fetch tags: {}", e)))?;
    let alt_titles_map = alt_titles_map
        .map_err(|e| ApiError::Internal(format!("Failed to fetch alternate titles: {}", e)))?;
    let ext_ratings_map = ext_ratings_map
        .map_err(|e| ApiError::Internal(format!("Failed to fetch external ratings: {}", e)))?;
    let ext_links_map = ext_links_map
        .map_err(|e| ApiError::Internal(format!("Failed to fetch external links: {}", e)))?;
    let ext_ids_map = ext_ids_map
        .map_err(|e| ApiError::Internal(format!("Failed to fetch external IDs: {}", e)))?;

    // Build full responses
    let mut results = Vec::with_capacity(series_list.len());

    for series in series_list {
        let series_id = series.id;

        // Get metadata (required)
        let metadata = metadata_map.get(&series_id).ok_or_else(|| {
            ApiError::Internal(format!("Series metadata not found for {}", series_id))
        })?;

        // Get other data (with defaults)
        let book_count = book_counts_map.get(&series_id).copied().unwrap_or(0);
        let unread_count = unread_counts_map.get(&series_id).copied();
        let selected_cover = selected_covers_map.get(&series_id);
        let has_custom_cover = custom_covers_map.get(&series_id).copied().unwrap_or(false);
        let library_name = libraries_map
            .get(&series.library_id)
            .map(|l| l.name.clone())
            .unwrap_or_else(|| "Unknown Library".to_string());

        // Convert genres to DTOs
        let genre_dtos: Vec<GenreDto> = genres_map
            .get(&series_id)
            .map(|genres| {
                genres
                    .iter()
                    .map(|g| GenreDto {
                        id: g.id,
                        name: g.name.clone(),
                        series_count: None,
                        created_at: g.created_at,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Convert tags to DTOs
        let tag_dtos: Vec<TagDto> = tags_map
            .get(&series_id)
            .map(|tags| {
                tags.iter()
                    .map(|t| TagDto {
                        id: t.id,
                        name: t.name.clone(),
                        series_count: None,
                        created_at: t.created_at,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Convert alternate titles to DTOs
        let alt_title_dtos: Vec<AlternateTitleDto> = alt_titles_map
            .get(&series_id)
            .map(|titles| {
                titles
                    .iter()
                    .map(|at| AlternateTitleDto {
                        id: at.id,
                        series_id: at.series_id,
                        label: at.label.clone(),
                        title: at.title.clone(),
                        created_at: at.created_at,
                        updated_at: at.updated_at,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Convert external ratings to DTOs
        let ext_rating_dtos: Vec<ExternalRatingDto> = ext_ratings_map
            .get(&series_id)
            .map(|ratings| {
                use sea_orm::prelude::Decimal;
                ratings
                    .iter()
                    .map(|er| ExternalRatingDto {
                        id: er.id,
                        series_id: er.series_id,
                        source_name: er.source_name.clone(),
                        rating: Decimal::to_string(&er.rating).parse::<f64>().unwrap_or(0.0),
                        vote_count: er.vote_count,
                        fetched_at: er.fetched_at,
                        created_at: er.created_at,
                        updated_at: er.updated_at,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Convert external links to DTOs
        let ext_link_dtos: Vec<ExternalLinkDto> = ext_links_map
            .get(&series_id)
            .map(|links| {
                links
                    .iter()
                    .map(|el| ExternalLinkDto {
                        id: el.id,
                        series_id: el.series_id,
                        source_name: el.source_name.clone(),
                        url: el.url.clone(),
                        external_id: el.external_id.clone(),
                        created_at: el.created_at,
                        updated_at: el.updated_at,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Convert external IDs to DTOs
        let ext_id_dtos: Vec<SeriesExternalIdDto> = ext_ids_map
            .get(&series_id)
            .map(|ids| ids.iter().cloned().map(SeriesExternalIdDto::from).collect())
            .unwrap_or_default();

        results.push(FullSeriesResponse {
            id: series.id,
            library_id: series.library_id,
            library_name,
            book_count,
            unread_count,
            path: Some(series.path),
            selected_cover_source: selected_cover.map(|c| c.source.clone()),
            has_custom_cover: Some(has_custom_cover),
            metadata: SeriesFullMetadata {
                title: metadata.title.clone(),
                title_sort: metadata.title_sort.clone(),
                summary: metadata.summary.clone(),
                publisher: metadata.publisher.clone(),
                imprint: metadata.imprint.clone(),
                status: metadata.status.clone(),
                age_rating: metadata.age_rating,
                language: metadata.language.clone(),
                reading_direction: metadata.reading_direction.clone(),
                year: metadata.year,
                total_book_count: metadata.total_book_count,
                custom_metadata: parse_custom_metadata(metadata.custom_metadata.as_deref()),
                locks: MetadataLocks {
                    title: metadata.title_lock,
                    title_sort: metadata.title_sort_lock,
                    summary: metadata.summary_lock,
                    publisher: metadata.publisher_lock,
                    imprint: metadata.imprint_lock,
                    status: metadata.status_lock,
                    age_rating: metadata.age_rating_lock,
                    language: metadata.language_lock,
                    reading_direction: metadata.reading_direction_lock,
                    year: metadata.year_lock,
                    total_book_count: metadata.total_book_count_lock,
                    genres: metadata.genres_lock,
                    tags: metadata.tags_lock,
                    custom_metadata: metadata.custom_metadata_lock,
                    cover: metadata.cover_lock,
                },
                created_at: metadata.created_at,
                updated_at: metadata.updated_at,
            },
            genres: genre_dtos,
            tags: tag_dtos,
            alternate_titles: alt_title_dtos,
            external_ratings: ext_rating_dtos,
            external_links: ext_link_dtos,
            external_ids: ext_id_dtos,
            created_at: series.created_at,
            updated_at: series.updated_at,
        });
    }

    Ok(results)
}

/// List series with optional library filter and pagination
#[utoipa::path(
    get,
    path = "/api/v1/series",
    params(SeriesListQuery),
    responses(
        (status = 200, description = "Paginated list of series (returns FullSeriesListResponse when full=true)", body = SeriesListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn list_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<SeriesListQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Validate and normalize pagination params (1-indexed)
    let page = query.page.max(1);
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(MAX_PAGE_SIZE)
    };

    // Fetch all series IDs first (for filtering)
    let all_series = if let Some(library_id) = query.library_id {
        SeriesRepository::list_by_library(&state.db, library_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
    } else {
        SeriesRepository::list_all(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
    };

    // Collect IDs after applying filters
    let mut filtered_ids: Vec<Uuid> = all_series.iter().map(|s| s.id).collect();

    // Apply sharing tag content filter (exclude series the user doesn't have access to)
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    if content_filter.has_restrictions {
        filtered_ids.retain(|id| content_filter.is_series_visible(*id));
    }

    // Apply genre filter if specified
    if let Some(genres_param) = &query.genres {
        let genre_names: Vec<String> = genres_param
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if !genre_names.is_empty() {
            let matching_series_ids =
                GenreRepository::get_series_ids_by_genre_names(&state.db, &genre_names)
                    .await
                    .map_err(|e| {
                        ApiError::Internal(format!("Failed to filter by genres: {}", e))
                    })?;

            filtered_ids.retain(|id| matching_series_ids.contains(id));
        }
    }

    // Apply tag filter if specified
    if let Some(tags_param) = &query.tags {
        let tag_names: Vec<String> = tags_param
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if !tag_names.is_empty() {
            let matching_series_ids =
                TagRepository::get_series_ids_by_tag_names(&state.db, &tag_names)
                    .await
                    .map_err(|e| ApiError::Internal(format!("Failed to filter by tags: {}", e)))?;

            filtered_ids.retain(|id| matching_series_ids.contains(id));
        }
    }

    // Parse sort parameter (default to name,asc)
    let sort = query
        .sort
        .as_ref()
        .map(|s| SeriesSortParam::parse(s))
        .unwrap_or_default();

    // Use database-level sorting with the filtered IDs (convert to 0-indexed offset)
    let offset = (page - 1) * page_size;
    let (series_list, total) = SeriesRepository::list_by_ids_sorted(
        &state.db,
        &filtered_ids,
        &sort,
        Some(auth.user_id),
        offset,
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch sorted series: {}", e)))?;

    // Build pagination links
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };
    let mut link_builder =
        PaginationLinkBuilder::new("/api/v1/series", page, page_size, total_pages);
    if let Some(library_id) = query.library_id {
        link_builder = link_builder.with_param("library_id", &library_id.to_string());
    }
    if let Some(ref genres) = query.genres {
        link_builder = link_builder.with_param("genres", genres);
    }
    if let Some(ref tags) = query.tags {
        link_builder = link_builder.with_param("tags", tags);
    }
    if let Some(ref sort_str) = query.sort {
        link_builder = link_builder.with_param("sort", sort_str);
    }
    if query.full {
        link_builder = link_builder.with_param("full", "true");
    }

    // Build response based on full parameter
    if query.full {
        let full_dtos =
            series_to_full_dtos_batched(&state.db, series_list, Some(auth.user_id)).await?;
        let response =
            FullSeriesListResponse::with_builder(full_dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    } else {
        let user_id = Some(auth.user_id);
        let dtos: Vec<SeriesDto> = futures::future::join_all(
            series_list
                .into_iter()
                .map(|series| series_to_dto(&state.db, series, user_id)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ApiError::Internal(format!("Failed to build series DTOs: {:?}", e)))?;

        let response =
            SeriesListResponse::with_builder(dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    }
}

/// Query parameters for getting a single series
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct SeriesGetQuery {
    /// Return full series data including metadata, locks, genres, tags, etc.
    #[serde(default)]
    pub full: bool,
}

/// Get series by ID
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        SeriesGetQuery,
    ),
    responses(
        (status = 200, description = "Series details (returns FullSeriesResponse when full=true)", body = SeriesDto),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn get_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Query(query): Query<SeriesGetQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Check sharing tag access
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    if !content_filter.is_series_visible(series_id) {
        return Err(ApiError::NotFound("Series not found".to_string()));
    }

    if query.full {
        let full_dtos =
            series_to_full_dtos_batched(&state.db, vec![series], Some(auth.user_id)).await?;
        let full_dto = full_dtos
            .into_iter()
            .next()
            .ok_or_else(|| ApiError::Internal("Failed to build full series DTO".to_string()))?;
        Ok(Json(full_dto).into_response())
    } else {
        let user_id = Some(auth.user_id);
        let dto = series_to_dto(&state.db, series, user_id).await?;
        Ok(Json(dto).into_response())
    }
}

/// Update series core fields (name/title)
///
/// Partially updates series_metadata fields. Only provided fields will be updated.
/// Absent fields are unchanged. When name is set to a non-null value, it is automatically locked.
#[utoipa::path(
    patch,
    path = "/api/v1/series/{series_id}",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = PatchSeriesRequest,
    responses(
        (status = 200, description = "Series updated successfully", body = SeriesUpdateResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn patch_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<PatchSeriesRequest>,
) -> Result<Json<SeriesUpdateResponse>, ApiError> {
    use sea_orm::{ActiveModelTrait, Set};

    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists
    let series_model = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let now = Utc::now();
    let mut has_changes = false;

    // Get or create series_metadata record (name is now stored as title in series_metadata)
    let existing_meta = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let updated_title: String;

    if let Some(existing) = existing_meta {
        // Update existing metadata record
        let mut active: series_metadata::ActiveModel = existing.clone().into();

        // Update title if provided
        if let Some(Some(title)) = request.title.into_nested_option() {
            active.title = Set(title.clone());
            active.title_lock = Set(true); // Auto-lock when user edits
            has_changes = true;
            updated_title = title;
        } else {
            updated_title = existing.title.clone();
        }

        if has_changes {
            active.updated_at = Set(now);
            active.update(&state.db).await.map_err(|e| {
                ApiError::Internal(format!("Failed to update series metadata: {}", e))
            })?;
        }
    } else {
        // Create new metadata record with provided title
        if let Some(Some(title)) = request.title.into_nested_option() {
            has_changes = true;
            updated_title = title.clone();

            let active = series_metadata::ActiveModel {
                series_id: Set(series_id),
                title: Set(title),
                title_sort: Set(None),
                summary: Set(None),
                publisher: Set(None),
                imprint: Set(None),
                status: Set(None),
                age_rating: Set(None),
                language: Set(None),
                reading_direction: Set(None),
                year: Set(None),
                total_book_count: Set(None),
                custom_metadata: Set(None),
                total_book_count_lock: Set(false),
                title_lock: Set(true), // Auto-lock when user edits
                title_sort_lock: Set(false),
                summary_lock: Set(false),
                publisher_lock: Set(false),
                imprint_lock: Set(false),
                status_lock: Set(false),
                age_rating_lock: Set(false),
                language_lock: Set(false),
                reading_direction_lock: Set(false),
                year_lock: Set(false),
                genres_lock: Set(false),
                tags_lock: Set(false),
                custom_metadata_lock: Set(false),
                cover_lock: Set(false),
                created_at: Set(now),
                updated_at: Set(now),
            };

            active.insert(&state.db).await.map_err(|e| {
                ApiError::Internal(format!("Failed to create series metadata: {}", e))
            })?;
        } else {
            // No title provided and no existing metadata - return current state
            updated_title = "Unknown Series".to_string();
        }
    }

    // Emit update event
    if has_changes {
        let event = EntityChangeEvent {
            event: EntityEvent::SeriesUpdated {
                series_id,
                library_id: series_model.library_id,
                fields: Some(vec!["title".to_string()]),
            },
            timestamp: now,
            user_id: Some(auth.user_id),
        };
        let _ = state.event_broadcaster.emit(event);
    }

    Ok(Json(SeriesUpdateResponse {
        id: series_id,
        title: updated_title,
        updated_at: now,
    }))
}

/// Search series by name
#[utoipa::path(
    post,
    path = "/api/v1/series/search",
    request_body = SearchSeriesRequest,
    responses(
        (status = 200, description = "Search results (returns Vec<FullSeriesResponse> when full=true)", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn search_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<SearchSeriesRequest>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series_list = SeriesRepository::search_by_name(&state.db, &request.query)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to search series: {}", e)))?;

    // Apply sharing tag content filter
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    // Filter by library and sharing tags
    let filtered: Vec<_> = series_list
        .into_iter()
        .filter(|s| {
            // Apply library filter if specified
            if let Some(lib_id) = request.library_id {
                if s.library_id != lib_id {
                    return false;
                }
            }
            // Apply sharing tag filter
            content_filter.is_series_visible(s.id)
        })
        .collect();

    // Build response based on full parameter
    if request.full {
        let full_dtos =
            series_to_full_dtos_batched(&state.db, filtered, Some(auth.user_id)).await?;
        Ok(Json(full_dtos).into_response())
    } else {
        let user_id = Some(auth.user_id);
        let dtos: Vec<SeriesDto> = futures::future::join_all(
            filtered
                .into_iter()
                .map(|series| series_to_dto(&state.db, series, user_id)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ApiError::Internal(format!("Failed to build series DTOs: {:?}", e)))?;

        Ok(Json(dtos).into_response())
    }
}

/// List series with advanced filtering
///
/// Supports complex filter conditions including nested AllOf/AnyOf logic,
/// genre/tag filtering with include/exclude, and more.
///
/// Pagination parameters (page, pageSize, sort) are passed as query parameters.
/// Filter conditions are passed in the request body.
#[utoipa::path(
    post,
    path = "/api/v1/series/list",
    params(ListPaginationParams),
    request_body = SeriesListRequest,
    responses(
        (status = 200, description = "Paginated list of filtered series (returns FullSeriesListResponse when full=true)", body = SeriesListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn list_series_filtered(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(pagination): Query<ListPaginationParams>,
    Json(request): Json<SeriesListRequest>,
) -> Result<Response, ApiError> {
    use crate::services::FilterService;
    use std::collections::HashSet;

    require_permission!(auth, Permission::SeriesRead)?;

    // Validate and normalize pagination params (1-indexed, from query params)
    let (page, page_size) = pagination.validated();

    // Get all series IDs first (we'll filter from this)
    let all_series = SeriesRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    // Apply sharing tag content filter
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    let all_series: Vec<_> = all_series
        .into_iter()
        .filter(|s| content_filter.is_series_visible(s.id))
        .collect();

    let all_series_ids: HashSet<Uuid> = all_series.iter().map(|s| s.id).collect();

    // Apply filter condition if provided (with user context for ReadStatus filtering)
    let matching_ids = if let Some(ref condition) = request.condition {
        FilterService::get_matching_series_for_user(
            &state.db,
            condition,
            Some(&all_series_ids),
            Some(auth.user_id),
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to apply filter: {}", e)))?
    } else {
        all_series_ids.clone()
    };

    // Apply full-text search if provided - get the final list of IDs
    let filtered_ids: Vec<Uuid> = if let Some(ref search_query) = request.full_text_search {
        if !search_query.trim().is_empty() {
            // Use full-text search with candidate filtering
            let candidate_ids: Vec<Uuid> = matching_ids.iter().cloned().collect();
            let search_results = SeriesRepository::full_text_search_filtered(
                &state.db,
                search_query,
                &candidate_ids,
            )
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to search series: {}", e)))?;
            search_results.iter().map(|s| s.id).collect()
        } else {
            // Empty search query, use condition-filtered results
            matching_ids.into_iter().collect()
        }
    } else {
        // No full-text search, use condition-filtered results
        matching_ids.into_iter().collect()
    };

    // Parse sort parameter from query params (default to name,asc)
    let sort = pagination
        .sort
        .as_ref()
        .map(|s| SeriesSortParam::parse(s))
        .unwrap_or_default();

    // Use database-level sorting with the filtered IDs (convert to 0-indexed offset)
    let offset = (page - 1) * page_size;
    let (series_list, total) = SeriesRepository::list_by_ids_sorted(
        &state.db,
        &filtered_ids,
        &sort,
        Some(auth.user_id),
        offset,
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch sorted series: {}", e)))?;

    // Build pagination links with query params
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };
    let mut link_builder =
        PaginationLinkBuilder::new("/api/v1/series/list", page, page_size, total_pages);
    if let Some(ref sort_str) = pagination.sort {
        link_builder = link_builder.with_param("sort", sort_str);
    }
    if pagination.full {
        link_builder = link_builder.with_param("full", "true");
    }

    // Build response based on full parameter
    if pagination.full {
        let full_dtos =
            series_to_full_dtos_batched(&state.db, series_list, Some(auth.user_id)).await?;
        let response =
            FullSeriesListResponse::with_builder(full_dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    } else {
        let user_id = Some(auth.user_id);
        let dtos: Vec<SeriesDto> = futures::future::join_all(
            series_list
                .into_iter()
                .map(|series| series_to_dto(&state.db, series, user_id)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ApiError::Internal(format!("Failed to build series DTOs: {:?}", e)))?;

        let response =
            SeriesListResponse::with_builder(dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    }
}

/// Get alphabetical groups for series
///
/// Returns a list of alphabetical groups with counts, showing how many series
/// start with each letter/character. This is useful for building A-Z navigation.
/// The same filters as list_series_filtered can be applied.
#[utoipa::path(
    post,
    path = "/api/v1/series/list/alphabetical-groups",
    request_body = SeriesListRequest,
    responses(
        (status = 200, description = "List of alphabetical groups with counts", body = Vec<AlphabeticalGroupDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn list_series_alphabetical_groups(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<SeriesListRequest>,
) -> Result<Json<Vec<AlphabeticalGroupDto>>, ApiError> {
    use crate::services::FilterService;
    use std::collections::HashMap;

    require_permission!(auth, Permission::SeriesRead)?;

    // Get all series IDs first (we'll filter from this)
    let all_series = SeriesRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    // Apply sharing tag content filter
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    let all_series: Vec<_> = all_series
        .into_iter()
        .filter(|s| content_filter.is_series_visible(s.id))
        .collect();

    let all_series_ids: std::collections::HashSet<Uuid> = all_series.iter().map(|s| s.id).collect();

    // Apply filter condition if provided (with user context for ReadStatus filtering)
    let matching_ids = if let Some(ref condition) = request.condition {
        FilterService::get_matching_series_for_user(
            &state.db,
            condition,
            Some(&all_series_ids),
            Some(auth.user_id),
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to apply filter: {}", e)))?
    } else {
        all_series_ids.clone()
    };

    // Get the filtered series
    let filtered_series: Vec<_> = all_series
        .into_iter()
        .filter(|s| matching_ids.contains(&s.id))
        .collect();

    // Get metadata for all filtered series to access title_sort
    let series_ids: Vec<Uuid> = filtered_series.iter().map(|s| s.id).collect();

    // Fetch all series metadata in one query
    let metadata_map = SeriesMetadataRepository::get_by_series_ids(&state.db, &series_ids)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch metadata: {}", e)))?;

    // Count series by first character of sort title (or title if sort title not set)
    let mut group_counts: HashMap<String, i64> = HashMap::new();

    for series in &filtered_series {
        let metadata = metadata_map.get(&series.id);

        // Get title_sort (fallback to title, then series name)
        let title_sort = metadata
            .and_then(|m| m.title_sort.as_ref().or(Some(&m.title)))
            .map(|s| s.as_str())
            .unwrap_or(&series.name);

        // Get first character, normalize to lowercase
        let first_char = title_sort
            .chars()
            .next()
            .map(|c| c.to_lowercase().to_string())
            .unwrap_or_else(|| "#".to_string());

        *group_counts.entry(first_char).or_insert(0) += 1;
    }

    // Convert to sorted list of AlphabeticalGroupDto
    let mut groups: Vec<AlphabeticalGroupDto> = group_counts
        .into_iter()
        .map(|(group, count)| AlphabeticalGroupDto { group, count })
        .collect();

    // Sort alphabetically (numbers/special chars first, then letters)
    groups.sort_by(|a, b| a.group.cmp(&b.group));

    Ok(Json(groups))
}

/// Get books in a series
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/books",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        ListBooksQuery
    ),
    responses(
        (status = 200, description = "List of books in the series (returns Vec<FullBookResponse> when full=true)", body = Vec<BookDto>),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn get_series_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Query(query): Query<ListBooksQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Fetch books
    let books = BookRepository::list_by_series(&state.db, series_id, query.include_deleted)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    // Return full or basic response based on the full parameter
    if query.full {
        let full_dtos =
            super::books::books_to_full_dtos_batched(&state.db, auth.user_id, books).await?;
        Ok(Json(full_dtos).into_response())
    } else {
        let dtos = super::books::books_to_dtos(&state.db, auth.user_id, books).await?;
        Ok(Json(dtos).into_response())
    }
}

/// Purge deleted books from a series
#[utoipa::path(
    delete,
    path = "/api/v1/series/{series_id}/purge-deleted",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "Number of books purged", body = u64),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn purge_series_deleted_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<u64>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Purge deleted books
    let count = BookRepository::purge_deleted_in_series(
        &state.db,
        series_id,
        Some(&state.event_broadcaster),
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to purge deleted books: {}", e)))?;

    // Emit bulk purge event if any books were deleted
    if count > 0 {
        let event = EntityChangeEvent {
            event: EntityEvent::SeriesBulkPurged {
                series_id,
                library_id: series.library_id,
                count,
            },
            timestamp: Utc::now(),
            user_id: Some(auth.user_id),
        };
        let _ = state.event_broadcaster.emit(event);
    }

    Ok(Json(count))
}

/// Upload a custom cover/poster for a series
#[utoipa::path(
    post,
    path = "/api/v1/series/{series_id}/cover",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body(content = inline(Object), description = "Multipart form with image file", content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Cover uploaded successfully"),
        (status = 400, description = "Invalid image or request"),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn upload_series_cover(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get its library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get the uploaded file from multipart form
    let mut image_data: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to read multipart field: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();

        if name == "cover" || name == "file" || name == "image" {
            let data = field
                .bytes()
                .await
                .map_err(|e| ApiError::BadRequest(format!("Failed to read file data: {}", e)))?;
            image_data = Some(data.to_vec());
            break;
        }
    }

    let image_data = image_data
        .ok_or_else(|| ApiError::BadRequest("No image file provided in request".to_string()))?;

    // Validate that it's a valid image
    image::load_from_memory(&image_data)
        .map_err(|e| ApiError::BadRequest(format!("Invalid image file: {}", e)))?;

    // Compute hash of image data for deduplication
    let image_hash = crate::utils::hasher::hash_bytes(&image_data);
    // Use first 16 chars of hash for filename (64 chars is excessive)
    let short_hash = &image_hash[..16];

    // Create covers directory within uploads dir if it doesn't exist
    let covers_dir = state.thumbnail_service.get_uploads_dir().join("covers");
    fs::create_dir_all(&covers_dir)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create covers directory: {}", e)))?;

    // Use series_id and image hash for filename to avoid duplicates
    let filename = format!("{}-{}.jpg", series_id, short_hash);
    let filepath = covers_dir.join(&filename);

    // Check if this exact image already exists for this series
    if filepath.exists() {
        return Err(ApiError::BadRequest(
            "This image has already been uploaded for this series".to_string(),
        ));
    }

    let mut file = fs::File::create(&filepath)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create cover file: {}", e)))?;

    file.write_all(&image_data)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to write cover file: {}", e)))?;

    // Create a new custom cover (allows multiple covers per series)
    // This automatically deselects any previously selected cover
    SeriesCoversRepository::create(
        &state.db,
        series_id,
        "custom",
        &filepath.to_string_lossy(),
        true, // is_selected
        None,
        None,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to create cover: {}", e)))?;

    // Auto-lock cover to prevent plugins from overwriting user's custom upload
    SeriesMetadataRepository::update_cover_lock(&state.db, series_id, true)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to lock cover: {}", e)))?;

    // Touch series to update updated_at (for cache busting)
    SeriesRepository::touch(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update series timestamp: {}", e)))?;

    // Emit cover updated event
    let event = EntityChangeEvent {
        event: EntityEvent::CoverUpdated {
            entity_type: EntityType::Series,
            entity_id: series_id,
            library_id: Some(series.library_id),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::OK)
}

/// Set which cover source to use for a series (partial update)
#[utoipa::path(
    patch,
    path = "/api/v1/series/{series_id}/cover/source",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = SelectCoverSourceRequest,
    responses(
        (status = 200, description = "Cover source updated successfully"),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn set_series_cover_source(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<SelectCoverSourceRequest>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Select the cover by source (e.g., "custom", "book:uuid")
    let selected = SeriesCoversRepository::select_by_source(&state.db, series_id, &request.source)
        .await
        .map_err(|e| {
            ApiError::Internal(format!("Failed to update selected cover source: {}", e))
        })?;

    if selected.is_none() {
        return Err(ApiError::NotFound(format!(
            "Cover source '{}' not found for this series",
            request.source
        )));
    }

    // Regenerate the series thumbnail to reflect the new cover
    regenerate_series_thumbnail(&state, series_id).await;

    // Emit cover updated event
    let event = EntityChangeEvent {
        event: EntityEvent::CoverUpdated {
            entity_type: EntityType::Series,
            entity_id: series_id,
            library_id: Some(series.library_id),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::OK)
}

/// Get thumbnail/cover image for a series
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/thumbnail",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
    ),
    responses(
        (status = 200, description = "Thumbnail image", content_type = "image/jpeg"),
        (status = 304, description = "Not modified (client cache is valid)"),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn get_series_thumbnail(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    headers: HeaderMap,
    Path(series_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // OPTIMIZATION 1: Check disk cache FIRST before hitting the database.
    // This avoids acquiring a DB connection for cached thumbnails.
    if let Some(meta) = state
        .thumbnail_service
        .get_series_thumbnail_metadata(series_id)
        .await
    {
        // Check If-None-Match header for ETag validation
        if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH) {
            if let Ok(client_etag) = if_none_match.to_str() {
                let client_etag = client_etag.trim().trim_start_matches("W/");
                if client_etag == meta.etag
                    || client_etag.trim_matches('"') == meta.etag.trim_matches('"')
                {
                    return Ok(Response::builder()
                        .status(StatusCode::NOT_MODIFIED)
                        .header(header::ETAG, &meta.etag)
                        .header(header::CACHE_CONTROL, "public, max-age=31536000")
                        .body(Body::empty())
                        .unwrap());
                }
            }
        }

        // Check If-Modified-Since header
        if let Some(if_modified_since) = headers.get(header::IF_MODIFIED_SINCE) {
            if let Ok(date_str) = if_modified_since.to_str() {
                if let Ok(client_time) = httpdate::parse_http_date(date_str) {
                    let file_time = UNIX_EPOCH + Duration::from_secs(meta.modified_unix);
                    if file_time <= client_time {
                        return Ok(Response::builder()
                            .status(StatusCode::NOT_MODIFIED)
                            .header(header::ETAG, &meta.etag)
                            .header(header::CACHE_CONTROL, "public, max-age=31536000")
                            .body(Body::empty())
                            .unwrap());
                    }
                }
            }
        }

        // Cache hit - stream the thumbnail directly (no DB query needed!)
        if let Some(stream) = state
            .thumbnail_service
            .get_series_thumbnail_stream(series_id)
            .await
        {
            let last_modified = UNIX_EPOCH + Duration::from_secs(meta.modified_unix);
            let last_modified_str = fmt_http_date(last_modified);

            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/jpeg")
                .header(header::CONTENT_LENGTH, meta.size)
                .header(header::ETAG, &meta.etag)
                .header(header::LAST_MODIFIED, last_modified_str)
                .header(header::CACHE_CONTROL, "public, max-age=31536000")
                .body(Body::from_stream(stream))
                .unwrap());
        }
    }

    // Cache miss - queue a background task to generate the thumbnail
    // and return a placeholder immediately.
    //
    // Check if there's a selected custom cover first. If there is, we still
    // need to wait for that since we can't generate it in background.
    let selected_cover = SeriesCoversRepository::get_selected(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch cover: {}", e)))?;

    if let Some(cover) = selected_cover {
        // Custom cover selected - read and resize synchronously since it's
        // a direct file read (not extracting from a book archive)
        match fs::read(&cover.path).await {
            Ok(data) => {
                // Use ThumbnailService for consistent settings (max_dimension, quality)
                match state
                    .thumbnail_service
                    .generate_thumbnail_from_image(&state.db, data)
                    .await
                {
                    Ok(thumbnail_data) => {
                        // Save to cache for future requests
                        if let Err(e) = state
                            .thumbnail_service
                            .save_series_thumbnail(series_id, &thumbnail_data)
                            .await
                        {
                            tracing::warn!(
                                "Failed to cache series thumbnail for {}: {}",
                                series_id,
                                e
                            );
                        }

                        // Generate ETag from cover ID + thumbnail size + current timestamp
                        // This ensures browser cache is busted when cover changes
                        let now = std::time::SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        let etag = format!(
                            "\"{:x}-{:x}-{:x}\"",
                            cover.id.as_u128(),
                            thumbnail_data.len(),
                            now
                        );
                        let last_modified_str = fmt_http_date(std::time::SystemTime::now());

                        return Ok(Response::builder()
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, "image/jpeg")
                            .header(header::CACHE_CONTROL, "public, max-age=31536000")
                            .header(header::CONTENT_LENGTH, thumbnail_data.len())
                            .header(header::ETAG, &etag)
                            .header(header::LAST_MODIFIED, last_modified_str)
                            .body(Body::from(thumbnail_data))
                            .unwrap());
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to generate thumbnail from cover {} for series {}: {:?}",
                            cover.path,
                            series_id,
                            e
                        );
                        return Ok(serve_series_placeholder_response());
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to read cover from {} for series {}: {}",
                    cover.path,
                    series_id,
                    e
                );
                return Ok(serve_series_placeholder_response());
            }
        }
    }

    // No custom cover - queue a background task to generate from first book
    // and return placeholder immediately
    tracing::debug!(
        "Series {} thumbnail cache miss, queueing generation task",
        series_id
    );

    // Queue the thumbnail generation task (fire and forget)
    use crate::db::repositories::TaskRepository;
    use crate::tasks::types::TaskType;

    let task_type = TaskType::GenerateSeriesThumbnail {
        series_id,
        force: false, // Don't force if we somehow have a race condition
    };

    if let Err(e) = TaskRepository::enqueue(&state.db, task_type, 0, None).await {
        tracing::warn!(
            "Failed to queue series thumbnail generation task for {}: {}",
            series_id,
            e
        );
    }

    // Return placeholder - client will retry and get real thumbnail once task completes
    Ok(serve_series_placeholder_response())
}

/// Serve a placeholder SVG image for missing/generating series thumbnails
fn serve_series_placeholder_response() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/svg+xml")
        .header(header::CACHE_CONTROL, "public, max-age=10") // Short cache for placeholders
        .header(header::CONTENT_LENGTH, PLACEHOLDER_SVG.len())
        .body(Body::from(PLACEHOLDER_SVG.to_vec()))
        .unwrap()
}

/// Query parameters for in-progress series
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct InProgressSeriesQuery {
    /// Filter by library ID (optional)
    #[serde(default)]
    pub library_id: Option<Uuid>,

    /// Return full series data including metadata, locks, genres, tags, etc.
    #[serde(default)]
    pub full: bool,
}

/// List series with in-progress books (series that have at least one book with reading progress that is not completed)
#[utoipa::path(
    get,
    path = "/api/v1/series/in-progress",
    params(InProgressSeriesQuery),
    responses(
        (status = 200, description = "List of in-progress series (returns Vec<FullSeriesResponse> when full=true)", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn list_in_progress_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<InProgressSeriesQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Fetch in-progress series for the current user
    let series_list = SeriesRepository::list_in_progress(&state.db, auth.user_id, query.library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch in-progress series: {}", e)))?;

    // Apply sharing tag content filter
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    let series_list: Vec<_> = series_list
        .into_iter()
        .filter(|s| content_filter.is_series_visible(s.id))
        .collect();

    // Build response based on full parameter
    if query.full {
        let full_dtos =
            series_to_full_dtos_batched(&state.db, series_list, Some(auth.user_id)).await?;
        Ok(Json(full_dtos).into_response())
    } else {
        let user_id = Some(auth.user_id);
        let dtos: Vec<SeriesDto> = futures::future::join_all(
            series_list
                .into_iter()
                .map(|series| series_to_dto(&state.db, series, user_id)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ApiError::Internal(format!("Failed to build series DTOs: {:?}", e)))?;

        Ok(Json(dtos).into_response())
    }
}

/// Query parameters for recently added/updated series
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct RecentSeriesQuery {
    /// Maximum number of series to return (default: 50)
    #[serde(default = "default_recent_limit")]
    pub limit: u64,

    /// Filter by library ID (optional)
    #[serde(default)]
    pub library_id: Option<Uuid>,

    /// Return full series data including metadata, locks, genres, tags, etc.
    #[serde(default)]
    pub full: bool,
}

fn default_recent_limit() -> u64 {
    50
}

/// List recently added series
#[utoipa::path(
    get,
    path = "/api/v1/series/recently-added",
    params(RecentSeriesQuery),
    responses(
        (status = 200, description = "List of recently added series (returns Vec<FullSeriesResponse> when full=true)", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn list_recently_added_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<RecentSeriesQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series_list =
        SeriesRepository::list_recently_added(&state.db, query.library_id, query.limit)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch recently added series: {}", e))
            })?;

    // Apply sharing tag content filter
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    let series_list: Vec<_> = series_list
        .into_iter()
        .filter(|s| content_filter.is_series_visible(s.id))
        .collect();

    // Build response based on full parameter
    if query.full {
        let full_dtos =
            series_to_full_dtos_batched(&state.db, series_list, Some(auth.user_id)).await?;
        Ok(Json(full_dtos).into_response())
    } else {
        let user_id = Some(auth.user_id);
        let dtos: Vec<SeriesDto> = futures::future::join_all(
            series_list
                .into_iter()
                .map(|series| series_to_dto(&state.db, series, user_id)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ApiError::Internal(format!("Failed to build series DTOs: {:?}", e)))?;

        Ok(Json(dtos).into_response())
    }
}

/// List recently added series in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/series/recently-added",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        RecentSeriesQuery,
    ),
    responses(
        (status = 200, description = "List of recently added series in library (returns Vec<FullSeriesResponse> when full=true)", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn list_library_recently_added_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<RecentSeriesQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series_list =
        SeriesRepository::list_recently_added(&state.db, Some(library_id), query.limit)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch recently added series: {}", e))
            })?;

    // Apply sharing tag content filter
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    let series_list: Vec<_> = series_list
        .into_iter()
        .filter(|s| content_filter.is_series_visible(s.id))
        .collect();

    // Build response based on full parameter
    if query.full {
        let full_dtos =
            series_to_full_dtos_batched(&state.db, series_list, Some(auth.user_id)).await?;
        Ok(Json(full_dtos).into_response())
    } else {
        let user_id = Some(auth.user_id);
        let dtos: Vec<SeriesDto> = futures::future::join_all(
            series_list
                .into_iter()
                .map(|series| series_to_dto(&state.db, series, user_id)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ApiError::Internal(format!("Failed to build series DTOs: {:?}", e)))?;

        Ok(Json(dtos).into_response())
    }
}

/// List recently updated series
#[utoipa::path(
    get,
    path = "/api/v1/series/recently-updated",
    params(RecentSeriesQuery),
    responses(
        (status = 200, description = "List of recently updated series (returns Vec<FullSeriesResponse> when full=true)", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn list_recently_updated_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<RecentSeriesQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series_list =
        SeriesRepository::list_recently_updated(&state.db, query.library_id, query.limit)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch recently updated series: {}", e))
            })?;

    // Apply sharing tag content filter
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    let series_list: Vec<_> = series_list
        .into_iter()
        .filter(|s| content_filter.is_series_visible(s.id))
        .collect();

    if query.full {
        let full_dtos =
            series_to_full_dtos_batched(&state.db, series_list, Some(auth.user_id)).await?;
        Ok(Json(full_dtos).into_response())
    } else {
        let user_id = Some(auth.user_id);
        let dtos: Vec<SeriesDto> = futures::future::join_all(
            series_list
                .into_iter()
                .map(|series| series_to_dto(&state.db, series, user_id)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ApiError::Internal(format!("Failed to build series DTOs: {:?}", e)))?;

        Ok(Json(dtos).into_response())
    }
}

/// List recently updated series in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/series/recently-updated",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        RecentSeriesQuery
    ),
    responses(
        (status = 200, description = "List of recently updated series in library (returns Vec<FullSeriesResponse> when full=true)", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn list_library_recently_updated_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<RecentSeriesQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series_list =
        SeriesRepository::list_recently_updated(&state.db, Some(library_id), query.limit)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch recently updated series: {}", e))
            })?;

    // Apply sharing tag content filter
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    let series_list: Vec<_> = series_list
        .into_iter()
        .filter(|s| content_filter.is_series_visible(s.id))
        .collect();

    if query.full {
        let full_dtos =
            series_to_full_dtos_batched(&state.db, series_list, Some(auth.user_id)).await?;
        Ok(Json(full_dtos).into_response())
    } else {
        let user_id = Some(auth.user_id);
        let dtos: Vec<SeriesDto> = futures::future::join_all(
            series_list
                .into_iter()
                .map(|series| series_to_dto(&state.db, series, user_id)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ApiError::Internal(format!("Failed to build series DTOs: {:?}", e)))?;

        Ok(Json(dtos).into_response())
    }
}

/// List series in a specific library with pagination
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/series",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        SeriesListQuery
    ),
    responses(
        (status = 200, description = "Paginated list of series in library (returns FullSeriesListResponse when full=true)", body = SeriesListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn list_library_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<SeriesListQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Validate and normalize pagination params (1-indexed)
    let page = query.page.max(1);
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(MAX_PAGE_SIZE)
    };

    // Parse sort parameter
    let sort = query
        .sort
        .as_ref()
        .map(|s| SeriesSortParam::parse(s))
        .unwrap_or_default();

    // Load content filter for sharing tags
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    // Fetch all series IDs from library for filtering
    let all_series = SeriesRepository::list_by_library(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    // Collect IDs after applying sharing tag filter
    let filtered_ids: Vec<Uuid> = if content_filter.has_restrictions {
        all_series
            .iter()
            .filter(|s| content_filter.is_series_visible(s.id))
            .map(|s| s.id)
            .collect()
    } else {
        all_series.iter().map(|s| s.id).collect()
    };

    // Use database-level sorting with the filtered IDs (convert to 0-indexed offset)
    let offset = (page - 1) * page_size;
    let (series_list, total) = SeriesRepository::list_by_ids_sorted(
        &state.db,
        &filtered_ids,
        &sort,
        Some(auth.user_id),
        offset,
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch sorted series: {}", e)))?;

    // Build pagination links
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };
    let link_builder = PaginationLinkBuilder::new(
        &format!("/api/v1/libraries/{}/series", library_id),
        page,
        page_size,
        total_pages,
    );

    if query.full {
        let full_dtos =
            series_to_full_dtos_batched(&state.db, series_list, Some(auth.user_id)).await?;
        let response =
            FullSeriesListResponse::with_builder(full_dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    } else {
        let user_id = Some(auth.user_id);
        let dtos: Vec<SeriesDto> = futures::future::join_all(
            series_list
                .into_iter()
                .map(|series| series_to_dto(&state.db, series, user_id)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ApiError::Internal(format!("Failed to build series DTOs: {:?}", e)))?;

        let response =
            SeriesListResponse::with_builder(dtos, page, page_size, total, &link_builder);
        Ok(paginated_response(response, &link_builder))
    }
}

/// Query parameters for library-scoped in-progress series
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct LibraryInProgressSeriesQuery {
    /// Return full series data including metadata, locks, genres, tags, etc.
    #[serde(default)]
    pub full: bool,
}

/// List in-progress series in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/series/in-progress",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        LibraryInProgressSeriesQuery
    ),
    responses(
        (status = 200, description = "List of in-progress series in library (returns Vec<FullSeriesResponse> when full=true)", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn list_library_in_progress_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<LibraryInProgressSeriesQuery>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Fetch in-progress series for the current user in this library
    let series_list = SeriesRepository::list_in_progress(&state.db, auth.user_id, Some(library_id))
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch in-progress series: {}", e)))?;

    // Apply sharing tag content filter
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    let series_list: Vec<_> = series_list
        .into_iter()
        .filter(|s| content_filter.is_series_visible(s.id))
        .collect();

    if query.full {
        let full_dtos =
            series_to_full_dtos_batched(&state.db, series_list, Some(auth.user_id)).await?;
        Ok(Json(full_dtos).into_response())
    } else {
        let user_id = Some(auth.user_id);
        let dtos: Vec<SeriesDto> = futures::future::join_all(
            series_list
                .into_iter()
                .map(|series| series_to_dto(&state.db, series, user_id)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ApiError::Internal(format!("Failed to build series DTOs: {:?}", e)))?;

        Ok(Json(dtos).into_response())
    }
}

/// Request to select which cover source to use
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SelectCoverSourceRequest {
    /// Cover source: "default" (first book cover) or "custom" (uploaded cover)
    pub source: String,
}

/// Mark all books in a series as read
#[utoipa::path(
    post,
    path = "/api/v1/series/{series_id}/read",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "Series marked as read", body = MarkReadResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn mark_series_as_read(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<MarkReadResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Verify series exists
    let _series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get all books in the series with their page counts
    let books = BookRepository::list_by_series(&state.db, series_id, false)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books in series: {}", e)))?;

    if books.is_empty() {
        return Ok(Json(MarkReadResponse {
            count: 0,
            message: "No books in series to mark as read".to_string(),
        }));
    }

    // Create a vector of (book_id, page_count) tuples
    let book_data: Vec<(Uuid, i32)> = books
        .iter()
        .map(|book| (book.id, book.page_count))
        .collect();

    // Mark all books as read
    let count = ReadProgressRepository::mark_series_as_read(&state.db, auth.user_id, book_data)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to mark series as read: {}", e)))?;

    Ok(Json(MarkReadResponse {
        count,
        message: format!("Marked {} books as read", count),
    }))
}

/// Mark all books in a series as unread
#[utoipa::path(
    post,
    path = "/api/v1/series/{series_id}/unread",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "Series marked as unread", body = MarkReadResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn mark_series_as_unread(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<MarkReadResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Verify series exists
    let _series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get all book IDs in the series
    let books = BookRepository::list_by_series(&state.db, series_id, false)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books in series: {}", e)))?;

    if books.is_empty() {
        return Ok(Json(MarkReadResponse {
            count: 0,
            message: "No books in series to mark as unread".to_string(),
        }));
    }

    let book_ids: Vec<Uuid> = books.iter().map(|book| book.id).collect();

    // Mark all books as unread (delete progress records)
    let count = ReadProgressRepository::mark_series_as_unread(&state.db, auth.user_id, book_ids)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to mark series as unread: {}", e)))?;

    Ok(Json(MarkReadResponse {
        count: count as usize,
        message: format!("Marked {} books as unread", count),
    }))
}

/// Download all books in a series as a zip file
///
/// Creates a zip archive containing all detected books in the series.
/// Only includes books that were scanned and detected by the library scanner.
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/download",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "Zip file containing all books in the series", content_type = "application/zip"),
        (status = 404, description = "Series not found or has no books"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn download_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Fetch series to verify it exists
    let _series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Fetch series name from series_metadata
    let series_name = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .ok()
        .flatten()
        .map(|m| m.title)
        .unwrap_or_else(|| format!("series-{}", series_id));

    // Fetch all non-deleted books in the series
    let books = BookRepository::list_by_series(&state.db, series_id, false)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    if books.is_empty() {
        return Err(ApiError::NotFound(
            "Series has no books to download".to_string(),
        ));
    }

    // Create zip archive in memory
    let buffer = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buffer);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o644);

    // Track which filenames we've used to avoid duplicates
    let mut used_filenames = std::collections::HashSet::new();

    for book in &books {
        let file_path = std::path::Path::new(&book.file_path);

        // Skip books whose files don't exist on disk
        if !file_path.exists() {
            tracing::warn!(
                book_id = %book.id,
                file_path = %book.file_path,
                "Skipping book download - file not found on disk"
            );
            continue;
        }

        // Read the file contents
        let file_contents = tokio::fs::read(&book.file_path).await.map_err(|e| {
            ApiError::Internal(format!(
                "Failed to read book file {}: {}",
                book.file_name, e
            ))
        })?;

        // Generate a unique filename if there are duplicates
        let mut filename = book.file_name.clone();
        let mut counter = 1;
        while used_filenames.contains(&filename) {
            let path = std::path::Path::new(&book.file_name);
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            filename = if ext.is_empty() {
                format!("{} ({})", stem, counter)
            } else {
                format!("{} ({}).{}", stem, counter, ext)
            };
            counter += 1;
        }
        used_filenames.insert(filename.clone());

        // Add file to zip
        zip.start_file(&filename, options)
            .map_err(|e| ApiError::Internal(format!("Failed to add file to zip: {}", e)))?;

        zip.write_all(&file_contents)
            .map_err(|e| ApiError::Internal(format!("Failed to write file to zip: {}", e)))?;
    }

    // Finalize the zip and get the buffer back
    let buffer = zip
        .finish()
        .map_err(|e| ApiError::Internal(format!("Failed to finalize zip: {}", e)))?;

    let zip_data = buffer.into_inner();

    // Sanitize series name for use as filename
    let safe_name = sanitize_filename(&series_name);
    let zip_filename = format!("{}.zip", safe_name);

    // Build response
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_LENGTH, zip_data.len())
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", zip_filename),
        )
        .body(Body::from(zip_data))
        .unwrap())
}

/// Replace all series metadata (PUT)
///
/// Replaces all metadata fields with the values in the request.
/// Omitting a field (or setting it to null) will clear that field.
#[utoipa::path(
    put,
    path = "/api/v1/series/{series_id}/metadata",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = ReplaceSeriesMetadataRequest,
    responses(
        (status = 200, description = "Metadata replaced successfully", body = SeriesMetadataResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn replace_series_metadata(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<ReplaceSeriesMetadataRequest>,
) -> Result<Json<SeriesMetadataResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Find the series
    let existing_series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get existing metadata
    let existing_metadata = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::Internal("Series metadata not found".to_string()))?;

    // Full replacement - all fields from request become the new state
    use sea_orm::{ActiveModelTrait, Set};
    let mut active: series_metadata::ActiveModel = existing_metadata.into();

    // Update title if provided, otherwise keep existing
    if let Some(title) = request.title.clone() {
        active.title = Set(title);
        active.title_lock = Set(true); // Auto-lock when user edits
    }
    active.title_sort = Set(request.title_sort.clone());
    active.summary = Set(request.summary.clone());
    active.publisher = Set(request.publisher.clone());
    active.imprint = Set(request.imprint.clone());
    active.status = Set(request.status.clone());
    active.age_rating = Set(request.age_rating);
    active.language = Set(request.language.clone());
    active.reading_direction = Set(request.reading_direction.clone());
    active.year = Set(request.year);
    active.total_book_count = Set(request.total_book_count);

    // Validate and convert custom_metadata from JSON Value to String
    if let Some(ref cm) = request.custom_metadata {
        validate_custom_metadata_size(Some(cm)).map_err(ApiError::BadRequest)?;
    }
    active.custom_metadata = Set(serialize_custom_metadata(request.custom_metadata.as_ref()));
    active.updated_at = Set(Utc::now());

    let updated_metadata = active
        .update(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update series metadata: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: existing_series.library_id,
            fields: Some(vec![
                "title".to_string(),
                "title_sort".to_string(),
                "summary".to_string(),
                "publisher".to_string(),
                "imprint".to_string(),
                "status".to_string(),
                "age_rating".to_string(),
                "language".to_string(),
                "reading_direction".to_string(),
                "year".to_string(),
                "total_book_count".to_string(),
                "custom_metadata".to_string(),
            ]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(SeriesMetadataResponse {
        id: series_id,
        title: updated_metadata.title,
        title_sort: updated_metadata.title_sort,
        summary: updated_metadata.summary,
        publisher: updated_metadata.publisher,
        imprint: updated_metadata.imprint,
        status: updated_metadata.status,
        age_rating: updated_metadata.age_rating,
        language: updated_metadata.language,
        reading_direction: updated_metadata.reading_direction,
        year: updated_metadata.year,
        total_book_count: updated_metadata.total_book_count,
        custom_metadata: parse_custom_metadata(updated_metadata.custom_metadata.as_deref()),
        updated_at: updated_metadata.updated_at,
    }))
}

/// Partially update series metadata (PATCH)
///
/// Only provided fields will be updated. Absent fields are unchanged.
/// Explicitly null fields will be cleared.
#[utoipa::path(
    patch,
    path = "/api/v1/series/{series_id}/metadata",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = PatchSeriesMetadataRequest,
    responses(
        (status = 200, description = "Metadata updated successfully", body = SeriesMetadataResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn patch_series_metadata(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<PatchSeriesMetadataRequest>,
) -> Result<Json<SeriesMetadataResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Find the series
    let existing_series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get existing metadata
    let existing_metadata = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::Internal("Series metadata not found".to_string()))?;

    // Partial update - only set fields that were provided
    use sea_orm::{ActiveModelTrait, Set};
    let mut metadata_active: series_metadata::ActiveModel = existing_metadata.clone().into();
    let mut has_changes = false;

    // Handle title update with auto-lock
    if let Some(Some(title)) = request.title.into_nested_option() {
        metadata_active.title = Set(title);
        metadata_active.title_lock = Set(true); // Auto-lock when user edits
        has_changes = true;
    }
    if let Some(opt) = request.title_sort.into_nested_option() {
        metadata_active.title_sort = Set(opt);
        has_changes = true;
    }
    if let Some(opt) = request.summary.into_nested_option() {
        metadata_active.summary = Set(opt);
        has_changes = true;
    }
    if let Some(opt) = request.publisher.into_nested_option() {
        metadata_active.publisher = Set(opt);
        has_changes = true;
    }
    if let Some(opt) = request.imprint.into_nested_option() {
        metadata_active.imprint = Set(opt);
        has_changes = true;
    }
    if let Some(opt) = request.status.into_nested_option() {
        metadata_active.status = Set(opt);
        has_changes = true;
    }
    if let Some(opt) = request.age_rating.into_nested_option() {
        metadata_active.age_rating = Set(opt);
        has_changes = true;
    }
    if let Some(opt) = request.language.into_nested_option() {
        metadata_active.language = Set(opt);
        has_changes = true;
    }
    if let Some(opt) = request.reading_direction.into_nested_option() {
        metadata_active.reading_direction = Set(opt);
        has_changes = true;
    }
    if let Some(opt) = request.year.into_nested_option() {
        metadata_active.year = Set(opt);
        has_changes = true;
    }
    if let Some(opt) = request.total_book_count.into_nested_option() {
        metadata_active.total_book_count = Set(opt);
        has_changes = true;
    }
    if let Some(opt) = request.custom_metadata.into_nested_option() {
        // Validate size if value is provided
        if let Some(ref cm) = opt {
            validate_custom_metadata_size(Some(cm)).map_err(ApiError::BadRequest)?;
        }
        // Convert from JSON Value to String for database storage
        metadata_active.custom_metadata = Set(serialize_custom_metadata(opt.as_ref()));
        has_changes = true;
    }

    // Update metadata table if needed
    let updated_metadata = if has_changes {
        metadata_active.updated_at = Set(Utc::now());
        metadata_active
            .update(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update series metadata: {}", e)))?
    } else {
        existing_metadata
    };

    // Emit update event
    if has_changes {
        let event = EntityChangeEvent {
            event: EntityEvent::SeriesUpdated {
                series_id,
                library_id: existing_series.library_id,
                fields: None, // PATCH updates only changed fields
            },
            timestamp: Utc::now(),
            user_id: Some(auth.user_id),
        };
        let _ = state.event_broadcaster.emit(event);
    }

    Ok(Json(SeriesMetadataResponse {
        id: series_id,
        title: updated_metadata.title,
        title_sort: updated_metadata.title_sort,
        summary: updated_metadata.summary,
        publisher: updated_metadata.publisher,
        imprint: updated_metadata.imprint,
        status: updated_metadata.status,
        age_rating: updated_metadata.age_rating,
        language: updated_metadata.language,
        reading_direction: updated_metadata.reading_direction,
        year: updated_metadata.year,
        total_book_count: updated_metadata.total_book_count,
        custom_metadata: parse_custom_metadata(updated_metadata.custom_metadata.as_deref()),
        updated_at: updated_metadata.updated_at,
    }))
}

/// Get series metadata including all related data
///
/// Returns comprehensive metadata with lock states, genres, tags, alternate titles,
/// external ratings, and external links.
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/metadata",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "Series metadata with all related data", body = FullSeriesMetadataResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn get_series_metadata(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<FullSeriesMetadataResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    let _series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get metadata
    let metadata = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series metadata: {}", e)))?
        .ok_or_else(|| ApiError::Internal("Series metadata not found".to_string()))?;

    // Fetch all related data in parallel
    let (genres_result, tags_result, alt_titles_result, ext_ratings_result, ext_links_result) = tokio::join!(
        GenreRepository::get_genres_for_series(&state.db, series_id),
        TagRepository::get_tags_for_series(&state.db, series_id),
        AlternateTitleRepository::get_for_series(&state.db, series_id),
        ExternalRatingRepository::get_for_series(&state.db, series_id),
        ExternalLinkRepository::get_for_series(&state.db, series_id),
    );

    let genres =
        genres_result.map_err(|e| ApiError::Internal(format!("Failed to fetch genres: {}", e)))?;
    let tags =
        tags_result.map_err(|e| ApiError::Internal(format!("Failed to fetch tags: {}", e)))?;
    let alt_titles = alt_titles_result
        .map_err(|e| ApiError::Internal(format!("Failed to fetch alternate titles: {}", e)))?;
    let ext_ratings = ext_ratings_result
        .map_err(|e| ApiError::Internal(format!("Failed to fetch external ratings: {}", e)))?;
    let ext_links = ext_links_result
        .map_err(|e| ApiError::Internal(format!("Failed to fetch external links: {}", e)))?;

    // Convert to DTOs
    let genre_dtos: Vec<GenreDto> = genres
        .into_iter()
        .map(|g| GenreDto {
            id: g.id,
            name: g.name,
            series_count: None,
            created_at: g.created_at,
        })
        .collect();

    let tag_dtos: Vec<TagDto> = tags
        .into_iter()
        .map(|t| TagDto {
            id: t.id,
            name: t.name,
            series_count: None,
            created_at: t.created_at,
        })
        .collect();

    let alt_title_dtos: Vec<AlternateTitleDto> = alt_titles
        .into_iter()
        .map(|at| AlternateTitleDto {
            id: at.id,
            series_id: at.series_id,
            label: at.label,
            title: at.title,
            created_at: at.created_at,
            updated_at: at.updated_at,
        })
        .collect();

    let ext_rating_dtos: Vec<ExternalRatingDto> = ext_ratings
        .into_iter()
        .map(|er| {
            use sea_orm::prelude::Decimal;
            ExternalRatingDto {
                id: er.id,
                series_id: er.series_id,
                source_name: er.source_name,
                rating: Decimal::to_string(&er.rating).parse::<f64>().unwrap_or(0.0),
                vote_count: er.vote_count,
                fetched_at: er.fetched_at,
                created_at: er.created_at,
                updated_at: er.updated_at,
            }
        })
        .collect();

    let ext_link_dtos: Vec<ExternalLinkDto> = ext_links
        .into_iter()
        .map(|el| ExternalLinkDto {
            id: el.id,
            series_id: el.series_id,
            source_name: el.source_name,
            url: el.url,
            external_id: el.external_id,
            created_at: el.created_at,
            updated_at: el.updated_at,
        })
        .collect();

    Ok(Json(FullSeriesMetadataResponse {
        series_id,
        title: metadata.title,
        title_sort: metadata.title_sort,
        summary: metadata.summary,
        publisher: metadata.publisher,
        imprint: metadata.imprint,
        status: metadata.status,
        age_rating: metadata.age_rating,
        language: metadata.language,
        reading_direction: metadata.reading_direction,
        year: metadata.year,
        total_book_count: metadata.total_book_count,
        custom_metadata: parse_custom_metadata(metadata.custom_metadata.as_deref()),
        locks: MetadataLocks {
            title: metadata.title_lock,
            title_sort: metadata.title_sort_lock,
            summary: metadata.summary_lock,
            publisher: metadata.publisher_lock,
            imprint: metadata.imprint_lock,
            status: metadata.status_lock,
            age_rating: metadata.age_rating_lock,
            language: metadata.language_lock,
            reading_direction: metadata.reading_direction_lock,
            year: metadata.year_lock,
            total_book_count: metadata.total_book_count_lock,
            genres: metadata.genres_lock,
            tags: metadata.tags_lock,
            custom_metadata: metadata.custom_metadata_lock,
            cover: metadata.cover_lock,
        },
        genres: genre_dtos,
        tags: tag_dtos,
        alternate_titles: alt_title_dtos,
        external_ratings: ext_rating_dtos,
        external_links: ext_link_dtos,
        created_at: metadata.created_at,
        updated_at: metadata.updated_at,
    }))
}

/// Update metadata lock states
///
/// Sets which metadata fields are locked. Locked fields will not be overwritten
/// by automatic metadata refresh from book analysis or external sources.
#[utoipa::path(
    put,
    path = "/api/v1/series/{series_id}/metadata/locks",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = UpdateMetadataLocksRequest,
    responses(
        (status = 200, description = "Lock states updated", body = MetadataLocks),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn update_metadata_locks(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<UpdateMetadataLocksRequest>,
) -> Result<Json<MetadataLocks>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get existing metadata
    let existing = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series metadata: {}", e)))?
        .ok_or_else(|| ApiError::Internal("Series metadata not found".to_string()))?;

    // Update locks
    use sea_orm::{ActiveModelTrait, Set};
    let mut active: series_metadata::ActiveModel = existing.into();
    let mut has_changes = false;

    if let Some(v) = request.title {
        active.title_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.title_sort {
        active.title_sort_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.summary {
        active.summary_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.publisher {
        active.publisher_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.imprint {
        active.imprint_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.status {
        active.status_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.age_rating {
        active.age_rating_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.language {
        active.language_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.reading_direction {
        active.reading_direction_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.year {
        active.year_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.total_book_count {
        active.total_book_count_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.genres {
        active.genres_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.tags {
        active.tags_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.custom_metadata {
        active.custom_metadata_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.cover {
        active.cover_lock = Set(v);
        has_changes = true;
    }

    let updated = if has_changes {
        active.updated_at = Set(Utc::now());
        active
            .update(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update locks: {}", e)))?
    } else {
        // No changes, fetch current state
        SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch metadata: {}", e)))?
            .ok_or_else(|| ApiError::Internal("Series metadata not found".to_string()))?
    };

    Ok(Json(MetadataLocks {
        title: updated.title_lock,
        title_sort: updated.title_sort_lock,
        summary: updated.summary_lock,
        publisher: updated.publisher_lock,
        imprint: updated.imprint_lock,
        status: updated.status_lock,
        age_rating: updated.age_rating_lock,
        language: updated.language_lock,
        reading_direction: updated.reading_direction_lock,
        year: updated.year_lock,
        total_book_count: updated.total_book_count_lock,
        genres: updated.genres_lock,
        tags: updated.tags_lock,
        custom_metadata: updated.custom_metadata_lock,
        cover: updated.cover_lock,
    }))
}

/// Get metadata lock states
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/metadata/locks",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "Current lock states", body = MetadataLocks),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn get_metadata_locks(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<MetadataLocks>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get metadata
    let metadata = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series metadata: {}", e)))?
        .ok_or_else(|| ApiError::Internal("Series metadata not found".to_string()))?;

    Ok(Json(MetadataLocks {
        title: metadata.title_lock,
        title_sort: metadata.title_sort_lock,
        summary: metadata.summary_lock,
        publisher: metadata.publisher_lock,
        imprint: metadata.imprint_lock,
        status: metadata.status_lock,
        age_rating: metadata.age_rating_lock,
        language: metadata.language_lock,
        reading_direction: metadata.reading_direction_lock,
        year: metadata.year_lock,
        total_book_count: metadata.total_book_count_lock,
        genres: metadata.genres_lock,
        tags: metadata.tags_lock,
        custom_metadata: metadata.custom_metadata_lock,
        cover: metadata.cover_lock,
    }))
}

/// Sanitize a string for use as a filename
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

// ============================================================================
// Genre Handlers
// ============================================================================

/// Query parameters for listing genres
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct GenreListParams {
    /// Page number (1-indexed, default 1)
    #[serde(default = "genre_default_page")]
    pub page: u64,

    /// Number of items per page (default 50, max 500)
    #[serde(default = "genre_default_page_size")]
    pub page_size: u64,
}

fn genre_default_page() -> u64 {
    DEFAULT_PAGE
}

fn genre_default_page_size() -> u64 {
    DEFAULT_PAGE_SIZE
}

/// List all genres
#[utoipa::path(
    get,
    path = "/api/v1/genres",
    params(GenreListParams),
    responses(
        (status = 200, description = "List of all genres", body = PaginatedResponse<GenreDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Genres"
)]
pub async fn list_genres(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(params): Query<GenreListParams>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Validate and clamp pagination params
    let page = params.page.max(1);
    let page_size = params.page_size.clamp(1, MAX_PAGE_SIZE);

    let genres = GenreRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch genres: {}", e)))?;

    let total = genres.len() as u64;
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };

    // Apply in-memory pagination
    let offset = (page - 1) * page_size;
    let paginated_genres: Vec<_> = genres
        .into_iter()
        .skip(offset as usize)
        .take(page_size as usize)
        .collect();

    let mut dtos: Vec<GenreDto> = Vec::with_capacity(paginated_genres.len());
    for g in paginated_genres {
        let count = GenreRepository::count_series_with_genre(&state.db, g.id)
            .await
            .ok();
        dtos.push(GenreDto {
            id: g.id,
            name: g.name,
            series_count: count,
            created_at: g.created_at,
        });
    }

    // Build pagination links
    let link_builder = PaginationLinkBuilder::new("/api/v1/genres", page, page_size, total_pages);

    let response = PaginatedResponse::with_builder(dtos, page, page_size, total, &link_builder);

    Ok(paginated_response(response, &link_builder))
}

/// Get genres for a series
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/genres",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "List of genres for the series", body = GenreListResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Genres"
)]
pub async fn get_series_genres(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<GenreListResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let genres = GenreRepository::get_genres_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch genres: {}", e)))?;

    let dtos: Vec<GenreDto> = genres
        .into_iter()
        .map(|g| GenreDto {
            id: g.id,
            name: g.name,
            series_count: None, // Don't include count for series-specific query
            created_at: g.created_at,
        })
        .collect();

    Ok(Json(GenreListResponse { genres: dtos }))
}

/// Set genres for a series (replaces existing)
#[utoipa::path(
    put,
    path = "/api/v1/series/{series_id}/genres",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = SetSeriesGenresRequest,
    responses(
        (status = 200, description = "Genres updated", body = GenreListResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Genres"
)]
pub async fn set_series_genres(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<SetSeriesGenresRequest>,
) -> Result<Json<GenreListResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let genres = GenreRepository::set_genres_for_series(&state.db, series_id, request.genres)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to set genres: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["genres".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    let dtos: Vec<GenreDto> = genres
        .into_iter()
        .map(|g| GenreDto {
            id: g.id,
            name: g.name,
            series_count: None,
            created_at: g.created_at,
        })
        .collect();

    Ok(Json(GenreListResponse { genres: dtos }))
}

// ============================================================================
// Tag Handlers
// ============================================================================

/// Query parameters for listing tags
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(rename_all = "camelCase")]
pub struct TagListParams {
    /// Page number (1-indexed, default 1)
    #[serde(default = "tag_default_page")]
    pub page: u64,

    /// Number of items per page (default 50, max 500)
    #[serde(default = "tag_default_page_size")]
    pub page_size: u64,
}

fn tag_default_page() -> u64 {
    DEFAULT_PAGE
}

fn tag_default_page_size() -> u64 {
    DEFAULT_PAGE_SIZE
}

/// List all tags
#[utoipa::path(
    get,
    path = "/api/v1/tags",
    params(TagListParams),
    responses(
        (status = 200, description = "List of all tags", body = PaginatedResponse<TagDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Tags"
)]
pub async fn list_tags(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(params): Query<TagListParams>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Validate and clamp pagination params
    let page = params.page.max(1);
    let page_size = params.page_size.clamp(1, MAX_PAGE_SIZE);

    let tags = TagRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch tags: {}", e)))?;

    let total = tags.len() as u64;
    let total_pages = if page_size == 0 {
        0
    } else {
        total.div_ceil(page_size)
    };

    // Apply in-memory pagination
    let offset = (page - 1) * page_size;
    let paginated_tags: Vec<_> = tags
        .into_iter()
        .skip(offset as usize)
        .take(page_size as usize)
        .collect();

    let mut dtos: Vec<TagDto> = Vec::with_capacity(paginated_tags.len());
    for t in paginated_tags {
        let count = TagRepository::count_series_with_tag(&state.db, t.id)
            .await
            .ok();
        dtos.push(TagDto {
            id: t.id,
            name: t.name,
            series_count: count,
            created_at: t.created_at,
        });
    }

    // Build pagination links
    let link_builder = PaginationLinkBuilder::new("/api/v1/tags", page, page_size, total_pages);

    let response = PaginatedResponse::with_builder(dtos, page, page_size, total, &link_builder);

    Ok(paginated_response(response, &link_builder))
}

/// Get tags for a series
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/tags",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "List of tags for the series", body = TagListResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Tags"
)]
pub async fn get_series_tags(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<TagListResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let tags = TagRepository::get_tags_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch tags: {}", e)))?;

    let dtos: Vec<TagDto> = tags
        .into_iter()
        .map(|t| TagDto {
            id: t.id,
            name: t.name,
            series_count: None, // Don't include count for series-specific query
            created_at: t.created_at,
        })
        .collect();

    Ok(Json(TagListResponse { tags: dtos }))
}

/// Set tags for a series (replaces existing)
#[utoipa::path(
    put,
    path = "/api/v1/series/{series_id}/tags",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = SetSeriesTagsRequest,
    responses(
        (status = 200, description = "Tags updated", body = TagListResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Tags"
)]
pub async fn set_series_tags(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<SetSeriesTagsRequest>,
) -> Result<Json<TagListResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let tags = TagRepository::set_tags_for_series(&state.db, series_id, request.tags)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to set tags: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["tags".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    let dtos: Vec<TagDto> = tags
        .into_iter()
        .map(|t| TagDto {
            id: t.id,
            name: t.name,
            series_count: None,
            created_at: t.created_at,
        })
        .collect();

    Ok(Json(TagListResponse { tags: dtos }))
}

/// Add a single genre to a series
#[utoipa::path(
    post,
    path = "/api/v1/series/{series_id}/genres",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = AddSeriesGenreRequest,
    responses(
        (status = 200, description = "Genre added", body = GenreDto),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Genres"
)]
pub async fn add_series_genre(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<AddSeriesGenreRequest>,
) -> Result<Json<GenreDto>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let genre = GenreRepository::add_genre_to_series(&state.db, series_id, &request.name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to add genre: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["genres".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(GenreDto {
        id: genre.id,
        name: genre.name,
        series_count: None,
        created_at: genre.created_at,
    }))
}

/// Remove a genre from a series
#[utoipa::path(
    delete,
    path = "/api/v1/series/{series_id}/genres/{genre_id}",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        ("genre_id" = Uuid, Path, description = "Genre ID")
    ),
    responses(
        (status = 204, description = "Genre removed from series"),
        (status = 404, description = "Series or genre link not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Genres"
)]
pub async fn remove_series_genre(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((series_id, genre_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let removed = GenreRepository::remove_genre_from_series(&state.db, series_id, genre_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to remove genre: {}", e)))?;

    if !removed {
        return Err(ApiError::NotFound(
            "Genre not linked to this series".to_string(),
        ));
    }

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["genres".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::NO_CONTENT)
}

/// Delete a genre from the taxonomy (admin only)
#[utoipa::path(
    delete,
    path = "/api/v1/genres/{genre_id}",
    params(
        ("genre_id" = Uuid, Path, description = "Genre ID")
    ),
    responses(
        (status = 204, description = "Genre deleted"),
        (status = 404, description = "Genre not found"),
        (status = 403, description = "Forbidden - admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Genres"
)]
pub async fn delete_genre(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(genre_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let deleted = GenreRepository::delete(&state.db, genre_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete genre: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("Genre not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Delete all unused genres (genres with no series linked)
#[utoipa::path(
    post,
    path = "/api/v1/genres/cleanup",
    responses(
        (status = 200, description = "Cleanup completed", body = TaxonomyCleanupResponse),
        (status = 403, description = "Forbidden - admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Genres"
)]
pub async fn cleanup_genres(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<TaxonomyCleanupResponse>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let deleted_names = GenreRepository::delete_unused(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to cleanup genres: {}", e)))?;

    Ok(Json(TaxonomyCleanupResponse {
        deleted_count: deleted_names.len() as u64,
        deleted_names,
    }))
}

/// Add a single tag to a series
#[utoipa::path(
    post,
    path = "/api/v1/series/{series_id}/tags",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = AddSeriesTagRequest,
    responses(
        (status = 200, description = "Tag added", body = TagDto),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Tags"
)]
pub async fn add_series_tag(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<AddSeriesTagRequest>,
) -> Result<Json<TagDto>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let tag = TagRepository::add_tag_to_series(&state.db, series_id, &request.name)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to add tag: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["tags".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(TagDto {
        id: tag.id,
        name: tag.name,
        series_count: None,
        created_at: tag.created_at,
    }))
}

/// Remove a tag from a series
#[utoipa::path(
    delete,
    path = "/api/v1/series/{series_id}/tags/{tag_id}",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        ("tag_id" = Uuid, Path, description = "Tag ID")
    ),
    responses(
        (status = 204, description = "Tag removed from series"),
        (status = 404, description = "Series or tag link not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Tags"
)]
pub async fn remove_series_tag(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((series_id, tag_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let removed = TagRepository::remove_tag_from_series(&state.db, series_id, tag_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to remove tag: {}", e)))?;

    if !removed {
        return Err(ApiError::NotFound(
            "Tag not linked to this series".to_string(),
        ));
    }

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["tags".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::NO_CONTENT)
}

/// Delete a tag from the taxonomy (admin only)
#[utoipa::path(
    delete,
    path = "/api/v1/tags/{tag_id}",
    params(
        ("tag_id" = Uuid, Path, description = "Tag ID")
    ),
    responses(
        (status = 204, description = "Tag deleted"),
        (status = 404, description = "Tag not found"),
        (status = 403, description = "Forbidden - admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Tags"
)]
pub async fn delete_tag(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(tag_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let deleted = TagRepository::delete(&state.db, tag_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete tag: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("Tag not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Delete all unused tags (tags with no series linked)
#[utoipa::path(
    post,
    path = "/api/v1/tags/cleanup",
    responses(
        (status = 200, description = "Cleanup completed", body = TaxonomyCleanupResponse),
        (status = 403, description = "Forbidden - admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Tags"
)]
pub async fn cleanup_tags(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<TaxonomyCleanupResponse>, ApiError> {
    require_permission!(auth, Permission::SystemAdmin)?;

    let deleted_names = TagRepository::delete_unused(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to cleanup tags: {}", e)))?;

    Ok(Json(TaxonomyCleanupResponse {
        deleted_count: deleted_names.len() as u64,
        deleted_names,
    }))
}

// ============================================================================
// User Rating Handlers
// ============================================================================

/// Get the current user's rating for a series
///
/// Returns null if no rating exists (not a 404, since the series exists but has no rating)
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/rating",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "User's rating for the series (null if not rated)", body = Option<UserSeriesRatingDto>),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Ratings"
)]
pub async fn get_series_rating(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<Option<UserSeriesRatingDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let rating =
        UserSeriesRatingRepository::get_by_user_and_series(&state.db, auth.user_id, series_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch rating: {}", e)))?;

    Ok(Json(rating.map(|r| UserSeriesRatingDto {
        id: r.id,
        series_id: r.series_id,
        rating: r.rating,
        notes: r.notes,
        created_at: r.created_at,
        updated_at: r.updated_at,
    })))
}

/// Set (create or update) the current user's rating for a series
#[utoipa::path(
    put,
    path = "/api/v1/series/{series_id}/rating",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = SetUserRatingRequest,
    responses(
        (status = 200, description = "Rating saved", body = UserSeriesRatingDto),
        (status = 400, description = "Invalid rating value"),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Ratings"
)]
pub async fn set_series_rating(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<SetUserRatingRequest>,
) -> Result<Json<UserSeriesRatingDto>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Validate rating range
    if !(1..=100).contains(&request.rating) {
        return Err(ApiError::BadRequest(
            "Rating must be between 1 and 100".to_string(),
        ));
    }

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let rating = UserSeriesRatingRepository::upsert(
        &state.db,
        auth.user_id,
        series_id,
        request.rating,
        request.notes,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to save rating: {}", e)))?;

    Ok(Json(UserSeriesRatingDto {
        id: rating.id,
        series_id: rating.series_id,
        rating: rating.rating,
        notes: rating.notes,
        created_at: rating.created_at,
        updated_at: rating.updated_at,
    }))
}

/// Delete the current user's rating for a series
#[utoipa::path(
    delete,
    path = "/api/v1/series/{series_id}/rating",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 204, description = "Rating deleted"),
        (status = 404, description = "Series or rating not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Ratings"
)]
pub async fn delete_series_rating(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let deleted =
        UserSeriesRatingRepository::delete_by_user_and_series(&state.db, auth.user_id, series_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to delete rating: {}", e)))?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound(
            "No rating found for this series".to_string(),
        ))
    }
}

/// List all of the current user's ratings
#[utoipa::path(
    get,
    path = "/api/v1/user/ratings",
    responses(
        (status = 200, description = "List of user's ratings", body = UserRatingsListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Ratings"
)]
pub async fn list_user_ratings(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<UserRatingsListResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let ratings = UserSeriesRatingRepository::get_all_for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch ratings: {}", e)))?;

    let dtos: Vec<UserSeriesRatingDto> = ratings
        .into_iter()
        .map(|r| UserSeriesRatingDto {
            id: r.id,
            series_id: r.series_id,
            rating: r.rating,
            notes: r.notes,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect();

    Ok(Json(UserRatingsListResponse { ratings: dtos }))
}

// ============================================================================
// Alternate Title Handlers
// ============================================================================

/// Get alternate titles for a series
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/alternate-titles",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "List of alternate titles for the series", body = AlternateTitleListResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn get_series_alternate_titles(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<AlternateTitleListResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let titles = AlternateTitleRepository::get_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch alternate titles: {}", e)))?;

    let dtos: Vec<AlternateTitleDto> = titles
        .into_iter()
        .map(|t| AlternateTitleDto {
            id: t.id,
            series_id: t.series_id,
            label: t.label,
            title: t.title,
            created_at: t.created_at,
            updated_at: t.updated_at,
        })
        .collect();

    Ok(Json(AlternateTitleListResponse { titles: dtos }))
}

/// Add an alternate title to a series
#[utoipa::path(
    post,
    path = "/api/v1/series/{series_id}/alternate-titles",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = CreateAlternateTitleRequest,
    responses(
        (status = 201, description = "Alternate title created", body = AlternateTitleDto),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn create_alternate_title(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<CreateAlternateTitleRequest>,
) -> Result<(StatusCode, Json<AlternateTitleDto>), ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let title =
        AlternateTitleRepository::create(&state.db, series_id, &request.label, &request.title)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to create alternate title: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["alternate_titles".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok((
        StatusCode::CREATED,
        Json(AlternateTitleDto {
            id: title.id,
            series_id: title.series_id,
            label: title.label,
            title: title.title,
            created_at: title.created_at,
            updated_at: title.updated_at,
        }),
    ))
}

/// Update an alternate title
#[utoipa::path(
    patch,
    path = "/api/v1/series/{series_id}/alternate-titles/{title_id}",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        ("title_id" = Uuid, Path, description = "Alternate title ID")
    ),
    request_body = UpdateAlternateTitleRequest,
    responses(
        (status = 200, description = "Alternate title updated", body = AlternateTitleDto),
        (status = 404, description = "Series or title not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn update_alternate_title(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((series_id, title_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateAlternateTitleRequest>,
) -> Result<Json<AlternateTitleDto>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Verify title belongs to series
    if !AlternateTitleRepository::belongs_to_series(&state.db, title_id, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to verify title ownership: {}", e)))?
    {
        return Err(ApiError::NotFound("Alternate title not found".to_string()));
    }

    let title = AlternateTitleRepository::update(
        &state.db,
        title_id,
        request.label.as_deref(),
        request.title.as_deref(),
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to update alternate title: {}", e)))?
    .ok_or_else(|| ApiError::NotFound("Alternate title not found".to_string()))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["alternate_titles".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(AlternateTitleDto {
        id: title.id,
        series_id: title.series_id,
        label: title.label,
        title: title.title,
        created_at: title.created_at,
        updated_at: title.updated_at,
    }))
}

/// Delete an alternate title
#[utoipa::path(
    delete,
    path = "/api/v1/series/{series_id}/alternate-titles/{title_id}",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        ("title_id" = Uuid, Path, description = "Alternate title ID")
    ),
    responses(
        (status = 204, description = "Alternate title deleted"),
        (status = 404, description = "Series or title not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn delete_alternate_title(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((series_id, title_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Verify title belongs to series
    if !AlternateTitleRepository::belongs_to_series(&state.db, title_id, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to verify title ownership: {}", e)))?
    {
        return Err(ApiError::NotFound("Alternate title not found".to_string()));
    }

    let deleted = AlternateTitleRepository::delete(&state.db, title_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete alternate title: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("Alternate title not found".to_string()));
    }

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["alternate_titles".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// External Rating Handlers
// ============================================================================

/// Get external ratings for a series
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/external-ratings",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "List of external ratings for the series", body = ExternalRatingListResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn get_series_external_ratings(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<ExternalRatingListResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let ratings = ExternalRatingRepository::get_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch external ratings: {}", e)))?;

    let dtos: Vec<ExternalRatingDto> = ratings
        .into_iter()
        .map(|r| ExternalRatingDto {
            id: r.id,
            series_id: r.series_id,
            source_name: r.source_name,
            rating: r.rating.to_string().parse().unwrap_or(0.0),
            vote_count: r.vote_count,
            fetched_at: r.fetched_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect();

    Ok(Json(ExternalRatingListResponse { ratings: dtos }))
}

/// Add or update an external rating for a series
#[utoipa::path(
    post,
    path = "/api/v1/series/{series_id}/external-ratings",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = CreateExternalRatingRequest,
    responses(
        (status = 200, description = "External rating created or updated", body = ExternalRatingDto),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden - admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn create_external_rating(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<CreateExternalRatingRequest>,
) -> Result<Json<ExternalRatingDto>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    use sea_orm::prelude::Decimal;
    let rating_decimal = Decimal::from_f64_retain(request.rating)
        .ok_or_else(|| ApiError::BadRequest("Invalid rating value".to_string()))?;

    let rating = ExternalRatingRepository::upsert(
        &state.db,
        series_id,
        &request.source_name,
        rating_decimal,
        request.vote_count,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to create external rating: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["external_ratings".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(ExternalRatingDto {
        id: rating.id,
        series_id: rating.series_id,
        source_name: rating.source_name,
        rating: rating.rating.to_string().parse().unwrap_or(0.0),
        vote_count: rating.vote_count,
        fetched_at: rating.fetched_at,
        created_at: rating.created_at,
        updated_at: rating.updated_at,
    }))
}

/// Delete an external rating by source name
#[utoipa::path(
    delete,
    path = "/api/v1/series/{series_id}/external-ratings/{source}",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        ("source" = String, Path, description = "Source name (e.g., 'myanimelist', 'anilist')")
    ),
    responses(
        (status = 204, description = "External rating deleted"),
        (status = 404, description = "Series or rating not found"),
        (status = 403, description = "Forbidden - admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn delete_external_rating(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((series_id, source)): Path<(Uuid, String)>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let deleted = ExternalRatingRepository::delete_by_source(&state.db, series_id, &source)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete external rating: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("External rating not found".to_string()));
    }

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["external_ratings".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Average Rating Handler
// ============================================================================

/// Get the average community rating for a series
///
/// Returns the average rating from all users and the total count of ratings.
/// Ratings are stored on a 0-100 scale internally.
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/ratings/average",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "Average rating for the series", body = SeriesAverageRatingResponse,
            example = json!({"average": 78.5, "count": 15})),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn get_series_average_rating(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<SeriesAverageRatingResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get average rating
    let average = UserSeriesRatingRepository::calculate_average_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to calculate average rating: {}", e)))?;

    // Get count
    let count = UserSeriesRatingRepository::count_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to count ratings: {}", e)))?;

    Ok(Json(SeriesAverageRatingResponse { average, count }))
}

// ============================================================================
// External Link Handlers
// ============================================================================

/// Get external links for a series
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/external-links",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "List of external links for the series", body = ExternalLinkListResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn get_series_external_links(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<ExternalLinkListResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let links = ExternalLinkRepository::get_for_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch external links: {}", e)))?;

    let dtos: Vec<ExternalLinkDto> = links
        .into_iter()
        .map(|l| ExternalLinkDto {
            id: l.id,
            series_id: l.series_id,
            source_name: l.source_name,
            url: l.url,
            external_id: l.external_id,
            created_at: l.created_at,
            updated_at: l.updated_at,
        })
        .collect();

    Ok(Json(ExternalLinkListResponse { links: dtos }))
}

/// Add or update an external link for a series
#[utoipa::path(
    post,
    path = "/api/v1/series/{series_id}/external-links",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    request_body = CreateExternalLinkRequest,
    responses(
        (status = 200, description = "External link created or updated", body = ExternalLinkDto),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden - admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn create_external_link(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Json(request): Json<CreateExternalLinkRequest>,
) -> Result<Json<ExternalLinkDto>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let link = ExternalLinkRepository::upsert(
        &state.db,
        series_id,
        &request.source_name,
        &request.url,
        request.external_id.as_deref(),
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to create external link: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["external_links".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(ExternalLinkDto {
        id: link.id,
        series_id: link.series_id,
        source_name: link.source_name,
        url: link.url,
        external_id: link.external_id,
        created_at: link.created_at,
        updated_at: link.updated_at,
    }))
}

/// Delete an external link by source name
#[utoipa::path(
    delete,
    path = "/api/v1/series/{series_id}/external-links/{source}",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        ("source" = String, Path, description = "Source name (e.g., 'myanimelist', 'mangadex')")
    ),
    responses(
        (status = 204, description = "External link deleted"),
        (status = 404, description = "Series or link not found"),
        (status = 403, description = "Forbidden - admin only"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn delete_external_link(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((series_id, source)): Path<(Uuid, String)>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let deleted = ExternalLinkRepository::delete_by_source(&state.db, series_id, &source)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete external link: {}", e)))?;

    if !deleted {
        return Err(ApiError::NotFound("External link not found".to_string()));
    }

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: series.library_id,
            fields: Some(vec!["external_links".to_string()]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Cover Management Handlers
// ============================================================================

/// List all covers for a series
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/covers",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "List of series covers", body = SeriesCoverListResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn list_series_covers(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<SeriesCoverListResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Fetch all covers
    let covers = SeriesCoversRepository::list_by_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch covers: {}", e)))?;

    let cover_dtos: Vec<SeriesCoverDto> = covers
        .into_iter()
        .map(|c| SeriesCoverDto {
            id: c.id,
            series_id: c.series_id,
            source: c.source,
            path: c.path,
            is_selected: c.is_selected,
            width: c.width,
            height: c.height,
            created_at: c.created_at,
            updated_at: c.updated_at,
        })
        .collect();

    Ok(Json(SeriesCoverListResponse { covers: cover_dtos }))
}

/// Select a cover as the primary cover for a series
#[utoipa::path(
    put,
    path = "/api/v1/series/{series_id}/covers/{cover_id}/select",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        ("cover_id" = Uuid, Path, description = "Cover ID to select")
    ),
    responses(
        (status = 200, description = "Cover selected successfully", body = SeriesCoverDto),
        (status = 404, description = "Series or cover not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn select_series_cover(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((series_id, cover_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SeriesCoverDto>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Select the cover (this also validates the cover belongs to the series)
    let cover = SeriesCoversRepository::select_cover(&state.db, series_id, cover_id)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") || e.to_string().contains("does not belong") {
                ApiError::NotFound(format!("Cover not found: {}", cover_id))
            } else {
                ApiError::Internal(format!("Failed to select cover: {}", e))
            }
        })?;

    // Auto-lock cover to prevent plugins from overwriting user's manual selection
    SeriesMetadataRepository::update_cover_lock(&state.db, series_id, true)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to lock cover: {}", e)))?;

    // Touch series to update updated_at (for cache busting)
    SeriesRepository::touch(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update series timestamp: {}", e)))?;

    // Regenerate the series thumbnail to reflect the new cover
    regenerate_series_thumbnail(&state, series_id).await;

    // Emit cover updated event
    let event = EntityChangeEvent {
        event: EntityEvent::CoverUpdated {
            entity_type: EntityType::Series,
            entity_id: series_id,
            library_id: Some(series.library_id),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(SeriesCoverDto {
        id: cover.id,
        series_id: cover.series_id,
        source: cover.source,
        path: cover.path,
        is_selected: cover.is_selected,
        width: cover.width,
        height: cover.height,
        created_at: cover.created_at,
        updated_at: cover.updated_at,
    }))
}

/// Reset series cover to default (deselect all custom covers)
#[utoipa::path(
    delete,
    path = "/api/v1/series/{series_id}/covers/selected",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 204, description = "Reset to default cover successfully"),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn reset_series_cover(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Deselect all covers (this will make the thumbnail endpoint use the default)
    SeriesCoversRepository::deselect_all(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to reset cover: {}", e)))?;

    // Touch series to update updated_at (for cache busting)
    SeriesRepository::touch(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update series timestamp: {}", e)))?;

    // Regenerate the series thumbnail to use the default cover (first book's cover)
    regenerate_series_thumbnail(&state, series_id).await;

    // Emit cover updated event
    let event = EntityChangeEvent {
        event: EntityEvent::CoverUpdated {
            entity_type: EntityType::Series,
            entity_id: series_id,
            library_id: Some(series.library_id),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::NO_CONTENT)
}

/// Delete a cover from a series
#[utoipa::path(
    delete,
    path = "/api/v1/series/{series_id}/covers/{cover_id}",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        ("cover_id" = Uuid, Path, description = "Cover ID to delete")
    ),
    responses(
        (status = 204, description = "Cover deleted successfully"),
        (status = 404, description = "Series or cover not found"),
        (status = 400, description = "Cannot delete the only selected cover"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn delete_series_cover(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path((series_id, cover_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists and get library_id
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get the cover to verify it exists and belongs to this series
    let cover = SeriesCoversRepository::get_by_id(&state.db, cover_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch cover: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Cover not found".to_string()))?;

    if cover.series_id != series_id {
        return Err(ApiError::NotFound("Cover not found".to_string()));
    }

    // If this is the selected cover, we need to select another one (if available)
    if cover.is_selected {
        // Get all covers for this series
        let all_covers = SeriesCoversRepository::list_by_series(&state.db, series_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to list covers: {}", e)))?;

        // Find another cover to select
        let alternate = all_covers.iter().find(|c| c.id != cover_id);
        if let Some(alt_cover) = alternate {
            SeriesCoversRepository::select_cover(&state.db, series_id, alt_cover.id)
                .await
                .map_err(|e| {
                    ApiError::Internal(format!("Failed to select alternate cover: {}", e))
                })?;
        }
        // If there's no alternate cover, we just delete this one
    }

    // If this is a custom cover, delete the file as well
    if cover.source == "custom" {
        let path = std::path::Path::new(&cover.path);
        if path.exists() {
            let _ = fs::remove_file(path).await;
        }
    }

    // Delete the cover record
    SeriesCoversRepository::delete(&state.db, cover_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete cover: {}", e)))?;

    // Touch series to update updated_at (for cache busting)
    SeriesRepository::touch(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update series timestamp: {}", e)))?;

    // Regenerate the series thumbnail (will use alternate cover or default)
    if cover.is_selected {
        regenerate_series_thumbnail(&state, series_id).await;
    }

    // Emit cover updated event
    let event = EntityChangeEvent {
        event: EntityEvent::CoverUpdated {
            entity_type: EntityType::Series,
            entity_id: series_id,
            library_id: Some(series.library_id),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::NO_CONTENT)
}

/// Get a specific cover image for a series
///
/// Supports HTTP conditional caching with ETag and Last-Modified headers,
/// returning 304 Not Modified when the client has a valid cached copy.
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/covers/{cover_id}/image",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        ("cover_id" = Uuid, Path, description = "Cover ID")
    ),
    responses(
        (status = 200, description = "Cover image", content_type = "image/jpeg"),
        (status = 304, description = "Not modified (client cache is valid)"),
        (status = 404, description = "Series or cover not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Series"
)]
pub async fn get_series_cover_image(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    headers: HeaderMap,
    Path((series_id, cover_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    let _series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get the cover
    let cover = SeriesCoversRepository::get_by_id(&state.db, cover_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch cover: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Cover not found".to_string()))?;

    // Verify cover belongs to this series
    if cover.series_id != series_id {
        return Err(ApiError::NotFound("Cover not found".to_string()));
    }

    // Get file metadata for conditional caching
    let metadata = fs::metadata(&cover.path).await.map_err(|e| {
        ApiError::Internal(format!(
            "Failed to read cover metadata from {}: {}",
            cover.path, e
        ))
    })?;

    let size = metadata.len();
    let modified_unix = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Generate ETag from cover_id + size + modified time
    let etag = format!(
        "\"{:x}-{:x}-{:x}\"",
        cover_id.as_u128(),
        size,
        modified_unix
    );

    // Check If-None-Match header for ETag validation
    if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH) {
        if let Ok(client_etag) = if_none_match.to_str() {
            let client_etag = client_etag.trim().trim_start_matches("W/");
            if client_etag == etag || client_etag.trim_matches('"') == etag.trim_matches('"') {
                return Ok(Response::builder()
                    .status(StatusCode::NOT_MODIFIED)
                    .header(header::ETAG, &etag)
                    .header(header::CACHE_CONTROL, "public, max-age=31536000")
                    .body(Body::empty())
                    .unwrap());
            }
        }
    }

    // Check If-Modified-Since header
    if let Some(if_modified_since) = headers.get(header::IF_MODIFIED_SINCE) {
        if let Ok(date_str) = if_modified_since.to_str() {
            if let Ok(client_time) = httpdate::parse_http_date(date_str) {
                let file_time = UNIX_EPOCH + Duration::from_secs(modified_unix);
                if file_time <= client_time {
                    return Ok(Response::builder()
                        .status(StatusCode::NOT_MODIFIED)
                        .header(header::ETAG, &etag)
                        .header(header::CACHE_CONTROL, "public, max-age=31536000")
                        .body(Body::empty())
                        .unwrap());
                }
            }
        }
    }

    // Stream the cover file directly
    let file = tokio::fs::File::open(&cover.path).await.map_err(|e| {
        ApiError::Internal(format!("Failed to open cover from {}: {}", cover.path, e))
    })?;
    let stream = ReaderStream::new(file);

    let last_modified = UNIX_EPOCH + Duration::from_secs(modified_unix);
    let last_modified_str = fmt_http_date(last_modified);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(header::CONTENT_LENGTH, size)
        .header(header::ETAG, &etag)
        .header(header::LAST_MODIFIED, last_modified_str)
        .header(header::CACHE_CONTROL, "public, max-age=31536000")
        .body(Body::from_stream(stream))
        .unwrap())
}

/// Regenerate the series thumbnail by deleting the cache and queuing a new generation task.
///
/// This should be called whenever a series cover is selected/unselected to ensure
/// the cached thumbnail reflects the current cover selection.
async fn regenerate_series_thumbnail(state: &AuthState, series_id: Uuid) {
    use crate::db::repositories::TaskRepository;
    use crate::tasks::types::TaskType;

    // Delete the cached series thumbnail first
    if let Err(e) = state
        .thumbnail_service
        .delete_series_thumbnail(series_id)
        .await
    {
        tracing::warn!(
            "Failed to delete series thumbnail cache for {}: {}",
            series_id,
            e
        );
    }

    // Queue a task to regenerate the thumbnail with force=true
    let task_type = TaskType::GenerateSeriesThumbnail {
        series_id,
        force: true, // Force regeneration since we just deleted the cache
    };

    if let Err(e) = TaskRepository::enqueue(&state.db, task_type, 0, None).await {
        tracing::warn!(
            "Failed to queue series thumbnail regeneration task for {}: {}",
            series_id,
            e
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_basic() {
        assert_eq!(sanitize_filename("My Series"), "My Series");
        assert_eq!(sanitize_filename("Volume 1"), "Volume 1");
    }

    #[test]
    fn test_sanitize_filename_special_chars() {
        assert_eq!(sanitize_filename("Series: Part 1"), "Series_ Part 1");
        assert_eq!(sanitize_filename("What?"), "What_");
        assert_eq!(sanitize_filename("A/B\\C"), "A_B_C");
        assert_eq!(sanitize_filename("Test*File"), "Test_File");
        assert_eq!(sanitize_filename("\"Quoted\""), "_Quoted_");
        assert_eq!(sanitize_filename("<tag>"), "_tag_");
        assert_eq!(sanitize_filename("A|B"), "A_B");
    }

    #[test]
    fn test_sanitize_filename_trims_whitespace() {
        assert_eq!(sanitize_filename("  My Series  "), "My Series");
        assert_eq!(sanitize_filename("   "), "");
    }

    #[test]
    fn test_sanitize_filename_control_chars() {
        assert_eq!(sanitize_filename("Test\x00Name"), "Test_Name");
        assert_eq!(sanitize_filename("Line\nBreak"), "Line_Break");
    }
}
