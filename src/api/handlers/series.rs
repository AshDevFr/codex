use crate::api::{
    dto::{
        series::{
            AddSeriesGenreRequest, AddSeriesTagRequest, AlternateTitleDto,
            AlternateTitleListResponse, CreateAlternateTitleRequest, CreateExternalLinkRequest,
            CreateExternalRatingRequest, ExternalLinkDto, ExternalLinkListResponse,
            ExternalRatingDto, ExternalRatingListResponse, FullSeriesMetadataResponse, GenreDto,
            GenreListResponse, MetadataLocks, PatchSeriesMetadataRequest,
            ReplaceSeriesMetadataRequest, SeriesCoverDto, SeriesCoverListResponse,
            SeriesMetadataResponse, SeriesSortParam, SetSeriesGenresRequest, SetSeriesTagsRequest,
            SetUserRatingRequest, TagDto, TagListResponse, TaxonomyCleanupResponse,
            UpdateAlternateTitleRequest, UpdateMetadataLocksRequest, UserRatingsListResponse,
            UserSeriesRatingDto,
        },
        BookDto, MarkReadResponse, SearchSeriesRequest, SeriesDto, SeriesListRequest,
        SeriesListResponse,
    },
    error::ApiError,
    extractors::{AuthContext, AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::entities::{series, series_metadata};
use crate::db::repositories::{
    AlternateTitleRepository, BookRepository, ExternalLinkRepository, ExternalRatingRepository,
    GenreRepository, ReadProgressRepository, SeriesCoversRepository, SeriesMetadataRepository,
    SeriesRepository, TagRepository, UserSeriesRatingRepository,
};
use crate::events::{EntityChangeEvent, EntityEvent, EntityType};
use crate::require_permission;
use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use chrono::Utc;
use image::{imageops::FilterType, ImageFormat};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use std::io::{Cursor, Write};
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

/// Query parameters for listing books in a series
#[derive(Debug, Deserialize)]
pub struct ListBooksQuery {
    /// Include deleted books in the result
    #[serde(default)]
    pub include_deleted: bool,
}

/// Query parameters for listing series
#[derive(Debug, Deserialize)]
pub struct SeriesListQuery {
    /// Page number (0-indexed)
    #[serde(default)]
    pub page: u64,

    /// Number of items per page (max 100)
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
}

fn default_page_size() -> u64 {
    20
}

/// Helper function to convert series model to DTO with unread count
/// Fetches metadata and cover info from related tables
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

    // Fetch metadata from series_metadata table
    let metadata = SeriesMetadataRepository::get_by_series_id(db, series.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series metadata: {:?}", e)))?;

    // Fetch cover info from series_covers table
    let selected_cover = SeriesCoversRepository::get_selected(db, series.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series cover: {:?}", e)))?;

    let has_custom_cover = SeriesCoversRepository::has_custom_cover(db, series.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check custom cover: {:?}", e)))?;

    Ok(SeriesDto {
        id: series.id,
        library_id: series.library_id,
        name: series.name,
        sort_name: metadata.as_ref().and_then(|m| m.title_sort.clone()),
        description: metadata.as_ref().and_then(|m| m.summary.clone()),
        publisher: metadata.as_ref().and_then(|m| m.publisher.clone()),
        year: metadata.as_ref().and_then(|m| m.year),
        book_count: series.book_count as i64,
        path: series.path,
        selected_cover_source: selected_cover.map(|c| c.source),
        has_custom_cover: Some(has_custom_cover),
        unread_count,
        created_at: series.created_at,
        updated_at: series.updated_at,
    })
}

/// List series with optional library filter and pagination
#[utoipa::path(
    get,
    path = "/api/v1/series",
    params(
        ("library_id" = Option<Uuid>, Query, description = "Filter by library ID"),
        ("page" = Option<u64>, Query, description = "Page number (0-indexed)"),
        ("page_size" = Option<u64>, Query, description = "Number of items per page (max 100)"),
        ("sort" = Option<String>, Query, description = "Sort parameter (format: 'field,direction')"),
        ("genres" = Option<String>, Query, description = "Filter by genres (comma-separated, AND logic)"),
        ("tags" = Option<String>, Query, description = "Filter by tags (comma-separated, AND logic)")
    ),
    responses(
        (status = 200, description = "Paginated list of series", body = SeriesListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn list_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<SeriesListQuery>,
) -> Result<Json<SeriesListResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Validate and normalize pagination params
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(100)
    };

    // Fetch series based on filter (all libraries or specific library)
    let mut series_list = if let Some(library_id) = query.library_id {
        SeriesRepository::list_by_library(&state.db, library_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
    } else {
        SeriesRepository::list_all(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
    };

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

            series_list.retain(|s| matching_series_ids.contains(&s.id));
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

            series_list.retain(|s| matching_series_ids.contains(&s.id));
        }
    }

    // Apply sorting if specified
    if let Some(sort_param) = &query.sort {
        apply_series_sorting(&mut series_list, sort_param);
    }

    let total = series_list.len() as u64;

    // Apply pagination manually
    let offset = query.page * page_size;
    let start = offset as usize;

    // If start is beyond the list, return empty results
    if start >= series_list.len() {
        return Ok(Json(SeriesListResponse::new(
            vec![],
            query.page,
            page_size,
            total,
        )));
    }

    let end = (start + page_size as usize).min(series_list.len());
    let paginated = series_list[start..end].to_vec();

    let user_id = Some(auth.user_id);
    let dtos: Vec<SeriesDto> = futures::future::join_all(
        paginated
            .into_iter()
            .map(|series| series_to_dto(&state.db, series, user_id)),
    )
    .await
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| ApiError::Internal(format!("Failed to build series DTOs: {:?}", e)))?;

    let response = SeriesListResponse::new(dtos, query.page, page_size, total);

    Ok(Json(response))
}

/// Get series by ID
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "Series details", body = SeriesDto),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn get_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<SeriesDto>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let user_id = Some(auth.user_id);
    let dto = series_to_dto(&state.db, series, user_id).await?;

    Ok(Json(dto))
}

