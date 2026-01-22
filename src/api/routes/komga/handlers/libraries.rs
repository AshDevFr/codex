//! Komga-compatible library handlers
//!
//! Handlers for library-related endpoints in the Komga-compatible API.

use super::super::dto::library::KomgaLibraryDto;
use crate::api::{
    error::ApiError,
    extractors::{AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::repositories::{
    BookRepository, LibraryRepository, SeriesCoversRepository, SeriesRepository,
};
use crate::require_permission;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use image::{imageops::FilterType, ImageFormat};
use std::io::Cursor;
use std::sync::Arc;
use tokio::fs;
use uuid::Uuid;

/// List all libraries
///
/// Returns all libraries in Komga-compatible format.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/libraries`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/libraries",
    responses(
        (status = 200, description = "List of libraries", body = Vec<KomgaLibraryDto>),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komgav1)")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "komga"
)]
pub async fn list_libraries(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
) -> Result<Json<Vec<KomgaLibraryDto>>, ApiError> {
    require_permission!(auth, Permission::LibrariesRead)?;

    let libraries = LibraryRepository::list_all(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch libraries: {}", e)))?;

    let dtos: Vec<KomgaLibraryDto> = libraries
        .into_iter()
        .map(|lib| {
            KomgaLibraryDto::from_codex(
                lib.id,
                &lib.name,
                &lib.path,
                true, // is_active - Codex doesn't have this field, assume active
                lib.excluded_patterns.as_deref(),
            )
        })
        .collect();

    Ok(Json(dtos))
}

/// Get library by ID
///
/// Returns a single library in Komga-compatible format.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/libraries/{libraryId}`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/libraries/{library_id}",
    responses(
        (status = 200, description = "Library details", body = KomgaLibraryDto),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Library not found"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komgav1)"),
        ("library_id" = Uuid, Path, description = "Library ID")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "komga"
)]
pub async fn get_library(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(library_id): Path<Uuid>,
) -> Result<Json<KomgaLibraryDto>, ApiError> {
    require_permission!(auth, Permission::LibrariesRead)?;

    let library = LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    let dto = KomgaLibraryDto::from_codex(
        library.id,
        &library.name,
        &library.path,
        true, // is_active
        library.excluded_patterns.as_deref(),
    );

    Ok(Json(dto))
}

/// Get library thumbnail
///
/// Returns a thumbnail image for the library. Uses the first series' cover
/// as the library thumbnail, or returns a 404 if no series exist.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/libraries/{libraryId}/thumbnail`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key (via cookie fallback for browser image tags)
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/libraries/{library_id}/thumbnail",
    responses(
        (status = 200, description = "Library thumbnail image", content_type = "image/jpeg"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Library not found or no series in library"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komgav1)"),
        ("library_id" = Uuid, Path, description = "Library ID")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "komga"
)]
pub async fn get_library_thumbnail(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(library_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::LibrariesRead)?;

    // Verify library exists
    let _library = LibraryRepository::get_by_id(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch library: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Library not found".to_string()))?;

    // Get the first series in this library to use as the thumbnail
    let series_list = SeriesRepository::list_by_library(&state.db, library_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    let first_series = series_list
        .into_iter()
        .next()
        .ok_or_else(|| ApiError::NotFound("No series in library".to_string()))?;

    // Get the series cover - try selected cover first, then default
    let image_data = if let Some(cover) =
        SeriesCoversRepository::get_selected(&state.db, first_series.id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch cover: {}", e)))?
    {
        fs::read(&cover.path).await.map_err(|e| {
            ApiError::Internal(format!("Failed to read cover from {}: {}", cover.path, e))
        })?
    } else {
        // Fall back to first book's first page
        get_default_series_cover(&state, first_series.id).await?
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

/// Generate a thumbnail from an image
pub fn generate_thumbnail(image_data: &[u8], max_dimension: u32) -> anyhow::Result<Vec<u8>> {
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
pub async fn extract_page_image(
    file_path: &str,
    file_format: &str,
    page_number: i32,
) -> anyhow::Result<Vec<u8>> {
    let path = std::path::Path::new(file_path);

    // Use the appropriate parser based on format
    let image_data = match file_format.to_uppercase().as_str() {
        "CBZ" => crate::parsers::cbz::extract_page_from_cbz(path, page_number)?,
        #[cfg(feature = "rar")]
        "CBR" => crate::parsers::cbr::extract_page_from_cbr(path, page_number)?,
        "EPUB" => crate::parsers::epub::extract_page_from_epub(path, page_number)?,
        "PDF" => crate::parsers::pdf::extract_page_from_pdf(path, page_number)?,
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported format for page extraction: {}",
                file_format
            ));
        }
    };

    Ok(image_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_dto_creation() {
        let id = Uuid::new_v4();
        let dto = KomgaLibraryDto::from_codex(id, "Test Library", "/path/to/library", true, None);

        assert_eq!(dto.id, id.to_string());
        assert_eq!(dto.name, "Test Library");
        assert_eq!(dto.root, "/path/to/library");
        assert!(!dto.unavailable);
    }

    #[test]
    fn test_library_dto_with_exclusions() {
        let id = Uuid::new_v4();
        let dto = KomgaLibraryDto::from_codex(
            id,
            "Test Library",
            "/path/to/library",
            true,
            Some(".DS_Store\nThumbs.db"),
        );

        assert_eq!(
            dto.scan_directory_exclusions,
            vec![".DS_Store", "Thumbs.db"]
        );
    }

    #[test]
    fn test_library_dto_inactive() {
        let id = Uuid::new_v4();
        let dto = KomgaLibraryDto::from_codex(id, "Test Library", "/path/to/library", false, None);

        assert!(dto.unavailable);
    }
}
