use crate::api::{
    dto::{
        series::SeriesSortParam, BookDto, MarkReadResponse, SearchSeriesRequest, SeriesDto,
        SeriesListResponse,
    },
    error::ApiError,
    extractors::{AuthContext, AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::entities::series;
use crate::db::repositories::{BookRepository, ReadProgressRepository, SeriesRepository};
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
}

fn default_page_size() -> u64 {
    20
}

/// Helper function to convert series model to DTO with unread count
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

    Ok(SeriesDto {
        id: series.id,
        library_id: series.library_id,
        name: series.name,
        sort_name: series.sort_name,
        description: series.summary,
        publisher: series.publisher,
        year: series.year,
        book_count: series.book_count as i64,
        path: series.path,
        selected_cover_source: series.selected_cover_source.clone(),
        has_custom_cover: Some(series.custom_cover_path.is_some()),
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
        ("sort" = Option<String>, Query, description = "Sort parameter (format: 'field,direction')")
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

    // Fetch series based on filter (all libraries)
    let mut series_list = SeriesRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

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
    path = "/api/v1/series/{id}",
    params(
        ("id" = Uuid, Path, description = "Series ID")
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
    Path(id): Path<Uuid>,
) -> Result<Json<SeriesDto>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series = SeriesRepository::get_by_id(&state.db, id)
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

/// Get books in a series
#[utoipa::path(
    get,
    path = "/api/v1/series/{id}/books",
    params(
        ("id" = Uuid, Path, description = "Series ID"),
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
    path = "/api/v1/series/{id}/purge-deleted",
    params(
        ("id" = Uuid, Path, description = "Series ID")
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
    path = "/api/v1/series/{id}/cover",
    params(
        ("id" = Uuid, Path, description = "Series ID")
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

    // Create covers directory if it doesn't exist
    let covers_dir = std::path::Path::new("data/covers");
    fs::create_dir_all(covers_dir)
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

    // Update series with custom cover path
    SeriesRepository::update_custom_cover(
        &state.db,
        series_id,
        Some(filepath.to_string_lossy().to_string()),
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to update series cover: {}", e)))?;

    // Set the selected cover source to "custom"
    SeriesRepository::update_selected_cover_source(
        &state.db,
        series_id,
        Some("custom".to_string()),
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to update selected cover source: {}", e)))?;

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

/// Set which cover source to use for a series
#[utoipa::path(
    put,
    path = "/api/v1/series/{id}/cover/source",
    params(
        ("id" = Uuid, Path, description = "Series ID")
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

    // Update the selected cover source
    SeriesRepository::update_selected_cover_source(&state.db, series_id, Some(request.source))
        .await
        .map_err(|e| {
            ApiError::Internal(format!("Failed to update selected cover source: {}", e))
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

    Ok(StatusCode::OK)
}

/// Get thumbnail/cover image for a series
#[utoipa::path(
    get,
    path = "/api/v1/series/{id}/thumbnail",
    params(
        ("id" = Uuid, Path, description = "Series ID"),
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

    // Fetch series
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Determine which cover to use based on selected_cover_source
    let image_data = match series.selected_cover_source.as_deref() {
        Some("custom") => {
            // Use custom uploaded cover
            if let Some(cover_path) = series.custom_cover_path {
                fs::read(&cover_path).await.map_err(|e| {
                    ApiError::Internal(format!("Failed to read custom cover: {}", e))
                })?
            } else {
                // Fall back to default if custom cover path is missing
                get_default_series_cover(&state, series_id).await?
            }
        }
        _ => {
            // Use default (first book's cover)
            get_default_series_cover(&state, series_id).await?
        }
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

/// List series with in-progress books (series that have at least one book with reading progress that is not completed)
#[utoipa::path(
    get,
    path = "/api/v1/series/in-progress",
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
) -> Result<Json<Vec<SeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Fetch in-progress series for the current user
    let series_list = SeriesRepository::list_in_progress(&state.db, auth.user_id, None)
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
}

fn default_recent_limit() -> u64 {
    50
}

/// List recently added series
#[utoipa::path(
    get,
    path = "/api/v1/series/recently-added",
    params(
        ("limit" = Option<u64>, Query, description = "Maximum number of series to return (default: 50)")
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

    let series_list = SeriesRepository::list_recently_added(&state.db, None, query.limit)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch recently added series: {}", e)))?;

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
        ("limit" = Option<u64>, Query, description = "Maximum number of series to return (default: 50)")
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

    let series_list = SeriesRepository::list_recently_updated(&state.db, None, query.limit)
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
        "year" => {
            series_list.sort_by(|a, b| {
                let cmp = a.year.cmp(&b.year);
                if ascending {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        }
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
    path = "/api/v1/series/{id}/read",
    params(
        ("id" = Uuid, Path, description = "Series ID")
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
    path = "/api/v1/series/{id}/unread",
    params(
        ("id" = Uuid, Path, description = "Series ID")
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
    path = "/api/v1/series/{id}/download",
    params(
        ("id" = Uuid, Path, description = "Series ID")
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
