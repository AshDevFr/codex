use crate::api::{
    error::ApiError,
    extractors::{AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::repositories::{BookRepository, PageRepository};
use crate::require_permission;
use crate::utils::{DeadlineResult, with_deadline};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use httpdate::fmt_http_date;
use image::{ImageFormat, imageops::FilterType};
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use uuid::Uuid;

/// Placeholder SVG for thumbnails that are being generated or don't exist
/// This is a simple gray rectangle with a book icon, loaded from assets at compile time
const PLACEHOLDER_SVG: &[u8] = include_bytes!("../../../../../assets/placeholder-cover.svg");

/// Get page image from a book
///
/// Extracts and serves the image for a specific page from CBZ/CBR/EPUB/PDF.
/// For PDF pages, supports HTTP conditional caching with ETag and Last-Modified
/// headers, returning 304 Not Modified when the client has a valid cached copy.
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/pages/{page_number}",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
        ("page_number" = i32, Path, description = "Page number (1-indexed)")
    ),
    responses(
        (status = 200, description = "Page image", content_type = "image/jpeg"),
        (status = 304, description = "Not modified (client cache is valid)"),
        (status = 404, description = "Book or page not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Pages"
)]
pub async fn get_page_image(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    headers: HeaderMap,
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

    // If book has been analyzed, validate page number against known page count
    if book.analyzed && page_number > book.page_count {
        return Err(ApiError::NotFound(format!(
            "Page {} not found (book has {} pages)",
            page_number, book.page_count
        )));
    }

    // For PDFs, we can serve pages directly without requiring page metadata in the database.
    // This handles cases where PDFs were scanned before page metadata population was implemented,
    // or where page analysis failed. PDF pages are rendered on-demand by PDFium.
    if book.format.eq_ignore_ascii_case("pdf") {
        // Update reading progress
        state
            .read_progress_service
            .record_progress(auth.user_id, book_id, page_number, book.page_count)
            .await;

        // PDFs render to JPEG
        return serve_pdf_page_with_streaming(
            &state,
            &headers,
            book_id,
            page_number,
            &book.file_path,
            "image/jpeg",
        )
        .await;
    }

    // Update reading progress via batching service (PSE-style tracking)
    // Progress updates are buffered in memory and flushed periodically
    // to reduce database load during high-traffic page viewing
    state
        .read_progress_service
        .record_progress(auth.user_id, book_id, page_number, book.page_count)
        .await;

    // Try to fetch page metadata for content type detection
    let page = PageRepository::get_by_book_and_number(&state.db, book_id, page_number)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch page: {}", e)))?;

    // For non-PDF formats, extract and serve directly
    // This works even if the book hasn't been analyzed - the parser will extract the page
    let image_data = match extract_page_image(&book.file_path, &book.format, page_number).await {
        Ok(data) => data,
        Err(e) => {
            // If extraction fails and book isn't analyzed, provide a helpful message
            if !book.analyzed {
                return Err(ApiError::NotFound(format!(
                    "Page {} not found. Book has not been analyzed yet - pages may not be available until analysis completes.",
                    page_number
                )));
            }
            return Err(ApiError::NotFound(format!(
                "Page {} not found: {}",
                page_number, e
            )));
        }
    };

    // Determine content type: use page metadata if available, otherwise detect from image data
    let content_type = if let Some(ref page) = page {
        match page.format.to_lowercase().as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "webp" => "image/webp",
            "gif" => "image/gif",
            "bmp" => "image/bmp",
            "avif" => "image/avif",
            _ => detect_content_type(&image_data),
        }
    } else {
        // No page metadata - detect content type from image data
        detect_content_type(&image_data)
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

/// Serve a PDF page with streaming and HTTP conditional caching
///
/// This function:
/// 1. Checks if the page is in cache and gets metadata
/// 2. If cached, checks If-None-Match/If-Modified-Since headers for 304 responses
/// 3. Streams the cached file directly without loading into memory
/// 4. Falls back to rendering if not cached
async fn serve_pdf_page_with_streaming(
    state: &Arc<AuthState>,
    headers: &HeaderMap,
    book_id: Uuid,
    page_number: i32,
    file_path: &str,
    content_type: &str,
) -> Result<Response, ApiError> {
    let dpi = state.pdf_config.render_dpi;
    let cache = &state.pdf_page_cache;

    // Check cache for metadata (fast - just stat the file)
    if let Some(meta) = cache.get_metadata(book_id, page_number, dpi).await {
        // Check If-None-Match header for ETag validation
        if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH)
            && let Ok(client_etag) = if_none_match.to_str()
        {
            // Compare ETags (handle weak ETags by stripping W/ prefix)
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

        // Check If-Modified-Since header
        if let Some(if_modified_since) = headers.get(header::IF_MODIFIED_SINCE)
            && let Ok(date_str) = if_modified_since.to_str()
            && let Ok(client_time) = httpdate::parse_http_date(date_str)
        {
            let file_time = UNIX_EPOCH + Duration::from_secs(meta.modified_unix);
            // If file hasn't been modified since client's copy, return 304
            if file_time <= client_time {
                return Ok(Response::builder()
                    .status(StatusCode::NOT_MODIFIED)
                    .header(header::ETAG, &meta.etag)
                    .header(header::CACHE_CONTROL, "public, max-age=31536000")
                    .body(Body::empty())
                    .unwrap());
            }
        }

        // Cache hit - stream the file directly
        if let Some(stream) = cache.get_stream(book_id, page_number, dpi).await {
            let last_modified = UNIX_EPOCH + Duration::from_secs(meta.modified_unix);
            let last_modified_str = fmt_http_date(last_modified);

            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CONTENT_LENGTH, meta.size)
                .header(header::ETAG, &meta.etag)
                .header(header::LAST_MODIFIED, last_modified_str)
                .header(header::CACHE_CONTROL, "public, max-age=31536000")
                .body(Body::from_stream(stream))
                .unwrap());
        }
    }

    // Cache miss - render the page using spawn_blocking to avoid blocking async runtime
    let path = std::path::PathBuf::from(file_path);
    let image_data = tokio::task::spawn_blocking(move || {
        crate::parsers::pdf::extract_page_from_pdf_with_dpi(&path, page_number, dpi)
    })
    .await
    .map_err(|e| ApiError::Internal(format!("Task join error: {}", e)))?
    .map_err(|e| ApiError::from_anyhow_with_context(e, "Failed to extract page image"))?;

    // Store in cache asynchronously
    let cache_clone = cache.clone();
    let image_data_clone = image_data.clone();
    tokio::spawn(async move {
        if let Err(e) = cache_clone
            .set(book_id, page_number, dpi, &image_data_clone)
            .await
        {
            tracing::warn!(
                "Failed to cache PDF page {} for book {}: {}",
                page_number,
                book_id,
                e
            );
        }
    });

    // Return rendered image
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=31536000")
        .header(header::CONTENT_LENGTH, image_data.len())
        .body(Body::from(image_data))
        .unwrap())
}