/// Search series by name
#[utoipa::path(
    post,
    path = "/api/v1/series/search",
    request_body = SearchSeriesRequest,
    responses(
        (status = 200, description = "Search results", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn search_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<SearchSeriesRequest>,
) -> Result<Json<Vec<SeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series_list = SeriesRepository::search_by_name(&state.db, &request.query)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to search series: {}", e)))?;

    // Filter by library if specified
    let filtered: Vec<_> = if let Some(lib_id) = request.library_id {
        series_list
            .into_iter()
            .filter(|s| s.library_id == lib_id)
            .collect()
    } else {
        series_list
    };

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

    Ok(Json(dtos))
}

/// List series with advanced filtering
///
/// Supports complex filter conditions including nested AllOf/AnyOf logic,
/// genre/tag filtering with include/exclude, and more.
#[utoipa::path(
    post,
    path = "/api/v1/series/list",
    request_body = SeriesListRequest,
    responses(
        (status = 200, description = "Paginated list of filtered series", body = SeriesListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn list_series_filtered(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<SeriesListRequest>,
) -> Result<Json<SeriesListResponse>, ApiError> {
    use crate::services::FilterService;
    use std::collections::HashSet;

    require_permission!(auth, Permission::SeriesRead)?;

    // Validate and normalize pagination params
    let page_size = if request.page_size == 0 {
        default_page_size()
    } else {
        request.page_size.min(100)
    };

    // Get all series IDs first (we'll filter from this)
    let all_series = SeriesRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

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

    // Apply full-text search if provided
    let mut filtered_series: Vec<_> = if let Some(ref search_query) = request.full_text_search {
        if !search_query.trim().is_empty() {
            // Use full-text search with candidate filtering
            let candidate_ids: Vec<Uuid> = matching_ids.iter().cloned().collect();
            SeriesRepository::full_text_search_filtered(&state.db, search_query, &candidate_ids)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to search series: {}", e)))?
        } else {
            // Empty search query, use condition-filtered results
            all_series
                .into_iter()
                .filter(|s| matching_ids.contains(&s.id))
                .collect()
        }
    } else {
        // No full-text search, use condition-filtered results
        all_series
            .into_iter()
            .filter(|s| matching_ids.contains(&s.id))
            .collect()
    };

    // Apply sorting if specified
    if let Some(ref sort_param) = request.sort {
        apply_series_sorting(&mut filtered_series, sort_param);
    }

    let total = filtered_series.len() as u64;

    // Apply pagination
    let offset = request.page * page_size;
    let start = offset as usize;

    if start >= filtered_series.len() {
        return Ok(Json(SeriesListResponse::new(
            vec![],
            request.page,
            page_size,
            total,
        )));
    }

    let end = (start + page_size as usize).min(filtered_series.len());
    let paginated = filtered_series[start..end].to_vec();

    // Convert to DTOs
    let user_id = Some(auth.user_id);
    let dtos: Vec<SeriesDto> = futures::future::join_all(
        paginated
            .into_iter()
            .map(|series| series_to_dto(&state.db, series, user_id)),
    )
    .await
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| ApiError::Internal(format!("Failed to build series DTOs: {:?}", e)))?;

    let response = SeriesListResponse::new(dtos, request.page, page_size, total);

    Ok(Json(response))
}

/// Get books in a series
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/books",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        ("include_deleted" = Option<bool>, Query, description = "Include deleted books (default: false)")
    ),
    responses(
        (status = 200, description = "List of books in the series", body = Vec<BookDto>),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn get_series_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Query(query): Query<ListBooksQuery>,
) -> Result<Json<Vec<BookDto>>, ApiError> {
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

    // Convert to DTOs using helper function
    let dtos = crate::api::handlers::books::books_to_dtos(&state.db, auth.user_id, books).await?;

    Ok(Json(dtos))
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
    tag = "series"
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
    tag = "series"
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

    // Create covers directory within uploads dir if it doesn't exist
    let covers_dir = state.thumbnail_service.get_uploads_dir().join("covers");
    fs::create_dir_all(&covers_dir)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create covers directory: {}", e)))?;

    // Save the image with a unique filename
    let filename = format!("{}.jpg", series_id);
    let filepath = covers_dir.join(&filename);

    let mut file = fs::File::create(&filepath)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create cover file: {}", e)))?;

    file.write_all(&image_data)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to write cover file: {}", e)))?;

    // Check if a custom cover already exists and update or create accordingly
    let existing_custom = SeriesCoversRepository::get_by_source(&state.db, series_id, "custom")
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to check existing cover: {}", e)))?;

    if let Some(existing) = existing_custom {
        // Update existing custom cover
        SeriesCoversRepository::update_path(&state.db, existing.id, &filepath.to_string_lossy())
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update cover path: {}", e)))?;

        // Select this cover
        SeriesCoversRepository::select_cover(&state.db, series_id, existing.id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to select cover: {}", e)))?;
    } else {
        // Create new custom cover and select it
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
    tag = "series"
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
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
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

    // Get the selected cover from series_covers table
    let selected_cover = SeriesCoversRepository::get_selected(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch cover: {}", e)))?;

    // Determine which cover to use based on the selected cover
    let image_data = if let Some(cover) = selected_cover {
        // Use the selected cover's path
        fs::read(&cover.path).await.map_err(|e| {
            ApiError::Internal(format!("Failed to read cover from {}: {}", cover.path, e))
        })?
    } else {
        // No selected cover, use default (first book's cover)
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

/// Query parameters for in-progress series
#[derive(Debug, Deserialize)]
pub struct InProgressSeriesQuery {
    /// Filter by library ID (optional)
    #[serde(default)]
    pub library_id: Option<Uuid>,
}

/// List series with in-progress books (series that have at least one book with reading progress that is not completed)
#[utoipa::path(
    get,
    path = "/api/v1/series/in-progress",
    params(
        ("library_id" = Option<Uuid>, Query, description = "Filter by library ID")
    ),
    responses(
        (status = 200, description = "List of in-progress series", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn list_in_progress_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<InProgressSeriesQuery>,
) -> Result<Json<Vec<SeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Fetch in-progress series for the current user
    let series_list = SeriesRepository::list_in_progress(&state.db, auth.user_id, query.library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch in-progress series: {}", e)))?;

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

    Ok(Json(dtos))
}

/// Query parameters for recently added/updated series
#[derive(Debug, Deserialize)]
pub struct RecentSeriesQuery {
    /// Maximum number of series to return (default: 50)
    #[serde(default = "default_recent_limit")]
    pub limit: u64,

    /// Filter by library ID (optional)
    #[serde(default)]
    pub library_id: Option<Uuid>,
}

fn default_recent_limit() -> u64 {
    50
}

/// List recently added series
#[utoipa::path(
    get,
    path = "/api/v1/series/recently-added",
    params(
        ("limit" = Option<u64>, Query, description = "Maximum number of series to return (default: 50)"),
        ("library_id" = Option<Uuid>, Query, description = "Filter by library ID")
    ),
    responses(
        (status = 200, description = "List of recently added series", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn list_recently_added_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<RecentSeriesQuery>,
) -> Result<Json<Vec<SeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series_list =
        SeriesRepository::list_recently_added(&state.db, query.library_id, query.limit)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch recently added series: {}", e))
            })?;

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

    Ok(Json(dtos))
}

/// List recently added series in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/series/recently-added",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        ("limit" = Option<u64>, Query, description = "Maximum number of series to return (default: 50)")
    ),
    responses(
        (status = 200, description = "List of recently added series in library", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn list_library_recently_added_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<RecentSeriesQuery>,
) -> Result<Json<Vec<SeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series_list =
        SeriesRepository::list_recently_added(&state.db, Some(library_id), query.limit)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch recently added series: {}", e))
            })?;

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

    Ok(Json(dtos))
}

/// List recently updated series
#[utoipa::path(
    get,
    path = "/api/v1/series/recently-updated",
    params(
        ("limit" = Option<u64>, Query, description = "Maximum number of series to return (default: 50)"),
        ("library_id" = Option<Uuid>, Query, description = "Filter by library ID")
    ),
    responses(
        (status = 200, description = "List of recently updated series", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn list_recently_updated_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<RecentSeriesQuery>,
) -> Result<Json<Vec<SeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series_list =
        SeriesRepository::list_recently_updated(&state.db, query.library_id, query.limit)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch recently updated series: {}", e))
            })?;

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

    Ok(Json(dtos))
}

