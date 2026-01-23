use crate::api::{
    error::ApiError,
    extractors::{AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::repositories::{BookRepository, PageRepository};
use crate::require_permission;
use crate::utils::{with_deadline, DeadlineResult};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
};
use image::{imageops::FilterType, ImageFormat};
use std::io::Cursor;
use std::sync::Arc;
use uuid::Uuid;

/// Get page image from a book
///
/// Extracts and serves the image for a specific page from CBZ/CBR/EPUB/PDF
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/pages/{page_number}",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
        ("page_number" = i32, Path, description = "Page number (1-indexed)")
    ),
    responses(
        (status = 200, description = "Page image", content_type = "image/jpeg"),
        (status = 404, description = "Book or page not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "pages"
)]
pub async fn get_page_image(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path((book_id, page_number)): Path<(Uuid, i32)>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::PagesRead)?;

    // Validate page number
    if page_number < 1 {
        return Err(ApiError::BadRequest("Page number must be >= 1".to_string()));
    }

    // Fetch book from database
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Check page number is valid
    if page_number > book.page_count {
        return Err(ApiError::NotFound(format!(
            "Page {} not found (book has {} pages)",
            page_number, book.page_count
        )));
    }

    // Fetch page metadata
    let page = PageRepository::get_by_book_and_number(&state.db, book_id, page_number)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch page: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Page not found".to_string()))?;

    // Update reading progress via batching service (PSE-style tracking)
    // Progress updates are buffered in memory and flushed periodically
    // to reduce database load during high-traffic page viewing
    state
        .read_progress_service
        .record_progress(auth.user_id, book_id, page_number, book.page_count)
        .await;

    // Extract image from book file based on format
    let image_data = extract_page_image(&book.file_path, &book.format, page_number)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to extract page image: {}", e)))?;

    // Determine content type from file format
    let content_type = match page.format.to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    };

    // Build response with caching headers
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=31536000") // Cache for 1 year
        .header(header::CONTENT_LENGTH, image_data.len())
        .body(Body::from(image_data))
        .unwrap())
}

/// Get thumbnail/cover image for a book
///
/// Extracts the first page and resizes it to a thumbnail (max 400px width/height)
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/thumbnail",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
    ),
    responses(
        (status = 200, description = "Thumbnail image", content_type = "image/jpeg"),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn get_book_thumbnail(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Fetch book from database
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Check if book has pages
    if book.page_count == 0 {
        return Err(ApiError::NotFound("Book has no pages".to_string()));
    }

    // Try to serve cached thumbnail first
    if let Ok(thumbnail_data) = state.thumbnail_service.read_thumbnail(book_id).await {
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/jpeg")
            .header(header::CACHE_CONTROL, "public, max-age=31536000") // Cache for 1 year
            .header(header::CONTENT_LENGTH, thumbnail_data.len())
            .body(Body::from(thumbnail_data))
            .unwrap());
    }

    // Cache miss - generate thumbnail on-demand
    // Extract first page
    let image_data = extract_page_image(&book.file_path, &book.format, 1)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to extract cover image: {}", e)))?;

    // Generate thumbnail (max 400px width or height)
    let thumbnail_data = generate_thumbnail(&image_data, 400)
        .map_err(|e| ApiError::Internal(format!("Failed to generate thumbnail: {}", e)))?;

    // Save to cache for future requests (with configurable deadline to prevent blocking)
    // Using inline await instead of fire-and-forget to manage connection pool usage
    let deadline_secs = state.database_config.operation_deadline_seconds();
    match with_deadline(
        deadline_secs,
        state
            .thumbnail_service
            .save_generated_thumbnail(&state.db, book_id, &thumbnail_data),
    )
    .await
    {
        DeadlineResult::Ok(_path) => {
            // Successfully saved
        }
        DeadlineResult::Err(e) => {
            tracing::warn!("Failed to cache thumbnail for book {}: {}", book_id, e);
        }
        DeadlineResult::TimedOut => {
            tracing::warn!(
                "Timeout saving thumbnail for book {} (>{}s), skipping cache",
                book_id,
                deadline_secs
            );
        }
    }

    // Build response with caching headers
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(header::CACHE_CONTROL, "public, max-age=31536000") // Cache for 1 year
        .header(header::CONTENT_LENGTH, thumbnail_data.len())
        .body(Body::from(thumbnail_data))
        .unwrap())
}

/// Generate a thumbnail from an image
///
/// Resizes the image to fit within max_dimension x max_dimension while maintaining aspect ratio
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