/// Get thumbnail/cover image for a book
///
/// Extracts the first page and resizes it to a thumbnail (max 400px width/height).
/// Supports HTTP conditional caching with ETag and Last-Modified headers,
/// returning 304 Not Modified when the client has a valid cached copy.
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/thumbnail",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
    ),
    responses(
        (status = 200, description = "Thumbnail image", content_type = "image/jpeg"),
        (status = 304, description = "Not modified (client cache is valid)"),
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
    headers: HeaderMap,
    Path(book_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // OPTIMIZATION: Check disk cache FIRST before hitting the database.
    // This avoids acquiring a DB connection for cached thumbnails, preventing
    // connection pool exhaustion when many thumbnail requests come in at once.
    if let Some(meta) = state
        .thumbnail_service
        .get_thumbnail_metadata(book_id)
        .await
    {
        // Check If-None-Match header for ETag validation
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
                    .header(header::CACHE_CONTROL, "public, max-age=31536000")
                    .body(Body::empty())
                    .unwrap());
            }
        }

        // Check If-Modified-Since header
        if let Some(if_modified_since) = headers.get(header::IF_MODIFIED_SINCE)
            && let Ok(date_str) = if_modified_since.to_str()
            && let Ok(client_time) = httpdate::parse_http_date(date_str)
        {
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

        // Cache hit - stream the thumbnail directly (no DB query needed!)
        if let Some(stream) = state.thumbnail_service.get_thumbnail_stream(book_id).await {
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

    // Cache miss - now we need to hit the database to generate the thumbnail
    // Fetch book from database
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Return placeholder for books with no pages (don't spam 404s)
    if book.page_count == 0 {
        return Ok(serve_placeholder_response());
    }

    // Use in-flight deduplication to prevent thundering herd
    // If another request is already generating this thumbnail, wait for it
    let thumbnail_data = match state.inflight_thumbnails.try_start(book_id) {
        Ok(guard) => {
            // We're the first request - generate the thumbnail
            match generate_book_thumbnail(&state, &book).await {
                Ok(data) => {
                    guard.complete(data.clone());
                    data
                }
                Err(e) => {
                    guard.fail(format!("{:?}", e));
                    // Return placeholder on generation failure instead of error
                    tracing::warn!("Failed to generate thumbnail for book {}: {:?}", book_id, e);
                    return Ok(serve_placeholder_response());
                }
            }
        }
        Err(handle) => {
            // Another request is generating - wait for it
            match handle.wait().await {
                Ok(data) => data,
                Err(e) => {
                    tracing::warn!(
                        "Waiting for thumbnail generation failed for book {}: {}",
                        book_id,
                        e
                    );
                    return Ok(serve_placeholder_response());
                }
            }
        }
    };

    // Build response with caching headers
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(header::CACHE_CONTROL, "public, max-age=31536000") // Cache for 1 year
        .header(header::CONTENT_LENGTH, thumbnail_data.len())
        .body(Body::from(thumbnail_data))
        .unwrap())
}

/// Generate a thumbnail for a book (handles extraction, resizing, and caching)
async fn generate_book_thumbnail(
    state: &Arc<AuthState>,
    book: &crate::db::entities::books::Model,
) -> Result<Vec<u8>, ApiError> {
    let book_id = book.id;

    // Extract first page (for PDFs, try cache first then render)
    let image_data = if book.format.eq_ignore_ascii_case("pdf") {
        let dpi = state.pdf_config.render_dpi;
        // Try cache first
        if let Some(cached) = state.pdf_page_cache.get(book_id, 1, dpi).await {
            cached
        } else {
            // Render in blocking task to avoid blocking async runtime
            let path = std::path::PathBuf::from(&book.file_path);
            let data = tokio::task::spawn_blocking(move || {
                crate::parsers::pdf::extract_page_from_pdf_with_dpi(&path, 1, dpi)
            })
            .await
            .map_err(|e| ApiError::Internal(format!("Task join error: {}", e)))?
            .map_err(|e| ApiError::from_anyhow_with_context(e, "Failed to extract cover image"))?;

            // Cache asynchronously
            let cache = state.pdf_page_cache.clone();
            let data_clone = data.clone();
            tokio::spawn(async move {
                if let Err(e) = cache.set(book_id, 1, dpi, &data_clone).await {
                    tracing::warn!("Failed to cache PDF page 1 for book {}: {}", book_id, e);
                }
            });

            data
        }
    } else {
        extract_page_image(&book.file_path, &book.format, 1)
            .await
            .map_err(|e| ApiError::from_anyhow_with_context(e, "Failed to extract cover image"))?
    };

    // Generate thumbnail (max 400px width or height) using spawn_blocking
    let thumbnail_data = generate_thumbnail(image_data, 400)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to generate thumbnail: {}", e)))?;

    // Save to cache for future requests (with configurable deadline to prevent blocking)
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

    Ok(thumbnail_data)
}

/// Serve a placeholder SVG image for missing/generating thumbnails
fn serve_placeholder_response() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/svg+xml")
        .header(header::CACHE_CONTROL, "public, max-age=60") // Short cache for placeholders
        .header(header::CONTENT_LENGTH, PLACEHOLDER_SVG.len())
        .body(Body::from(PLACEHOLDER_SVG.to_vec()))
        .unwrap()
}