/// List recently updated series in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/series/recently-updated",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        ("limit" = Option<u64>, Query, description = "Maximum number of series to return (default: 50)")
    ),
    responses(
        (status = 200, description = "List of recently updated series in library", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn list_library_recently_updated_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<RecentSeriesQuery>,
) -> Result<Json<Vec<SeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series_list =
        SeriesRepository::list_recently_updated(&state.db, Some(library_id), query.limit)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch recently updated series: {}", e))
            })?;

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

    Ok(Json(dtos))
}

/// List series in a specific library with pagination
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/series",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        ("page" = Option<u64>, Query, description = "Page number (0-indexed)"),
        ("page_size" = Option<u64>, Query, description = "Number of items per page (max 100)")
    ),
    responses(
        (status = 200, description = "Paginated list of series in library", body = SeriesListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn list_library_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<SeriesListQuery>,
) -> Result<Json<SeriesListResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Validate and normalize pagination params
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(100)
    };

    // Parse sort parameter
    let sort = query
        .sort
        .as_ref()
        .map(|s| SeriesSortParam::parse(s))
        .unwrap_or_default();

    // Get total count for pagination
    let total = SeriesRepository::count_by_library(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to count series: {}", e)))?
        as u64;

    // Fetch sorted and paginated series
    let offset = query.page * page_size;
    let user_id = Some(auth.user_id);

    let series_list = SeriesRepository::list_by_library_sorted(
        &state.db, library_id, &sort, user_id, offset, page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    let dtos: Vec<SeriesDto> = futures::future::join_all(
        series_list
            .into_iter()
            .map(|series| series_to_dto(&state.db, series, user_id)),
    )
    .await
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| ApiError::Internal(format!("Failed to build series DTOs: {:?}", e)))?;

    let response = SeriesListResponse::new(dtos, query.page, page_size, total);

    Ok(Json(response))
}

