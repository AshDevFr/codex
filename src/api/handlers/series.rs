use crate::api::{
    dto::{BookDto, SearchSeriesRequest, SeriesDto, SeriesFilter},
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::repositories::{BookRepository, SeriesRepository};
use crate::require_permission;
use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use image::{imageops::FilterType, ImageFormat};
use serde::Deserialize;
use std::io::Cursor;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

/// Query parameters for listing books in a series
#[derive(Debug, Deserialize)]
pub struct ListBooksQuery {
    /// Include deleted books in the result
    #[serde(default)]
    pub include_deleted: bool,
}

/// List series with optional library filter
#[utoipa::path(
    get,
    path = "/api/v1/series",
    params(
        ("library_id" = Option<Uuid>, Query, description = "Filter by library ID")
    ),
    responses(
        (status = 200, description = "List of series", body = Vec<SeriesDto>),
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
    Query(filter): Query<SeriesFilter>,
) -> Result<Json<Vec<SeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Fetch series based on filter
    let series_list = if let Some(lib_id) = filter.library_id {
        SeriesRepository::list_by_library(&state.db, lib_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
    } else {
        SeriesRepository::list_all(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
    };

    let dtos: Vec<SeriesDto> = series_list
        .into_iter()
        .map(|series| SeriesDto {
            id: series.id,
            library_id: series.library_id,
            name: series.name,
            sort_name: series.sort_name,
            description: series.summary, // Use summary instead of description
            publisher: series.publisher,
            year: series.year,
            book_count: series.book_count as i64, // Convert i32 to i64
            path: series.path,
            selected_cover_source: series.selected_cover_source.clone(),
            has_custom_cover: Some(series.custom_cover_path.is_some()),
            created_at: series.created_at,
            updated_at: series.updated_at,
        })
        .collect();

    Ok(Json(dtos))
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

    let dto = SeriesDto {
        id: series.id,
        library_id: series.library_id,
        name: series.name,
        sort_name: series.sort_name,
        description: series.summary, // Use summary instead of description
        publisher: series.publisher,
        year: series.year,
        book_count: series.book_count as i64, // Convert i32 to i64
        path: series.path,
        selected_cover_source: series.selected_cover_source.clone(),
        has_custom_cover: Some(series.custom_cover_path.is_some()),
        created_at: series.created_at,
        updated_at: series.updated_at,
    };

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

    let dtos: Vec<SeriesDto> = filtered
        .into_iter()
        .map(|series| SeriesDto {
            id: series.id,
            library_id: series.library_id,
            name: series.name,
            sort_name: series.sort_name,
            description: series.summary, // Use summary instead of description
            publisher: series.publisher,
            year: series.year,
            book_count: series.book_count as i64, // Convert i32 to i64
            path: series.path,
            selected_cover_source: series.selected_cover_source.clone(),
            has_custom_cover: Some(series.custom_cover_path.is_some()),
            created_at: series.created_at,
            updated_at: series.updated_at,
        })
        .collect();

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

    // Convert to DTOs
    let dtos: Vec<BookDto> = books
        .into_iter()
        .map(|book| {
            let title = book.title.clone().unwrap_or_else(|| book.file_name.clone());
            BookDto {
                id: book.id,
                series_id: book.series_id,
                title: title.clone(),
                sort_title: book.title,
                file_path: book.file_path,
                file_format: book.format,
                file_size: book.file_size,
                file_hash: book.file_hash,
                page_count: book.page_count,
                number: book
                    .number
                    .map(|n| n.to_string().parse::<i32>().unwrap_or(0)),
                created_at: book.created_at,
                updated_at: book.updated_at,
            }
        })
        .collect();

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

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Purge deleted books
    let count = BookRepository::purge_deleted_in_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to purge deleted books: {}", e)))?;

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

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
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

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Update the selected cover source
    SeriesRepository::update_selected_cover_source(&state.db, series_id, Some(request.source))
        .await
        .map_err(|e| {
            ApiError::Internal(format!("Failed to update selected cover source: {}", e))
        })?;

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
    auth: AuthContext,
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