/// Generate a thumbnail from an image (synchronous version for use in spawn_blocking)
///
/// Resizes the image to fit within max_dimension x max_dimension while maintaining aspect ratio
fn generate_thumbnail_sync(image_data: &[u8], max_dimension: u32) -> anyhow::Result<Vec<u8>> {
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

/// Generate a thumbnail from an image (async version using spawn_blocking)
///
/// Uses spawn_blocking to avoid blocking the async runtime during CPU-intensive
/// image decoding, resizing (Lanczos3), and JPEG encoding operations
async fn generate_thumbnail(image_data: Vec<u8>, max_dimension: u32) -> anyhow::Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || generate_thumbnail_sync(&image_data, max_dimension))
        .await
        .map_err(|e| anyhow::anyhow!("Task join error: {}", e))?
}

/// Detect content type from image data using magic bytes
///
/// Falls back to "image/jpeg" if the format cannot be determined
fn detect_content_type(data: &[u8]) -> &'static str {
    if data.len() < 4 {
        return "application/octet-stream";
    }

    // Check magic bytes for common image formats
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        // PNG: 89 50 4E 47 (‰PNG)
        "image/png"
    } else if data.starts_with(b"RIFF") && data.len() > 12 && &data[8..12] == b"WEBP" {
        "image/webp"
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        "image/gif"
    } else if data.starts_with(&[0x42, 0x4D]) {
        // BMP: 42 4D (BM)
        "image/bmp"
    } else if data.len() >= 12 && &data[4..12] == b"ftypavif" {
        "image/avif"
    } else {
        // Default to JPEG as it's the most common format in comics
        "image/jpeg"
    }
}

/// Extract page image from book file
///
/// Uses spawn_blocking to avoid blocking the async runtime during CPU-intensive
/// image extraction operations (ZIP parsing, RAR extraction, EPUB parsing, PDF rendering)
async fn extract_page_image(
    file_path: &str,
    file_format: &str,
    page_number: i32,
) -> anyhow::Result<Vec<u8>> {
    let path = std::path::PathBuf::from(file_path);
    let format = file_format.to_uppercase();

    // Use spawn_blocking for CPU-intensive file parsing operations
    tokio::task::spawn_blocking(move || match format.as_str() {
        "CBZ" => crate::parsers::cbz::extract_page_from_cbz(&path, page_number),
        #[cfg(feature = "rar")]
        "CBR" => crate::parsers::cbr::extract_page_from_cbr(&path, page_number),
        "EPUB" => crate::parsers::epub::extract_page_from_epub(&path, page_number),
        "PDF" => crate::parsers::pdf::extract_page_from_pdf(&path, page_number),
        _ => anyhow::bail!("Unsupported format: {}", format),
    })
    .await
    .map_err(|e| anyhow::anyhow!("Task join error: {}", e))?
}
