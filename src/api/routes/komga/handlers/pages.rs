//! Komga-compatible page handlers
//!
//! Handlers for page-related endpoints in the Komga-compatible API.
//! These endpoints provide page listing, streaming, and thumbnail generation.

use super::super::dto::page::KomgaPageDto;
use super::libraries::{extract_page_image, generate_thumbnail};
use crate::api::{
    error::ApiError,
    extractors::{AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::repositories::{BookRepository, PageRepository};
use crate::require_permission;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

/// List all pages for a book
///
/// Returns an array of page metadata for all pages in a book.
/// Pages are ordered by page number (1-indexed).
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/books/{bookId}/pages`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
///
/// ## Response
/// Returns an array of `KomgaPageDto` objects with page metadata including
/// filename, MIME type, dimensions, and size.
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/books/{book_id}/pages",
    responses(
        (status = 200, description = "List of pages in the book", body = Vec<KomgaPageDto>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Book not found"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komgav1)"),
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "komga"
)]
pub async fn list_pages(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<Vec<KomgaPageDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Verify book exists and get book info
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Get all pages from database
    let pages = PageRepository::list_by_book(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch pages: {}", e)))?;

    // Convert to Komga DTOs
    let page_dtos: Vec<KomgaPageDto> = if pages.is_empty() {
        // If no pages in database, generate synthetic page info from book's page_count
        // This handles books that haven't been fully analyzed yet
        (1..=book.page_count)
            .map(|page_number| {
                KomgaPageDto::from_codex(
                    &format!("page{:04}.jpg", page_number), // Synthetic filename
                    page_number,
                    None, // width unknown
                    None, // height unknown
                    None, // size unknown
                    None, // media type will be guessed
                )
            })
            .collect()
    } else {
        pages
            .into_iter()
            .map(|page| {
                KomgaPageDto::from_codex(
                    &page.file_name,
                    page.page_number,
                    Some(page.width),
                    Some(page.height),
                    Some(page.file_size),
                    Some(&format!("image/{}", page.format.to_lowercase())),
                )
            })
            .collect()
    };

    Ok(Json(page_dtos))
}

/// Get a specific page image
///
/// Streams the raw page image for the requested page number.
/// Page numbers are 1-indexed.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/books/{bookId}/pages/{pageNumber}`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key (via cookie fallback for browser image tags)
///
/// ## Response
/// Returns the raw image data with appropriate Content-Type header.
/// Response is cached for 1 year (immutable content).
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/books/{book_id}/pages/{page_number}",
    responses(
        (status = 200, description = "Page image", content_type = "image/*"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Book or page not found"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komgav1)"),
        ("book_id" = Uuid, Path, description = "Book ID"),
        ("page_number" = i32, Path, description = "Page number (1-indexed)")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "komga"
)]
pub async fn get_page(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path((book_id, page_number)): Path<(Uuid, i32)>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate page number
    if page_number < 1 {
        return Err(ApiError::BadRequest(
            "Page number must be at least 1".to_string(),
        ));
    }

    // Get the book
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Validate page number is within bounds
    if page_number > book.page_count {
        return Err(ApiError::NotFound(format!(
            "Page {} not found (book has {} pages)",
            page_number, book.page_count
        )));
    }

    // Try to get page metadata from database for content type
    let page_metadata = PageRepository::get_by_book_and_number(&state.db, book_id, page_number)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch page metadata: {}", e)))?;

    // Determine content type from metadata or default to JPEG
    let content_type = page_metadata
        .as_ref()
        .map(|p| format!("image/{}", p.format.to_lowercase()))
        .unwrap_or_else(|| "image/jpeg".to_string());

    // Extract the page image from the book file
    let image_data = extract_page_image(&book.file_path, &book.format, page_number)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to extract page: {}", e)))?;

    // Build response with caching headers (immutable content)
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .header(header::CONTENT_LENGTH, image_data.len())
        .body(Body::from(image_data))
        .unwrap())
}

/// Get a page thumbnail
///
/// Returns a thumbnail version of the requested page.
/// Thumbnails are resized to max 300px width/height while maintaining aspect ratio.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/books/{bookId}/pages/{pageNumber}/thumbnail`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key (via cookie fallback for browser image tags)
///
/// ## Response
/// Returns a JPEG thumbnail with appropriate caching headers.
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/books/{book_id}/pages/{page_number}/thumbnail",
    responses(
        (status = 200, description = "Page thumbnail image", content_type = "image/jpeg"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Book or page not found"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komgav1)"),
        ("book_id" = Uuid, Path, description = "Book ID"),
        ("page_number" = i32, Path, description = "Page number (1-indexed)")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "komga"
)]
pub async fn get_page_thumbnail(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path((book_id, page_number)): Path<(Uuid, i32)>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate page number
    if page_number < 1 {
        return Err(ApiError::BadRequest(
            "Page number must be at least 1".to_string(),
        ));
    }

    // Get the book
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Validate page number is within bounds
    if page_number > book.page_count {
        return Err(ApiError::NotFound(format!(
            "Page {} not found (book has {} pages)",
            page_number, book.page_count
        )));
    }

    // Extract the page image from the book file
    let image_data = extract_page_image(&book.file_path, &book.format, page_number)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to extract page: {}", e)))?;

    // Generate thumbnail (max 300px for page thumbnails)
    let thumbnail_data = generate_thumbnail(&image_data, 300)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_page_dto_generation() {
        // Test that synthetic page DTOs have reasonable defaults
        let dto = KomgaPageDto::from_codex(
            "page0001.jpg",
            1,
            None, // no width
            None, // no height
            None, // no size
            None, // media type guessed from extension
        );

        assert_eq!(dto.number, 1);
        assert_eq!(dto.file_name, "page0001.jpg");
        assert_eq!(dto.media_type, "image/jpeg");
        assert_eq!(dto.width, 0);
        assert_eq!(dto.height, 0);
        assert_eq!(dto.size_bytes, 0);
    }

    #[test]
    fn test_page_dto_with_full_metadata() {
        let dto = KomgaPageDto::from_codex(
            "chapter1/img001.png",
            5,
            Some(1920),
            Some(2560),
            Some(1048576),
            Some("image/png"),
        );

        assert_eq!(dto.number, 5);
        assert_eq!(dto.file_name, "chapter1/img001.png");
        assert_eq!(dto.media_type, "image/png");
        assert_eq!(dto.width, 1920);
        assert_eq!(dto.height, 2560);
        assert_eq!(dto.size_bytes, 1048576);
        assert_eq!(dto.size, "1.0 MiB");
    }

    #[test]
    fn test_page_dto_media_type_from_format() {
        // Test that format string is converted to proper MIME type
        let dto = KomgaPageDto::from_codex(
            "image.webp",
            1,
            Some(100),
            Some(100),
            Some(1000),
            Some("image/webp"),
        );

        assert_eq!(dto.media_type, "image/webp");
    }
}
