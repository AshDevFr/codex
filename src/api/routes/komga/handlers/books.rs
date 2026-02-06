//! Komga-compatible book handlers
//!
//! Handlers for book-related endpoints in the Komga-compatible API.

use super::super::dto::book::{
    KomgaBookDto, KomgaBooksSearchRequestDto, extract_library_id_from_condition,
    extract_read_status_from_condition, extract_release_date_from_condition,
    extract_series_id_from_condition,
};
use super::super::dto::pagination::KomgaPage;
use super::libraries::{extract_page_image, generate_thumbnail};
use crate::api::{
    error::ApiError,
    extractors::{AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::repositories::{
    BookMetadataRepository, BookQueryOptions, BookQuerySort, BookRepository, BookSortField,
    ReadProgressRepository, ReadStatusFilter, ReleaseDateFilter, ReleaseDateOperator,
    SeriesMetadataRepository,
};
use crate::require_permission;
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use chrono::Datelike;
use serde::Deserialize;
use std::sync::Arc;
use tokio_util::io::ReaderStream;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Query parameters for paginated book endpoints
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct BooksPaginationQuery {
    /// Page number (0-indexed, Komga-style)
    #[serde(default)]
    pub page: i32,
    /// Page size (default: 20)
    #[serde(default = "default_page_size")]
    pub size: i32,
    /// Sort parameter (e.g., "createdDate,desc", "metadata.numberSort,asc")
    pub sort: Option<String>,
    /// Filter by library ID
    pub library_id: Option<Uuid>,
}

fn default_page_size() -> i32 {
    20
}

/// Parse Komga sort parameter into BookSortField and direction
/// Format: "field,direction" e.g., "metadata.numberSort,asc" or "createdDate,desc"
/// Also supports compound sorts like "series,metadata.numberSort,asc"
fn parse_komga_sort_param(sort: Option<&str>) -> Option<BookQuerySort> {
    let sort_str = sort?;
    let parts: Vec<&str> = sort_str.split(',').collect();

    // Handle compound sort: "series,metadata.numberSort,asc" or "series,metadata.numberSort"
    if parts.len() >= 2 && parts[0] == "series" {
        let last = parts.last().map(|s| s.to_lowercase());
        let ascending = last.as_deref() != Some("desc");
        return Some(BookQuerySort {
            field: BookSortField::Series,
            ascending,
        });
    }

    let field_str = parts.first()?;
    let ascending = parts.get(1).is_none_or(|d| d.to_lowercase() != "desc");

    // Map Komga field names to repository BookSortField
    let field = match *field_str {
        "readProgress.readDate" | "readDate" => BookSortField::LastRead,
        "metadata.releaseDate" | "releaseDate" => BookSortField::ReleaseDate,
        "metadata.numberSort" | "numberSort" => BookSortField::ChapterNumber,
        "metadata.number" | "number" => BookSortField::ChapterNumber,
        "createdDate" | "created" => BookSortField::DateAdded,
        "lastModifiedDate" | "lastModified" => BookSortField::DateAdded, // Best approximation
        "name" | "metadata.title" => BookSortField::Title,
        "media.pagesCount" | "pagesCount" => BookSortField::PageCount,
        "fileSize" | "sizeBytes" => BookSortField::FileSize,
        _ => return None, // Unknown sort field - use default
    };

    Some(BookQuerySort { field, ascending })
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
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/books/{book_id}",
    responses(
        (status = 200, description = "Book details", body = KomgaBookDto),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Book not found"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
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

    // Get book metadata
    let metadata = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .ok()
        .flatten();

    // Get book number from metadata
    let book_number = metadata
        .as_ref()
        .and_then(|m| m.number)
        .map(|d| d.to_string().parse::<i32>().unwrap_or(1))
        .unwrap_or(1);

    // Get read progress for this book and user
    let read_progress = if let Some(uid) = user_id {
        ReadProgressRepository::get_by_user_and_book(&state.db, uid, book.id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let dto = KomgaBookDto::from_codex_with_metadata(
        &book,
        &series_title,
        book_number,
        read_progress.as_ref(),
        metadata.as_ref(),
    );
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
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/books/{book_id}/thumbnail",
    responses(
        (status = 200, description = "Book thumbnail image", content_type = "image/jpeg"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Book not found or has no pages"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
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
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/books/ondeck",
    responses(
        (status = 200, description = "Paginated list of in-progress books", body = KomgaPage<KomgaBookDto>),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        BooksPaginationQuery
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn get_books_ondeck(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<BooksPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaBookDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let user_id = auth.user_id;
    let page = query.page.max(0) as u64;
    let size = query.size.clamp(1, 500) as u64;

    // On Deck = first unread book in series where user completed at least one book
    // and no books are currently in-progress. Uses the same logic as the v1 API.
    let (books, total) =
        BookRepository::list_on_deck(&state.db, user_id, query.library_id, page, size)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch on-deck books: {}", e)))?;

    // Batch-fetch book metadata for all books
    let book_ids: Vec<Uuid> = books.iter().map(|b| b.id).collect();
    let metadata_map = BookMetadataRepository::get_by_book_ids(&state.db, &book_ids)
        .await
        .unwrap_or_default();

    // Convert to DTOs - on-deck books have no read progress by definition
    let mut dtos = Vec::with_capacity(books.len());
    for book in books {
        let series_title = get_series_title(&state, book.series_id).await?;
        let meta = metadata_map.get(&book.id);
        let book_number = meta
            .and_then(|m| m.number)
            .map(|d| d.to_string().parse::<i32>().unwrap_or(1))
            .unwrap_or(1);

        let dto =
            KomgaBookDto::from_codex_with_metadata(&book, &series_title, book_number, None, meta);
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
#[utoipa::path(
    post,
    path = "/{prefix}/api/v1/books/list",
    request_body = KomgaBooksSearchRequestDto,
    responses(
        (status = 200, description = "Paginated list of books matching filter", body = KomgaPage<KomgaBookDto>),
        (status = 401, description = "Unauthorized"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        BooksPaginationQuery
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn search_books(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<BooksPaginationQuery>,
    Json(body): Json<KomgaBooksSearchRequestDto>,
) -> Result<Json<KomgaPage<KomgaBookDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let user_id = auth.user_id;
    let page = query.page.max(0) as u64;
    let size = query.size.clamp(1, 500) as u64;

    // Parse filter criteria - first try direct field, then from condition object
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

    // First try to get series_id from direct field, then from condition object
    let series_id = body
        .series_id
        .as_ref()
        .and_then(|ids| ids.first())
        .and_then(|id| Uuid::parse_str(id).ok())
        .or_else(|| {
            body.condition
                .as_ref()
                .and_then(extract_series_id_from_condition)
                .and_then(|id| Uuid::parse_str(id).ok())
        });

    // Extract readStatus from condition if present
    let read_status_str = body
        .condition
        .as_ref()
        .and_then(extract_read_status_from_condition);

    // Map Komga readStatus to repository filter
    let read_status = match read_status_str {
        Some("IN_PROGRESS") => Some(ReadStatusFilter::InProgress),
        Some("READ") => Some(ReadStatusFilter::Read),
        Some("UNREAD") => Some(ReadStatusFilter::Unread),
        _ => None,
    };

    // Extract releaseDate condition filter if present
    let release_date = body
        .condition
        .as_ref()
        .and_then(extract_release_date_from_condition)
        .and_then(|rd| {
            let operator = match rd.operator.as_str() {
                "after" => ReleaseDateOperator::After,
                "before" => ReleaseDateOperator::Before,
                _ => return None,
            };
            // Parse ISO 8601 datetime to extract year/month/day
            let dt = chrono::DateTime::parse_from_rfc3339(&rd.date_time).ok()?;
            Some(ReleaseDateFilter {
                operator,
                year: dt.year(),
                month: dt.month() as i32,
                day: dt.day() as i32,
            })
        });

    // Parse sort parameter
    let sort = parse_komga_sort_param(query.sort.as_deref());

    // Get search term from either field
    let search_term = body
        .full_text_search
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(body.search_term.as_deref().filter(|s| !s.is_empty()));

    // Use composable query with database-level sorting
    let options = BookQueryOptions {
        library_id,
        series_id,
        read_status,
        user_id: Some(user_id),
        search: search_term,
        release_date,
        include_deleted: false,
        sort,
        page,
        page_size: size,
    };

    let (books, total) = BookRepository::query(&state.db, options)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to query books: {}", e)))?;

    // Batch-fetch book metadata for all books
    let book_ids: Vec<Uuid> = books.iter().map(|b| b.id).collect();
    let metadata_map = BookMetadataRepository::get_by_book_ids(&state.db, &book_ids)
        .await
        .unwrap_or_default();

    // Convert to DTOs (no in-memory sorting needed - already sorted by database)
    let mut dtos = Vec::with_capacity(books.len());
    for book in books {
        let series_title = get_series_title(&state, book.series_id).await?;
        let meta = metadata_map.get(&book.id);
        let book_number = meta
            .and_then(|m| m.number)
            .map(|d| d.to_string().parse::<i32>().unwrap_or(1))
            .unwrap_or(1);

        let read_progress =
            ReadProgressRepository::get_by_user_and_book(&state.db, user_id, book.id)
                .await
                .ok()
                .flatten();

        let dto = KomgaBookDto::from_codex_with_metadata(
            &book,
            &series_title,
            book_number,
            read_progress.as_ref(),
            meta,
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
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/books/{book_id}/next",
    responses(
        (status = 200, description = "Next book in series", body = KomgaBookDto),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "No next book"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
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
    let metadata = BookMetadataRepository::get_by_book_id(&state.db, next_book.id)
        .await
        .ok()
        .flatten();
    let book_number = metadata
        .as_ref()
        .and_then(|m| m.number)
        .map(|d| d.to_string().parse::<i32>().unwrap_or(1))
        .unwrap_or(1);

    let read_progress = if let Some(uid) = user_id {
        ReadProgressRepository::get_by_user_and_book(&state.db, uid, next_book.id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let dto = KomgaBookDto::from_codex_with_metadata(
        &next_book,
        &series_title,
        book_number,
        read_progress.as_ref(),
        metadata.as_ref(),
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
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/books/{book_id}/previous",
    responses(
        (status = 200, description = "Previous book in series", body = KomgaBookDto),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "No previous book"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
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
    let metadata = BookMetadataRepository::get_by_book_id(&state.db, prev_book.id)
        .await
        .ok()
        .flatten();
    let book_number = metadata
        .as_ref()
        .and_then(|m| m.number)
        .map(|d| d.to_string().parse::<i32>().unwrap_or(1))
        .unwrap_or(1);

    let read_progress = if let Some(uid) = user_id {
        ReadProgressRepository::get_by_user_and_book(&state.db, uid, prev_book.id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let dto = KomgaBookDto::from_codex_with_metadata(
        &prev_book,
        &series_title,
        book_number,
        read_progress.as_ref(),
        metadata.as_ref(),
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
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/books/{book_id}/file",
    responses(
        (status = 200, description = "Book file download", content_type = "application/octet-stream"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Book not found or file missing"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
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

    #[test]
    fn test_parse_komga_sort_param_simple() {
        // Simple sort with direction
        let sort = parse_komga_sort_param(Some("createdDate,desc")).unwrap();
        assert_eq!(sort.field, BookSortField::DateAdded);
        assert!(!sort.ascending);

        let sort = parse_komga_sort_param(Some("createdDate,asc")).unwrap();
        assert_eq!(sort.field, BookSortField::DateAdded);
        assert!(sort.ascending);

        // Default to ascending if no direction
        let sort = parse_komga_sort_param(Some("createdDate")).unwrap();
        assert_eq!(sort.field, BookSortField::DateAdded);
        assert!(sort.ascending);

        // None case
        assert!(parse_komga_sort_param(None).is_none());
    }

    #[test]
    fn test_parse_komga_sort_param_komga_fields() {
        // Test Komga-specific field names
        let sort = parse_komga_sort_param(Some("readProgress.readDate,desc")).unwrap();
        assert_eq!(sort.field, BookSortField::LastRead);
        assert!(!sort.ascending);

        let sort = parse_komga_sort_param(Some("metadata.releaseDate,asc")).unwrap();
        assert_eq!(sort.field, BookSortField::ReleaseDate);
        assert!(sort.ascending);

        let sort = parse_komga_sort_param(Some("media.pagesCount,desc")).unwrap();
        assert_eq!(sort.field, BookSortField::PageCount);
        assert!(!sort.ascending);

        let sort = parse_komga_sort_param(Some("fileSize,asc")).unwrap();
        assert_eq!(sort.field, BookSortField::FileSize);
        assert!(sort.ascending);
    }

    #[test]
    fn test_parse_komga_sort_param_compound() {
        // Compound sort with direction
        let sort = parse_komga_sort_param(Some("series,metadata.numberSort,asc")).unwrap();
        assert_eq!(sort.field, BookSortField::Series);
        assert!(sort.ascending);

        let sort = parse_komga_sort_param(Some("series,metadata.numberSort,desc")).unwrap();
        assert_eq!(sort.field, BookSortField::Series);
        assert!(!sort.ascending);

        // Compound sort without direction defaults to ascending
        let sort = parse_komga_sort_param(Some("series,metadata.numberSort")).unwrap();
        assert_eq!(sort.field, BookSortField::Series);
        assert!(sort.ascending);
    }

    #[test]
    fn test_parse_komga_sort_param_unknown() {
        // Unknown sort field returns None
        assert!(parse_komga_sort_param(Some("unknownField,asc")).is_none());
    }
}
