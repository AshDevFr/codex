//! Komga-compatible book handlers
//!
//! Handlers for book-related endpoints in the Komga-compatible API.

use super::super::dto::book::{KomgaBookDto, KomgaBooksSearchRequestDto};
use super::super::dto::pagination::KomgaPage;
use super::libraries::{extract_page_image, generate_thumbnail};
use crate::api::{
    error::ApiError,
    extractors::{AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::repositories::{
    BookMetadataRepository, BookRepository, ReadProgressRepository, SeriesMetadataRepository,
};
use crate::require_permission;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

/// Query parameters for paginated book endpoints
#[derive(Debug, Deserialize)]
pub struct BooksPaginationQuery {
    /// Page number (0-indexed, Komga-style)
    #[serde(default)]
    pub page: i32,
    /// Page size (default: 20)
    #[serde(default = "default_page_size")]
    pub size: i32,
    /// Sort parameter (e.g., "createdDate,desc", "metadata.numberSort,asc")
    pub sort: Option<String>,
}

fn default_page_size() -> i32 {
    20
}

/// Get a book by ID
///
/// Returns a single book in Komga-compatible format.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/books/{bookId}`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
pub async fn get_book(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<KomgaBookDto>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let user_id = Some(auth.user_id);

    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Get series title
    let series_title = get_series_title(&state, book.series_id).await?;

    // Get book number from metadata
    let book_number = get_book_number(&state, book_id).await.unwrap_or(1);

    // Get read progress for this book and user
    let read_progress = if let Some(uid) = user_id {
        ReadProgressRepository::get_by_user_and_book(&state.db, uid, book.id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let dto = KomgaBookDto::from_codex(&book, &series_title, book_number, read_progress.as_ref());
    Ok(Json(dto))
}

/// Get book thumbnail
///
/// Returns a thumbnail image for the book's first page.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/books/{bookId}/thumbnail`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
pub async fn get_book_thumbnail(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Fetch book
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Check if book has pages
    if book.page_count == 0 {
        return Err(ApiError::NotFound("Book has no pages".to_string()));
    }

    // Extract first page from the book
    let image_data = extract_page_image(&book.file_path, &book.format, 1)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to extract cover image: {}", e)))?;

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

/// Get "on deck" books
///
/// Returns books that are currently in-progress (started but not completed).
/// This is the "continue reading" shelf in Komic.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/books/ondeck`
///
/// ## Query Parameters
/// - `page` - Page number (0-indexed, default: 0)
/// - `size` - Page size (default: 20)
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
pub async fn get_books_ondeck(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<BooksPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaBookDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let user_id = auth.user_id;
    let page = query.page.max(0) as u64;
    let size = query.size.max(1).min(500) as u64;

    // Get in-progress books (completed = false)
    let (books, total) = BookRepository::list_with_progress(
        &state.db,
        user_id,
        None,        // library_id
        Some(false), // completed = false means in-progress
        page,
        size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch on-deck books: {}", e)))?;

    // Convert to DTOs
    let mut dtos = Vec::with_capacity(books.len());
    for book in books {
        let series_title = get_series_title(&state, book.series_id).await?;
        let book_number = get_book_number(&state, book.id).await.unwrap_or(1);

        let read_progress =
            ReadProgressRepository::get_by_user_and_book(&state.db, user_id, book.id)
                .await
                .ok()
                .flatten();

        let dto =
            KomgaBookDto::from_codex(&book, &series_title, book_number, read_progress.as_ref());
        dtos.push(dto);
    }

    Ok(Json(KomgaPage::new(
        dtos,
        query.page,
        query.size,
        total as i64,
    )))
}

/// Search/filter books
///
/// Returns books matching the filter criteria.
/// This uses POST to support complex filter bodies.
///
/// ## Endpoint
/// `POST /{prefix}/api/v1/books/list`
///
/// ## Query Parameters
/// - `page` - Page number (0-indexed, default: 0)
/// - `size` - Page size (default: 20)
/// - `sort` - Sort parameter (e.g., "createdDate,desc")
///
/// ## Request Body
/// JSON object with filter criteria (library_id, series_id, search_term, etc.)
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
pub async fn search_books(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<BooksPaginationQuery>,
    Json(body): Json<KomgaBooksSearchRequestDto>,
) -> Result<Json<KomgaPage<KomgaBookDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let user_id = Some(auth.user_id);
    let page = query.page.max(0) as u64;
    let size = query.size.max(1).min(500) as u64;

    // Parse filter criteria
    let library_id = body
        .library_id
        .as_ref()
        .and_then(|ids| ids.first())
        .and_then(|id| Uuid::parse_str(id).ok());

    let series_id = body
        .series_id
        .as_ref()
        .and_then(|ids| ids.first())
        .and_then(|id| Uuid::parse_str(id).ok());

    // Fetch books based on filters
    let (books, total) = if let Some(series_id) = series_id {
        // Filter by series
        let all_books = BookRepository::list_by_series(&state.db, series_id, false)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

        let total = all_books.len() as u64;
        let paginated: Vec<_> = all_books
            .into_iter()
            .skip((page * size) as usize)
            .take(size as usize)
            .collect();
        (paginated, total)
    } else if let Some(library_id) = library_id {
        // Filter by library
        BookRepository::list_by_library(&state.db, library_id, false, page, size)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?
    } else {
        // No filter - get all books
        BookRepository::list_all(&state.db, false, page, size)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?
    };

    // Convert to DTOs
    let mut dtos = Vec::with_capacity(books.len());
    for book in books {
        let series_title = get_series_title(&state, book.series_id).await?;
        let book_number = get_book_number(&state, book.id).await.unwrap_or(1);

        let read_progress = if let Some(uid) = user_id {
            ReadProgressRepository::get_by_user_and_book(&state.db, uid, book.id)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        let dto =
            KomgaBookDto::from_codex(&book, &series_title, book_number, read_progress.as_ref());
        dtos.push(dto);
    }

    Ok(Json(KomgaPage::new(
        dtos,
        query.page,
        query.size,
        total as i64,
    )))
}

/// Get next book in series
///
/// Returns the next book in the same series by sort order.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/books/{bookId}/next`
///
/// ## Response
/// - 200: Next book DTO
/// - 404: No next book (this is the last book in series)
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
pub async fn get_next_book(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<KomgaBookDto>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let user_id = Some(auth.user_id);

    // Get adjacent books
    let (_prev, next) = BookRepository::get_adjacent_in_series(&state.db, book_id)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                ApiError::NotFound("Book not found".to_string())
            } else {
                ApiError::Internal(format!("Failed to get next book: {}", e))
            }
        })?;

    let next_book = next.ok_or_else(|| ApiError::NotFound("No next book".to_string()))?;

    // Get series title and metadata
    let series_title = get_series_title(&state, next_book.series_id).await?;
    let book_number = get_book_number(&state, next_book.id).await.unwrap_or(1);

    let read_progress = if let Some(uid) = user_id {
        ReadProgressRepository::get_by_user_and_book(&state.db, uid, next_book.id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let dto = KomgaBookDto::from_codex(
        &next_book,
        &series_title,
        book_number,
        read_progress.as_ref(),
    );
    Ok(Json(dto))
}

/// Get previous book in series
///
/// Returns the previous book in the same series by sort order.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/books/{bookId}/previous`
///
/// ## Response
/// - 200: Previous book DTO
/// - 404: No previous book (this is the first book in series)
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
pub async fn get_previous_book(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<KomgaBookDto>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let user_id = Some(auth.user_id);

    // Get adjacent books
    let (prev, _next) = BookRepository::get_adjacent_in_series(&state.db, book_id)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                ApiError::NotFound("Book not found".to_string())
            } else {
                ApiError::Internal(format!("Failed to get previous book: {}", e))
            }
        })?;

    let prev_book = prev.ok_or_else(|| ApiError::NotFound("No previous book".to_string()))?;

    // Get series title and metadata
    let series_title = get_series_title(&state, prev_book.series_id).await?;
    let book_number = get_book_number(&state, prev_book.id).await.unwrap_or(1);

    let read_progress = if let Some(uid) = user_id {
        ReadProgressRepository::get_by_user_and_book(&state.db, uid, prev_book.id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let dto = KomgaBookDto::from_codex(
        &prev_book,
        &series_title,
        book_number,
        read_progress.as_ref(),
    );
    Ok(Json(dto))
}

/// Download book file
///
/// Streams the original book file (CBZ, CBR, EPUB, PDF) for download.
/// Includes proper Content-Disposition header with UTF-8 encoding.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/books/{bookId}/file`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
pub async fn download_book_file(
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

    // Check if file exists
    let file_path = std::path::Path::new(&book.file_path);
    if !file_path.exists() {
        return Err(ApiError::NotFound(
            "Book file not found on disk".to_string(),
        ));
    }

    // Get file metadata for content-length
    let metadata = tokio::fs::metadata(&book.file_path)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to read file metadata: {}", e)))?;

    // Determine content type based on format
    let content_type = match book.format.to_lowercase().as_str() {
        "cbz" | "zip" => "application/zip",
        "cbr" | "rar" => "application/x-rar-compressed",
        "epub" => "application/epub+zip",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    };

    // Open file for streaming
    let file = tokio::fs::File::open(&book.file_path)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to open book file: {}", e)))?;

    // Create a stream from the file
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    // Build Content-Disposition header with UTF-8 encoding (RFC 5987)
    // Format: attachment; filename="quoted-filename"; filename*=UTF-8''encoded-filename
    let filename_encoded = percent_encode_filename(&book.file_name);
    let content_disposition = format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        book.file_name.replace('"', "\\\""),
        filename_encoded
    );

    // Build response with appropriate headers
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, metadata.len())
        .header(header::CONTENT_DISPOSITION, content_disposition)
        .body(body)
        .unwrap())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get series title from series metadata or series name
async fn get_series_title(state: &Arc<AuthState>, series_id: Uuid) -> Result<String, ApiError> {
    if let Some(metadata) = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series metadata: {}", e)))?
    {
        Ok(metadata.title)
    } else {
        // Fallback to series name
        use crate::db::repositories::SeriesRepository;
        if let Some(series) = SeriesRepository::get_by_id(&state.db, series_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        {
            Ok(series.name)
        } else {
            Ok("Unknown Series".to_string())
        }
    }
}

/// Get book number from book metadata
async fn get_book_number(state: &Arc<AuthState>, book_id: Uuid) -> Option<i32> {
    BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .ok()
        .flatten()
        .and_then(|m| m.number)
        .map(|d| d.to_string().parse::<i32>().unwrap_or(1))
}

/// Percent-encode a filename for use in Content-Disposition header (RFC 5987)
///
/// Encodes characters that are not allowed in the filename* parameter:
/// - Unreserved characters (A-Z, a-z, 0-9, -._~) are preserved
/// - All other characters are percent-encoded
fn percent_encode_filename(filename: &str) -> String {
    let mut result = String::with_capacity(filename.len() * 3);
    for byte in filename.bytes() {
        match byte {
            // Unreserved characters per RFC 3986 (safe in filename*)
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                result.push(byte as char);
            }
            // Everything else gets percent-encoded
            _ => {
                result.push('%');
                result.push_str(&format!("{:02X}", byte));
            }
        }
    }
    result
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
        let query: BooksPaginationQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(query.page, 0);
        assert_eq!(query.size, 20);
        assert!(query.sort.is_none());
    }

    #[test]
    fn test_pagination_query_with_sort() {
        let query: BooksPaginationQuery =
            serde_json::from_str(r#"{"page": 1, "size": 50, "sort": "createdDate,desc"}"#).unwrap();
        assert_eq!(query.page, 1);
        assert_eq!(query.size, 50);
        assert_eq!(query.sort, Some("createdDate,desc".to_string()));
    }

    #[test]
    fn test_percent_encode_filename_ascii() {
        // Simple ASCII filename should be mostly unchanged
        assert_eq!(percent_encode_filename("test.cbz"), "test.cbz");
        assert_eq!(
            percent_encode_filename("my-file_v1.0.epub"),
            "my-file_v1.0.epub"
        );
    }

    #[test]
    fn test_percent_encode_filename_spaces() {
        // Spaces should be encoded
        assert_eq!(percent_encode_filename("My File.cbz"), "My%20File.cbz");
    }

    #[test]
    fn test_percent_encode_filename_unicode() {
        // Japanese characters should be encoded
        let encoded = percent_encode_filename("漫画 Vol 1.cbz");
        assert!(encoded.contains("%"));
        assert!(encoded.ends_with(".cbz"));
    }

    #[test]
    fn test_percent_encode_filename_special_chars() {
        // Special characters should be encoded
        assert_eq!(percent_encode_filename("file[1].cbz"), "file%5B1%5D.cbz");
    }
}