/// List in-progress series in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/series/in-progress",
    params(
        ("library_id" = Uuid, Path, description = "Library ID")
    ),
    responses(
        (status = 200, description = "List of in-progress series in library", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn list_library_in_progress_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
) -> Result<Json<Vec<SeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Fetch in-progress series for the current user in this library
    let series_list = SeriesRepository::list_in_progress(&state.db, auth.user_id, Some(library_id))
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch in-progress series: {}", e)))?;

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

    Ok(Json(dtos))
}

/// Apply sorting to series list
fn apply_series_sorting(series_list: &mut [crate::db::entities::series::Model], sort_param: &str) {
    let parts: Vec<&str> = sort_param.split(',').collect();
    if parts.len() != 2 {
        return; // Invalid format, skip sorting
    }

    let field = parts[0];
    let direction = parts[1];
    let ascending = direction == "asc";

    match field {
        "name" => {
            series_list.sort_by(|a, b| {
                let cmp = a.name.cmp(&b.name);
                if ascending {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        }
        "created_at" => {
            series_list.sort_by(|a, b| {
                let cmp = a.created_at.cmp(&b.created_at);
                if ascending {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        }
        "book_count" => {
            series_list.sort_by(|a, b| {
                let cmp = a.book_count.cmp(&b.book_count);
                if ascending {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        }
        // Note: "year" sorting requires metadata table join - use repository-level sorting
        // "year" => { ... }
        _ => {} // Unknown field, skip sorting
    }
}

/// Helper function to get the default series cover (first book's first page)
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

/// Generate a thumbnail from an image
fn generate_thumbnail(image_data: &[u8], max_dimension: u32) -> anyhow::Result<Vec<u8>> {
    // Load image from bytes
    let img = image::load_from_memory(image_data)?;

    // Calculate new dimensions while maintaining aspect ratio
    let (width, height) = (img.width(), img.height());
    let (new_width, new_height) = if width > height {
        let ratio = max_dimension as f32 / width as f32;
        (max_dimension, (height as f32 * ratio) as u32)
    } else {
        let ratio = max_dimension as f32 / height as f32;
        ((width as f32 * ratio) as u32, max_dimension)
    };

    // Resize using Lanczos3 filter for high quality
    let thumbnail = img.resize(new_width, new_height, FilterType::Lanczos3);

    // Encode as JPEG with 85% quality
    let mut output = Cursor::new(Vec::new());
    thumbnail.write_to(&mut output, ImageFormat::Jpeg)?;

    Ok(output.into_inner())
}

/// Extract page image from book file
async fn extract_page_image(
    file_path: &str,
    file_format: &str,
    page_number: i32,
) -> anyhow::Result<Vec<u8>> {
    let path = std::path::Path::new(file_path);

    // Call the appropriate parser extraction function
    match file_format.to_uppercase().as_str() {
        "CBZ" => crate::parsers::cbz::extract_page_from_cbz(path, page_number),
        #[cfg(feature = "rar")]
        "CBR" => crate::parsers::cbr::extract_page_from_cbr(path, page_number),
        "EPUB" => crate::parsers::epub::extract_page_from_epub(path, page_number),
        "PDF" => crate::parsers::pdf::extract_page_from_pdf(path, page_number),
        _ => anyhow::bail!("Unsupported format: {}", file_format),
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
    tag = "series"
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
    tag = "series"
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
    tag = "series"
)]
pub async fn download_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Fetch series to verify it exists and get the name for the zip filename
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

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
    let safe_name = sanitize_filename(&series.name);
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
    tag = "series"
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

    active.title_sort = Set(request.sort_name);
    active.summary = Set(request.summary);
    active.publisher = Set(request.publisher);
    active.year = Set(request.year);
    active.reading_direction = Set(request.reading_direction);
    active.updated_at = Set(Utc::now());

    let updated_metadata = active
        .update(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update series metadata: {}", e)))?;

    // Update custom_metadata on series table if provided
    if request.custom_metadata.is_some() || request.custom_metadata.is_none() {
        let mut series_active: series::ActiveModel = existing_series.into();
        series_active.custom_metadata = Set(request.custom_metadata.clone());
        series_active.updated_at = Set(Utc::now());
        series_active
            .update(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update series: {}", e)))?;
    }

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::SeriesUpdated {
            series_id,
            library_id: SeriesRepository::get_by_id(&state.db, series_id)
                .await
                .ok()
                .flatten()
                .map(|s| s.library_id)
                .unwrap_or_default(),
            fields: Some(vec![
                "sort_name".to_string(),
                "summary".to_string(),
                "publisher".to_string(),
                "year".to_string(),
                "reading_direction".to_string(),
                "custom_metadata".to_string(),
            ]),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(SeriesMetadataResponse {
        id: series_id,
        sort_name: updated_metadata.title_sort,
        summary: updated_metadata.summary,
        publisher: updated_metadata.publisher,
        year: updated_metadata.year,
        reading_direction: updated_metadata.reading_direction,
        custom_metadata: request.custom_metadata,
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
    tag = "series"
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
    let mut has_metadata_changes = false;
    let mut has_series_changes = false;
    let mut custom_metadata_value: Option<String> = existing_series.custom_metadata.clone();

    if let Some(opt) = request.sort_name.to_active_value() {
        metadata_active.title_sort = Set(opt);
        has_metadata_changes = true;
    }
    if let Some(opt) = request.summary.to_active_value() {
        metadata_active.summary = Set(opt);
        has_metadata_changes = true;
    }
    if let Some(opt) = request.publisher.to_active_value() {
        metadata_active.publisher = Set(opt);
        has_metadata_changes = true;
    }
    if let Some(opt) = request.year.to_active_value() {
        metadata_active.year = Set(opt);
        has_metadata_changes = true;
    }
    if let Some(opt) = request.reading_direction.to_active_value() {
        metadata_active.reading_direction = Set(opt);
        has_metadata_changes = true;
    }
    if let Some(opt) = request.custom_metadata.to_active_value() {
        custom_metadata_value = opt.clone();
        has_series_changes = true;
    }

    // Update metadata table if needed
    let updated_metadata = if has_metadata_changes {
        metadata_active.updated_at = Set(Utc::now());
        metadata_active
            .update(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update series metadata: {}", e)))?
    } else {
        existing_metadata
    };

    // Update series table for custom_metadata if needed
    if has_series_changes {
        let mut series_active: series::ActiveModel = existing_series.into();
        series_active.custom_metadata = Set(custom_metadata_value.clone());
        series_active.updated_at = Set(Utc::now());
        series_active
            .update(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update series: {}", e)))?;
    }

    // Emit update event
    if has_metadata_changes || has_series_changes {
        let event = EntityChangeEvent {
            event: EntityEvent::SeriesUpdated {
                series_id,
                library_id: SeriesRepository::get_by_id(&state.db, series_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|s| s.library_id)
                    .unwrap_or_default(),
                fields: None, // PATCH updates only changed fields
            },
            timestamp: Utc::now(),
            user_id: Some(auth.user_id),
        };
        let _ = state.event_broadcaster.emit(event);
    }

    Ok(Json(SeriesMetadataResponse {
        id: series_id,
        sort_name: updated_metadata.title_sort,
        summary: updated_metadata.summary,
        publisher: updated_metadata.publisher,
        year: updated_metadata.year,
        reading_direction: updated_metadata.reading_direction,
        custom_metadata: custom_metadata_value,
        updated_at: updated_metadata.updated_at,
    }))
}

/// Get full series metadata including all related data
///
/// Returns comprehensive metadata with lock states, genres, tags, alternate titles,
/// external ratings, and external links.
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/metadata/full",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "Full series metadata with all related data", body = FullSeriesMetadataResponse),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn get_full_series_metadata(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<FullSeriesMetadataResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists and get custom_metadata
    let series = SeriesRepository::get_by_id(&state.db, series_id)
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
        custom_metadata: series.custom_metadata,
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
            genres: metadata.genres_lock,
            tags: metadata.tags_lock,
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
    tag = "series"
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
    if let Some(v) = request.genres {
        active.genres_lock = Set(v);
        has_changes = true;
    }
    if let Some(v) = request.tags {
        active.tags_lock = Set(v);
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
        genres: updated.genres_lock,
        tags: updated.tags_lock,
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
    tag = "series"
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
        genres: metadata.genres_lock,
        tags: metadata.tags_lock,
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

/// List all genres
#[utoipa::path(
    get,
    path = "/api/v1/genres",
    responses(
        (status = 200, description = "List of all genres", body = GenreListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "genres"
)]
pub async fn list_genres(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<GenreListResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let genres = GenreRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch genres: {}", e)))?;

    let mut dtos: Vec<GenreDto> = Vec::with_capacity(genres.len());
    for g in genres {
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

    Ok(Json(GenreListResponse { genres: dtos }))
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
    tag = "genres"
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
    tag = "genres"
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

/// List all tags
#[utoipa::path(
    get,
    path = "/api/v1/tags",
    responses(
        (status = 200, description = "List of all tags", body = TagListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "tags"
)]
pub async fn list_tags(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<TagListResponse>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let tags = TagRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch tags: {}", e)))?;

    let mut dtos: Vec<TagDto> = Vec::with_capacity(tags.len());
    for t in tags {
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

    Ok(Json(TagListResponse { tags: dtos }))
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
    tag = "tags"
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
    tag = "tags"
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
    tag = "genres"
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
    tag = "genres"
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
    tag = "genres"
)]
pub async fn delete_genre(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(genre_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    if !auth.is_admin {
        return Err(ApiError::Forbidden(
            "Admin access required to delete genres".to_string(),
        ));
    }

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
    tag = "genres"
)]
pub async fn cleanup_genres(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<TaxonomyCleanupResponse>, ApiError> {
    if !auth.is_admin {
        return Err(ApiError::Forbidden(
            "Admin access required to cleanup genres".to_string(),
        ));
    }

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
    tag = "tags"
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
    tag = "tags"
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
    tag = "tags"
)]
pub async fn delete_tag(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(tag_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    if !auth.is_admin {
        return Err(ApiError::Forbidden(
            "Admin access required to delete tags".to_string(),
        ));
    }

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
    tag = "tags"
)]
pub async fn cleanup_tags(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
) -> Result<Json<TaxonomyCleanupResponse>, ApiError> {
    if !auth.is_admin {
        return Err(ApiError::Forbidden(
            "Admin access required to cleanup tags".to_string(),
        ));
    }

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
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/rating",
    params(
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "User's rating for the series", body = UserSeriesRatingDto),
        (status = 404, description = "Series or rating not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "ratings"
)]
pub async fn get_series_rating(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<UserSeriesRatingDto>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let rating =
        UserSeriesRatingRepository::get_by_user_and_series(&state.db, auth.user_id, series_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch rating: {}", e)))?
            .ok_or_else(|| ApiError::NotFound("No rating found for this series".to_string()))?;

    Ok(Json(UserSeriesRatingDto {
        id: rating.id,
        series_id: rating.series_id,
        rating: rating.rating,
        notes: rating.notes,
        created_at: rating.created_at,
        updated_at: rating.updated_at,
    }))
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
    tag = "ratings"
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
    tag = "ratings"
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
    tag = "ratings"
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
    tag = "series"
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
    tag = "series"
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
    tag = "series"
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
    tag = "series"
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
    tag = "series"
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
    tag = "series"
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
    tag = "series"
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
    tag = "series"
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
    tag = "series"
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
    tag = "series"
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
    tag = "series"
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
    tag = "series"
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
    tag = "series"
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
