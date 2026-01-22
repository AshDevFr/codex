use super::super::dto::{
    book::{BookSortField, BookSortParam},
    AdjacentBooksResponse, BookDetailResponse, BookDto, BookListRequest, BookListResponse,
    BookMetadataDto, PaginationParams, SortDirection,
};
use crate::api::{
    error::ApiError,
    extractors::{AuthContext, AuthState, ContentFilter, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::repositories::{
    BookMetadataRepository, BookRepository, LibraryRepository, ReadProgressRepository,
    SeriesMetadataRepository,
};
use crate::require_permission;
use crate::services::FilterService;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

/// Query parameters for listing books
#[derive(Debug, Deserialize)]
pub struct BookListQuery {
    /// Optional library filter
    #[serde(default)]
    pub library_id: Option<Uuid>,

    /// Optional series filter
    #[serde(default)]
    pub series_id: Option<Uuid>,

    /// Page number (0-indexed)
    #[serde(default)]
    pub page: u64,

    /// Number of items per page (max 100)
    #[serde(default = "default_page_size")]
    pub page_size: u64,

    /// Sort parameter (format: "field,direction" e.g. "title,asc")
    #[serde(default)]
    pub sort: Option<String>,
}

/// Query parameters for listing books with analysis errors
#[derive(Debug, Deserialize)]
pub struct BooksWithErrorsQuery {
    /// Optional library filter
    #[serde(default)]
    pub library_id: Option<Uuid>,

    /// Optional series filter
    #[serde(default)]
    pub series_id: Option<Uuid>,

    /// Page number (0-indexed)
    #[serde(default)]
    pub page: u64,

    /// Number of items per page (max 100)
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

fn default_page_size() -> u64 {
    20
}

/// Helper function to convert books to DTOs with series information and read progress
pub async fn books_to_dtos(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    books: Vec<crate::db::entities::books::Model>,
) -> Result<Vec<BookDto>, ApiError> {
    // Collect unique series IDs and library IDs
    let series_ids: Vec<Uuid> = books
        .iter()
        .map(|b| b.series_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let library_ids: Vec<Uuid> = books
        .iter()
        .map(|b| b.library_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Collect book IDs for metadata lookup
    let book_ids: Vec<Uuid> = books.iter().map(|b| b.id).collect();

    // Fetch series metadata (contains title, reading direction, etc.)
    let mut series_metadata_map: HashMap<Uuid, crate::db::entities::series_metadata::Model> =
        HashMap::new();
    for series_id in &series_ids {
        if let Ok(Some(metadata)) = SeriesMetadataRepository::get_by_series_id(db, *series_id).await
        {
            series_metadata_map.insert(*series_id, metadata);
        }
    }

    // Fetch book metadata for all books (contains title, number, etc.)
    let mut book_metadata_map: HashMap<Uuid, crate::db::entities::book_metadata::Model> =
        HashMap::new();
    for book_id in &book_ids {
        if let Ok(Some(metadata)) = BookMetadataRepository::get_by_book_id(db, *book_id).await {
            book_metadata_map.insert(*book_id, metadata);
        }
    }

    // Fetch libraries for name and default reading direction fallback
    let mut library_map: HashMap<Uuid, crate::db::entities::libraries::Model> = HashMap::new();
    for library_id in &library_ids {
        if let Ok(Some(library)) = LibraryRepository::get_by_id(db, *library_id).await {
            library_map.insert(*library_id, library);
        }
    }

    // Fetch read progress for all books
    let mut progress_map = HashMap::new();
    for book in &books {
        if let Ok(Some(progress)) =
            ReadProgressRepository::get_by_user_and_book(db, user_id, book.id).await
        {
            progress_map.insert(book.id, progress.into());
        }
    }

    // Convert books to DTOs
    let dtos = books
        .into_iter()
        .map(|book| {
            // Get library info
            let library = library_map.get(&book.library_id);
            let library_name = library
                .map(|l| l.name.clone())
                .unwrap_or_else(|| "Unknown Library".to_string());

            // Get series name from series_metadata.title
            let series_name = series_metadata_map
                .get(&book.series_id)
                .map(|m| m.title.clone())
                .unwrap_or_else(|| "Unknown Series".to_string());

            // Get book metadata
            let book_meta = book_metadata_map.get(&book.id);

            // Use title from book_metadata if available, otherwise use file_name (without extension)
            let title = book_meta.and_then(|m| m.title.clone()).unwrap_or_else(|| {
                // Extract filename without extension
                let file_name = &book.file_name;
                if let Some(pos) = file_name.rfind('.') {
                    file_name[..pos].to_string()
                } else {
                    file_name.clone()
                }
            });

            // Get title_sort from book_metadata
            let title_sort = book_meta.and_then(|m| m.title_sort.clone());

            // Get number from book_metadata
            let number = book_meta
                .and_then(|m| m.number)
                .map(|d| d.to_string().parse::<i32>().unwrap_or(0));

            let read_progress = progress_map.get(&book.id).cloned();

            // Determine effective reading direction: series metadata > library default
            let reading_direction = series_metadata_map
                .get(&book.series_id)
                .and_then(|m| m.reading_direction.clone())
                .or_else(|| library.map(|l| l.default_reading_direction.clone()));

            BookDto {
                id: book.id,
                library_id: book.library_id,
                library_name,
                series_id: book.series_id,
                series_name,
                title,
                title_sort,
                file_path: book.file_path,
                file_format: book.format,
                file_size: book.file_size,
                file_hash: book.file_hash,
                page_count: book.page_count,
                number,
                created_at: book.created_at,
                updated_at: book.updated_at,
                read_progress,
                analysis_error: book.analysis_error,
                deleted: book.deleted,
                reading_direction,
            }
        })
        .collect();

    Ok(dtos)
}

/// List books with pagination
#[utoipa::path(
    get,
    path = "/api/v1/books",
    params(
        PaginationParams,
        ("series_id" = Option<Uuid>, Query, description = "Filter by series ID")
    ),
    responses(
        (status = 200, description = "Paginated list of books", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn list_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<BookListQuery>,
) -> Result<Json<BookListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(100)
    };

    // Load content filter for sharing tags
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    // Fetch books based on filter
    let (books_list, total) = if let Some(ser_id) = query.series_id {
        // Check if the series is visible to the user
        if !content_filter.is_series_visible(ser_id) {
            return Ok(Json(BookListResponse::new(
                vec![],
                query.page,
                page_size,
                0,
            )));
        }

        // By default, don't include deleted books in API responses
        let books = BookRepository::list_by_series(&state.db, ser_id, false)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;
        let total = books.len() as u64;

        // Apply pagination manually
        let offset = query.page * page_size;
        let start = offset as usize;

        // If start is beyond the list, return empty results
        let paginated = if start >= books.len() {
            vec![]
        } else {
            let end = (start + page_size as usize).min(books.len());
            books[start..end].to_vec()
        };

        (paginated, total)
    } else {
        // List all books with pagination, then filter by sharing tags
        // Use i64::MAX as page_size to avoid SQLite integer overflow (u64::MAX > i64::MAX)
        let (books, _) = BookRepository::list_all(
            &state.db,
            false, // exclude deleted
            0,
            i64::MAX as u64,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

        // Filter books by sharing tags
        let filtered: Vec<_> = books
            .into_iter()
            .filter(|b| content_filter.is_book_visible(b.series_id))
            .collect();

        let total = filtered.len() as u64;

        // Apply pagination
        let offset = query.page * page_size;
        let start = offset as usize;

        let paginated = if start >= filtered.len() {
            vec![]
        } else {
            let end = (start + page_size as usize).min(filtered.len());
            filtered[start..end].to_vec()
        };

        (paginated, total)
    };

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    let response = BookListResponse::new(dtos, query.page, page_size, total);

    Ok(Json(response))
}

/// List books with advanced filtering
///
/// Supports complex filter conditions including nested AllOf/AnyOf logic,
/// genre/tag filtering with include/exclude, and more.
#[utoipa::path(
    post,
    path = "/api/v1/books/list",
    request_body = BookListRequest,
    responses(
        (status = 200, description = "Paginated list of filtered books", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn list_books_filtered(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<BookListRequest>,
) -> Result<Json<BookListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params
    let page_size = if request.page_size == 0 {
        default_page_size()
    } else {
        request.page_size.min(100)
    };

    // If there's a condition, evaluate it to get matching book IDs (with user context for ReadStatus filtering)
    let filtered_ids: Option<HashSet<Uuid>> = if let Some(ref condition) = request.condition {
        let matching = FilterService::get_matching_books_for_user(
            &state.db,
            condition,
            None,
            Some(auth.user_id),
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to evaluate filter: {}", e)))?;
        Some(matching)
    } else {
        None
    };

    // Fetch books based on filter results and full-text search
    let (books_list, total) = match (&filtered_ids, &request.full_text_search) {
        // Full-text search with filter conditions
        (Some(ids), Some(search_query)) if !search_query.trim().is_empty() => {
            if ids.is_empty() {
                (vec![], 0)
            } else {
                let id_vec: Vec<Uuid> = ids.iter().cloned().collect();
                BookRepository::full_text_search_filtered(
                    &state.db,
                    search_query,
                    &id_vec,
                    request.include_deleted,
                    request.page,
                    page_size,
                )
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to search books: {}", e)))?
            }
        }
        // Full-text search without filter conditions
        (None, Some(search_query)) if !search_query.trim().is_empty() => {
            BookRepository::full_text_search(
                &state.db,
                search_query,
                request.include_deleted,
                request.page,
                page_size,
            )
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to search books: {}", e)))?
        }
        // Filter conditions only (no full-text search)
        (Some(ids), _) => {
            if ids.is_empty() {
                (vec![], 0)
            } else {
                let id_vec: Vec<Uuid> = ids.iter().cloned().collect();
                BookRepository::list_by_ids(
                    &state.db,
                    &id_vec,
                    request.include_deleted,
                    request.page,
                    page_size,
                )
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?
            }
        }
        // No filter and no full-text search
        (None, _) => {
            BookRepository::list_all(&state.db, request.include_deleted, request.page, page_size)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?
        }
    };

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    let response = BookListResponse::new(dtos, request.page, page_size, total);

    Ok(Json(response))
}

/// List books with analysis errors
#[utoipa::path(
    get,
    path = "/api/v1/books/with-errors",
    params(
        ("library_id" = Option<Uuid>, Query, description = "Filter by library ID"),
        ("series_id" = Option<Uuid>, Query, description = "Filter by series ID"),
        ("page" = Option<u64>, Query, description = "Page number (0-indexed)"),
        ("page_size" = Option<u64>, Query, description = "Number of items per page (max 100)")
    ),
    responses(
        (status = 200, description = "Paginated list of books with analysis errors", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn list_books_with_errors(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<BooksWithErrorsQuery>,
) -> Result<Json<BookListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(100)
    };

    // Fetch books with errors
    let (books_list, total) = BookRepository::list_with_errors(
        &state.db,
        query.library_id,
        query.series_id,
        query.page,
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch books with errors: {}", e)))?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    let response = BookListResponse::new(dtos, query.page, page_size, total);

    Ok(Json(response))
}

/// List books with analysis errors in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/books/with-errors",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        ("page" = Option<u64>, Query, description = "Page number (0-indexed)"),
        ("page_size" = Option<u64>, Query, description = "Number of items per page (max 100)")
    ),
    responses(
        (status = 200, description = "Paginated list of books with analysis errors in library", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn list_library_books_with_errors(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<BooksWithErrorsQuery>,
) -> Result<Json<BookListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(100)
    };

    let (books_list, total) =
        BookRepository::list_with_errors(&state.db, Some(library_id), None, query.page, page_size)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch library books with errors: {}", e))
            })?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    let response = BookListResponse::new(dtos, query.page, page_size, total);

    Ok(Json(response))
}

/// List books with analysis errors in a specific series
#[utoipa::path(
    get,
    path = "/api/v1/series/{series_id}/books/with-errors",
    params(
        ("series_id" = Uuid, Path, description = "Series ID"),
        ("page" = Option<u64>, Query, description = "Page number (0-indexed)"),
        ("page_size" = Option<u64>, Query, description = "Number of items per page (max 100)")
    ),
    responses(
        (status = 200, description = "Paginated list of books with analysis errors in series", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn list_series_books_with_errors(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Query(query): Query<BooksWithErrorsQuery>,
) -> Result<Json<BookListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(100)
    };

    let (books_list, total) =
        BookRepository::list_with_errors(&state.db, None, Some(series_id), query.page, page_size)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch series books with errors: {}", e))
            })?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    let response = BookListResponse::new(dtos, query.page, page_size, total);

    Ok(Json(response))
}

/// Get book by ID
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    responses(
        (status = 200, description = "Book details", body = BookDetailResponse),
        (status = 404, description = "Book not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn get_book(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<BookDetailResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Check sharing tag access for the book's series
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    if !content_filter.is_book_visible(book.series_id) {
        return Err(ApiError::NotFound("Book not found".to_string()));
    }

    // Try to fetch metadata - now contains title, title_sort, number
    let metadata = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .ok()
        .flatten()
        .map(|meta| BookMetadataDto {
            id: meta.id,
            book_id: meta.book_id,
            title: meta.title,
            series: None, // Series name is fetched separately via series_metadata
            number: meta.number.map(|d| d.to_string()),
            summary: meta.summary,
            publisher: meta.publisher,
            imprint: meta.imprint,
            genre: meta.genre,
            page_count: None, // Page count is in books table, not metadata
            language_iso: meta.language_iso,
            release_date: None, // Release date is computed from year/month/day
            writers: meta.writer.map(|s| vec![s]).unwrap_or_default(),
            pencillers: meta.penciller.map(|s| vec![s]).unwrap_or_default(),
            inkers: meta.inker.map(|s| vec![s]).unwrap_or_default(),
            colorists: meta.colorist.map(|s| vec![s]).unwrap_or_default(),
            letterers: meta.letterer.map(|s| vec![s]).unwrap_or_default(),
            cover_artists: meta.cover_artist.map(|s| vec![s]).unwrap_or_default(),
            editors: meta.editor.map(|s| vec![s]).unwrap_or_default(),
        });

    let mut dtos = books_to_dtos(&state.db, auth.user_id, vec![book]).await?;
    let book_dto = dtos.pop().unwrap(); // Safe because we just passed a single book

    let response = BookDetailResponse {
        book: book_dto,
        metadata,
    };

    Ok(Json(response))
}

/// Update book core fields (title, number)
///
/// Partially updates book_metadata fields. Only provided fields will be updated.
/// Absent fields are unchanged. Explicitly null fields will be cleared.
/// When a field is set to a non-null value, it is automatically locked.
#[utoipa::path(
    patch,
    path = "/api/v1/books/{book_id}",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = PatchBookRequest,
    responses(
        (status = 200, description = "Book updated successfully", body = BookUpdateResponse),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn patch_book(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<PatchBookRequest>,
) -> Result<Json<BookUpdateResponse>, ApiError> {
    use sea_orm::prelude::Decimal;

    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    let now = Utc::now();
    let mut has_changes = false;

    // Get or create book_metadata record
    let existing_meta = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let updated_meta = if let Some(existing) = existing_meta {
        // Update existing metadata record
        let mut active: book_metadata::ActiveModel = existing.into();

        // Update title if provided (also lock it when set to non-null)
        if let Some(opt) = request.title.into_nested_option() {
            active.title = Set(opt.clone());
            if opt.is_some() {
                active.title_lock = Set(true);
            }
            has_changes = true;
        }

        // Update number if provided (convert f64 to Decimal, also lock when set)
        if let Some(opt) = request.number.into_nested_option() {
            let decimal_opt = opt.and_then(Decimal::from_f64_retain);
            active.number = Set(decimal_opt);
            if opt.is_some() {
                active.number_lock = Set(true);
            }
            has_changes = true;
        }

        if has_changes {
            active.updated_at = Set(now);
        }

        active
            .update(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update book metadata: {}", e)))?
    } else {
        // Create new metadata record with provided fields
        has_changes = true;
        let title_opt = request.title.into_option();
        let number_opt = request.number.into_option();
        let decimal_opt = number_opt.and_then(Decimal::from_f64_retain);

        let active = book_metadata::ActiveModel {
            id: Set(Uuid::new_v4()),
            book_id: Set(book_id),
            title: Set(title_opt.clone()),
            title_sort: Set(None),
            number: Set(decimal_opt),
            summary: Set(None),
            writer: Set(None),
            penciller: Set(None),
            inker: Set(None),
            colorist: Set(None),
            letterer: Set(None),
            cover_artist: Set(None),
            editor: Set(None),
            publisher: Set(None),
            imprint: Set(None),
            genre: Set(None),
            web: Set(None),
            language_iso: Set(None),
            format_detail: Set(None),
            black_and_white: Set(None),
            manga: Set(None),
            year: Set(None),
            month: Set(None),
            day: Set(None),
            volume: Set(None),
            count: Set(None),
            isbns: Set(None),
            // Auto-lock fields that are set
            title_lock: Set(title_opt.is_some()),
            title_sort_lock: Set(false),
            number_lock: Set(number_opt.is_some()),
            summary_lock: Set(false),
            writer_lock: Set(false),
            penciller_lock: Set(false),
            inker_lock: Set(false),
            colorist_lock: Set(false),
            letterer_lock: Set(false),
            cover_artist_lock: Set(false),
            editor_lock: Set(false),
            publisher_lock: Set(false),
            imprint_lock: Set(false),
            genre_lock: Set(false),
            web_lock: Set(false),
            language_iso_lock: Set(false),
            format_detail_lock: Set(false),
            black_and_white_lock: Set(false),
            manga_lock: Set(false),
            year_lock: Set(false),
            month_lock: Set(false),
            day_lock: Set(false),
            volume_lock: Set(false),
            count_lock: Set(false),
            isbns_lock: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        };

        active
            .insert(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to create book metadata: {}", e)))?
    };

    // Emit update event
    if has_changes {
        let event = EntityChangeEvent {
            event: EntityEvent::BookUpdated {
                book_id,
                series_id: book.series_id,
                library_id: book.library_id,
                fields: Some(vec!["title".to_string(), "number".to_string()]),
            },
            timestamp: now,
            user_id: Some(auth.user_id),
        };
        let _ = state.event_broadcaster.emit(event);
    }

    Ok(Json(BookUpdateResponse {
        id: book_id,
        title: updated_meta.title,
        number: updated_meta
            .number
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
        updated_at: updated_meta.updated_at,
    }))
}

/// Get adjacent books in the same series
///
/// Returns the previous and next books relative to the requested book,
/// ordered by book number within the series.
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/adjacent",
    params(
        ("book_id" = Uuid, Path, description = "Book ID"),
    ),
    responses(
        (status = 200, description = "Adjacent books", body = AdjacentBooksResponse),
        (status = 404, description = "Book not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn get_adjacent_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<AdjacentBooksResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let (prev, next) = BookRepository::get_adjacent_in_series(&state.db, book_id)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                ApiError::NotFound("Book not found".to_string())
            } else {
                ApiError::Internal(format!("Failed to get adjacent books: {}", e))
            }
        })?;

    // Convert to DTOs
    let prev_dto = if let Some(book) = prev {
        let mut dtos = books_to_dtos(&state.db, auth.user_id, vec![book]).await?;
        dtos.pop()
    } else {
        None
    };

    let next_dto = if let Some(book) = next {
        let mut dtos = books_to_dtos(&state.db, auth.user_id, vec![book]).await?;
        dtos.pop()
    } else {
        None
    };

    Ok(Json(AdjacentBooksResponse {
        prev: prev_dto,
        next: next_dto,
    }))
}

/// List books in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/books",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        PaginationParams,
    ),
    responses(
        (status = 200, description = "Paginated list of books in library", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn list_library_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<BookListQuery>,
) -> Result<Json<BookListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

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
        .map(|s| BookSortParam::parse(s))
        .unwrap_or_default();

    // Use database-level sorting for all sort types
    let (books_list, total) = BookRepository::list_by_library_sorted(
        &state.db, library_id, &sort, false, // exclude deleted
        query.page, page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch library books: {}", e)))?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    let response = BookListResponse::new(dtos, query.page, page_size, total);

    Ok(Json(response))
}

/// List books with reading progress (in-progress books)
#[utoipa::path(
    get,
    path = "/api/v1/books/in-progress",
    params(
        ("library_id" = Option<Uuid>, Query, description = "Filter by library ID"),
        PaginationParams,
    ),
    responses(
        (status = 200, description = "Paginated list of in-progress books", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn list_in_progress_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<BookListQuery>,
) -> Result<Json<BookListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(100)
    };

    // Fetch books with reading progress (not completed)
    let (books_list, total) = BookRepository::list_with_progress(
        &state.db,
        auth.user_id,
        query.library_id,
        Some(false), // only in-progress (not completed)
        query.page,
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch in-progress books: {}", e)))?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    let response = BookListResponse::new(dtos, query.page, page_size, total);

    Ok(Json(response))
}

/// List books with reading progress in a specific library (in-progress books)
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/books/in-progress",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        PaginationParams,
    ),
    responses(
        (status = 200, description = "Paginated list of in-progress books in library", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn list_library_in_progress_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<BookListQuery>,
) -> Result<Json<BookListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(100)
    };

    // Fetch books with reading progress (not completed) in this library
    let (books_list, total) = BookRepository::list_with_progress(
        &state.db,
        auth.user_id,
        Some(library_id),
        Some(false), // only in-progress (not completed)
        query.page,
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch in-progress books: {}", e)))?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    let response = BookListResponse::new(dtos, query.page, page_size, total);

    Ok(Json(response))
}

/// List on-deck books (next unread book in series where user has completed at least one book)
#[utoipa::path(
    get,
    path = "/api/v1/books/on-deck",
    params(
        ("library_id" = Option<Uuid>, Query, description = "Filter by library ID"),
        PaginationParams,
    ),
    responses(
        (status = 200, description = "Paginated list of on-deck books", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn list_on_deck_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<BookListQuery>,
) -> Result<Json<BookListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(100)
    };

    // Fetch on-deck books
    let (books_list, total) = BookRepository::list_on_deck(
        &state.db,
        auth.user_id,
        query.library_id,
        query.page,
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch on-deck books: {}", e)))?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    let response = BookListResponse::new(dtos, query.page, page_size, total);

    Ok(Json(response))
}

/// List on-deck books in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/books/on-deck",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        PaginationParams,
    ),
    responses(
        (status = 200, description = "Paginated list of on-deck books in library", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn list_library_on_deck_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<BookListQuery>,
) -> Result<Json<BookListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(100)
    };

    // Fetch on-deck books in this library
    let (books_list, total) = BookRepository::list_on_deck(
        &state.db,
        auth.user_id,
        Some(library_id),
        query.page,
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch on-deck books: {}", e)))?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    let response = BookListResponse::new(dtos, query.page, page_size, total);

    Ok(Json(response))
}

/// List recently added books
#[utoipa::path(
    get,
    path = "/api/v1/books/recently-added",
    params(
        ("library_id" = Option<Uuid>, Query, description = "Filter by library ID"),
        PaginationParams,
    ),
    responses(
        (status = 200, description = "Paginated list of recently added books", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn list_recently_added_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<BookListQuery>,
) -> Result<Json<BookListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(100)
    };

    // Fetch recently added books
    let (books_list, total) = BookRepository::list_recently_added(
        &state.db,
        query.library_id,
        false, // exclude deleted
        query.page,
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch recently added books: {}", e)))?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    let response = BookListResponse::new(dtos, query.page, page_size, total);

    Ok(Json(response))
}

/// List recently added books in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/books/recently-added",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        PaginationParams,
    ),
    responses(
        (status = 200, description = "Paginated list of recently added books in library", body = BookListResponse),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn list_library_recently_added_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<BookListQuery>,
) -> Result<Json<BookListResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Validate and normalize pagination params
    let page_size = if query.page_size == 0 {
        default_page_size()
    } else {
        query.page_size.min(100)
    };

    // Fetch recently added books in this library
    let (books_list, total) = BookRepository::list_recently_added(
        &state.db,
        Some(library_id),
        false, // exclude deleted
        query.page,
        page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch recently added books: {}", e)))?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    let response = BookListResponse::new(dtos, query.page, page_size, total);

    Ok(Json(response))
}

/// Query parameters for recently read books
#[derive(Debug, Deserialize)]
pub struct RecentBooksQuery {
    /// Maximum number of books to return (default: 50)
    #[serde(default = "default_recent_limit")]
    pub limit: u64,

    /// Filter by library ID (optional)
    #[serde(default)]
    pub library_id: Option<Uuid>,
}

fn default_recent_limit() -> u64 {
    50
}

/// List recently read books (ordered by last read activity)
#[utoipa::path(
    get,
    path = "/api/v1/books/recently-read",
    params(
        ("limit" = Option<u64>, Query, description = "Maximum number of books to return (default: 50)"),
        ("library_id" = Option<Uuid>, Query, description = "Filter by library ID")
    ),
    responses(
        (status = 200, description = "List of recently read books", body = Vec<BookDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn list_recently_read_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(query): Query<RecentBooksQuery>,
) -> Result<Json<Vec<BookDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let books_list =
        BookRepository::list_recently_read(&state.db, auth.user_id, query.library_id, query.limit)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch recently read books: {}", e))
            })?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    Ok(Json(dtos))
}

/// List recently read books in a specific library
#[utoipa::path(
    get,
    path = "/api/v1/libraries/{library_id}/books/recently-read",
    params(
        ("library_id" = Uuid, Path, description = "Library ID"),
        ("limit" = Option<u64>, Query, description = "Maximum number of books to return (default: 50)")
    ),
    responses(
        (status = 200, description = "List of recently read books in library", body = Vec<BookDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn list_library_recently_read_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(library_id): Path<Uuid>,
    Query(query): Query<RecentBooksQuery>,
) -> Result<Json<Vec<BookDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let books_list =
        BookRepository::list_recently_read(&state.db, auth.user_id, Some(library_id), query.limit)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch recently read books: {}", e))
            })?;

    let dtos = books_to_dtos(&state.db, auth.user_id, books_list).await?;

    Ok(Json(dtos))
}

/// Download book file
///
/// Streams the original book file (CBZ, CBR, EPUB, PDF) for download.
/// Used by OPDS clients for acquisition links.
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/file",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    responses(
        (status = 200, description = "Book file", content_type = "application/octet-stream"),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn get_book_file(
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

    // Check sharing tag access for the book's series
    let content_filter = ContentFilter::for_user(&state.db, auth.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))?;

    if !content_filter.is_book_visible(book.series_id) {
        return Err(ApiError::NotFound("Book not found".to_string()));
    }

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

    // Build response with appropriate headers
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, metadata.len())
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", book.file_name),
        )
        .body(body)
        .unwrap())
}

// ============================================================================
// Book Metadata Endpoints
// ============================================================================

use crate::api::routes::v1::dto::{
    BookMetadataResponse, PatchBookMetadataRequest, ReplaceBookMetadataRequest,
};
use crate::db::entities::book_metadata;
use crate::events::{EntityChangeEvent, EntityEvent};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, Set};

/// Replace all book metadata (PUT)
///
/// Completely replaces all metadata fields. Omitted or null fields will be cleared.
/// If no metadata record exists, one will be created.
#[utoipa::path(
    put,
    path = "/api/v1/books/{book_id}/metadata",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = ReplaceBookMetadataRequest,
    responses(
        (status = 200, description = "Metadata replaced successfully", body = BookMetadataResponse),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn replace_book_metadata(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<ReplaceBookMetadataRequest>,
) -> Result<Json<BookMetadataResponse>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Check if metadata record exists
    let existing = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let now = Utc::now();
    let updated = if let Some(existing) = existing {
        // Update existing record - full replacement
        // Auto-lock fields that are being set to non-null values
        let mut active: book_metadata::ActiveModel = existing.into();

        active.summary = Set(request.summary.clone());
        active.writer = Set(request.writer.clone());
        active.penciller = Set(request.penciller.clone());
        active.inker = Set(request.inker.clone());
        active.colorist = Set(request.colorist.clone());
        active.letterer = Set(request.letterer.clone());
        active.cover_artist = Set(request.cover_artist.clone());
        active.editor = Set(request.editor.clone());
        active.publisher = Set(request.publisher.clone());
        active.imprint = Set(request.imprint.clone());
        active.genre = Set(request.genre.clone());
        active.web = Set(request.web.clone());
        active.language_iso = Set(request.language_iso.clone());
        active.format_detail = Set(request.format_detail.clone());
        active.black_and_white = Set(request.black_and_white);
        active.manga = Set(request.manga);
        active.year = Set(request.year);
        active.month = Set(request.month);
        active.day = Set(request.day);
        active.volume = Set(request.volume);
        active.count = Set(request.count);
        active.isbns = Set(request.isbns.clone());

        // Auto-lock fields that are being set to non-null values
        if request.summary.is_some() {
            active.summary_lock = Set(true);
        }
        if request.writer.is_some() {
            active.writer_lock = Set(true);
        }
        if request.penciller.is_some() {
            active.penciller_lock = Set(true);
        }
        if request.inker.is_some() {
            active.inker_lock = Set(true);
        }
        if request.colorist.is_some() {
            active.colorist_lock = Set(true);
        }
        if request.letterer.is_some() {
            active.letterer_lock = Set(true);
        }
        if request.cover_artist.is_some() {
            active.cover_artist_lock = Set(true);
        }
        if request.editor.is_some() {
            active.editor_lock = Set(true);
        }
        if request.publisher.is_some() {
            active.publisher_lock = Set(true);
        }
        if request.imprint.is_some() {
            active.imprint_lock = Set(true);
        }
        if request.genre.is_some() {
            active.genre_lock = Set(true);
        }
        if request.web.is_some() {
            active.web_lock = Set(true);
        }
        if request.language_iso.is_some() {
            active.language_iso_lock = Set(true);
        }
        if request.format_detail.is_some() {
            active.format_detail_lock = Set(true);
        }
        if request.black_and_white.is_some() {
            active.black_and_white_lock = Set(true);
        }
        if request.manga.is_some() {
            active.manga_lock = Set(true);
        }
        if request.year.is_some() {
            active.year_lock = Set(true);
        }
        if request.month.is_some() {
            active.month_lock = Set(true);
        }
        if request.day.is_some() {
            active.day_lock = Set(true);
        }
        if request.volume.is_some() {
            active.volume_lock = Set(true);
        }
        if request.count.is_some() {
            active.count_lock = Set(true);
        }
        if request.isbns.is_some() {
            active.isbns_lock = Set(true);
        }

        active.updated_at = Set(now);

        active
            .update(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update metadata: {}", e)))?
    } else {
        // Create new record with locks set for non-null fields
        let active = book_metadata::ActiveModel {
            id: Set(Uuid::new_v4()),
            book_id: Set(book_id),
            title: Set(None), // Title is not set via this endpoint (use PATCH /books/{id})
            title_sort: Set(None), // Title sort is not set via this endpoint
            number: Set(None), // Number is not set via this endpoint (use PATCH /books/{id})
            summary: Set(request.summary.clone()),
            writer: Set(request.writer.clone()),
            penciller: Set(request.penciller.clone()),
            inker: Set(request.inker.clone()),
            colorist: Set(request.colorist.clone()),
            letterer: Set(request.letterer.clone()),
            cover_artist: Set(request.cover_artist.clone()),
            editor: Set(request.editor.clone()),
            publisher: Set(request.publisher.clone()),
            imprint: Set(request.imprint.clone()),
            genre: Set(request.genre.clone()),
            web: Set(request.web.clone()),
            language_iso: Set(request.language_iso.clone()),
            format_detail: Set(request.format_detail.clone()),
            black_and_white: Set(request.black_and_white),
            manga: Set(request.manga),
            year: Set(request.year),
            month: Set(request.month),
            day: Set(request.day),
            volume: Set(request.volume),
            count: Set(request.count),
            isbns: Set(request.isbns.clone()),
            // Set locks for non-null fields
            title_lock: Set(false),
            title_sort_lock: Set(false),
            number_lock: Set(false),
            summary_lock: Set(request.summary.is_some()),
            writer_lock: Set(request.writer.is_some()),
            penciller_lock: Set(request.penciller.is_some()),
            inker_lock: Set(request.inker.is_some()),
            colorist_lock: Set(request.colorist.is_some()),
            letterer_lock: Set(request.letterer.is_some()),
            cover_artist_lock: Set(request.cover_artist.is_some()),
            editor_lock: Set(request.editor.is_some()),
            publisher_lock: Set(request.publisher.is_some()),
            imprint_lock: Set(request.imprint.is_some()),
            genre_lock: Set(request.genre.is_some()),
            web_lock: Set(request.web.is_some()),
            language_iso_lock: Set(request.language_iso.is_some()),
            format_detail_lock: Set(request.format_detail.is_some()),
            black_and_white_lock: Set(request.black_and_white.is_some()),
            manga_lock: Set(request.manga.is_some()),
            year_lock: Set(request.year.is_some()),
            month_lock: Set(request.month.is_some()),
            day_lock: Set(request.day.is_some()),
            volume_lock: Set(request.volume.is_some()),
            count_lock: Set(request.count.is_some()),
            isbns_lock: Set(request.isbns.is_some()),
            created_at: Set(now),
            updated_at: Set(now),
        };

        active
            .insert(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to create metadata: {}", e)))?
    };

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::BookUpdated {
            book_id,
            series_id: book.series_id,
            library_id: book.library_id,
            fields: Some(vec!["metadata".to_string()]),
        },
        timestamp: now,
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(BookMetadataResponse {
        book_id: updated.book_id,
        summary: updated.summary,
        writer: updated.writer,
        penciller: updated.penciller,
        inker: updated.inker,
        colorist: updated.colorist,
        letterer: updated.letterer,
        cover_artist: updated.cover_artist,
        editor: updated.editor,
        publisher: updated.publisher,
        imprint: updated.imprint,
        genre: updated.genre,
        web: updated.web,
        language_iso: updated.language_iso,
        format_detail: updated.format_detail,
        black_and_white: updated.black_and_white,
        manga: updated.manga,
        year: updated.year,
        month: updated.month,
        day: updated.day,
        volume: updated.volume,
        count: updated.count,
        isbns: updated.isbns,
        updated_at: updated.updated_at,
    }))
}

/// Partially update book metadata (PATCH)
///
/// Only provided fields will be updated. Absent fields are unchanged.
/// Explicitly null fields will be cleared.
/// If no metadata record exists, one will be created with the provided fields.
#[utoipa::path(
    patch,
    path = "/api/v1/books/{book_id}/metadata",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = PatchBookMetadataRequest,
    responses(
        (status = 200, description = "Metadata updated successfully", body = BookMetadataResponse),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn patch_book_metadata(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<PatchBookMetadataRequest>,
) -> Result<Json<BookMetadataResponse>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Check if metadata record exists
    let existing = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?;

    let now = Utc::now();
    let mut has_changes = false;

    let updated = if let Some(existing) = existing {
        // Partial update existing record with auto-locking
        let mut active: book_metadata::ActiveModel = existing.into();

        if let Some(opt) = request.summary.into_nested_option() {
            active.summary = Set(opt.clone());
            if opt.is_some() {
                active.summary_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.writer.into_nested_option() {
            active.writer = Set(opt.clone());
            if opt.is_some() {
                active.writer_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.penciller.into_nested_option() {
            active.penciller = Set(opt.clone());
            if opt.is_some() {
                active.penciller_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.inker.into_nested_option() {
            active.inker = Set(opt.clone());
            if opt.is_some() {
                active.inker_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.colorist.into_nested_option() {
            active.colorist = Set(opt.clone());
            if opt.is_some() {
                active.colorist_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.letterer.into_nested_option() {
            active.letterer = Set(opt.clone());
            if opt.is_some() {
                active.letterer_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.cover_artist.into_nested_option() {
            active.cover_artist = Set(opt.clone());
            if opt.is_some() {
                active.cover_artist_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.editor.into_nested_option() {
            active.editor = Set(opt.clone());
            if opt.is_some() {
                active.editor_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.publisher.into_nested_option() {
            active.publisher = Set(opt.clone());
            if opt.is_some() {
                active.publisher_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.imprint.into_nested_option() {
            active.imprint = Set(opt.clone());
            if opt.is_some() {
                active.imprint_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.genre.into_nested_option() {
            active.genre = Set(opt.clone());
            if opt.is_some() {
                active.genre_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.web.into_nested_option() {
            active.web = Set(opt.clone());
            if opt.is_some() {
                active.web_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.language_iso.into_nested_option() {
            active.language_iso = Set(opt.clone());
            if opt.is_some() {
                active.language_iso_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.format_detail.into_nested_option() {
            active.format_detail = Set(opt.clone());
            if opt.is_some() {
                active.format_detail_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.black_and_white.into_nested_option() {
            active.black_and_white = Set(opt);
            if opt.is_some() {
                active.black_and_white_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.manga.into_nested_option() {
            active.manga = Set(opt);
            if opt.is_some() {
                active.manga_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.year.into_nested_option() {
            active.year = Set(opt);
            if opt.is_some() {
                active.year_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.month.into_nested_option() {
            active.month = Set(opt);
            if opt.is_some() {
                active.month_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.day.into_nested_option() {
            active.day = Set(opt);
            if opt.is_some() {
                active.day_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.volume.into_nested_option() {
            active.volume = Set(opt);
            if opt.is_some() {
                active.volume_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.count.into_nested_option() {
            active.count = Set(opt);
            if opt.is_some() {
                active.count_lock = Set(true);
            }
            has_changes = true;
        }
        if let Some(opt) = request.isbns.into_nested_option() {
            active.isbns = Set(opt.clone());
            if opt.is_some() {
                active.isbns_lock = Set(true);
            }
            has_changes = true;
        }

        if has_changes {
            active.updated_at = Set(now);
        }

        active
            .update(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update metadata: {}", e)))?
    } else {
        // Create new record with provided fields and auto-locking
        has_changes = true;
        let summary_opt = request.summary.into_option();
        let writer_opt = request.writer.into_option();
        let penciller_opt = request.penciller.into_option();
        let inker_opt = request.inker.into_option();
        let colorist_opt = request.colorist.into_option();
        let letterer_opt = request.letterer.into_option();
        let cover_artist_opt = request.cover_artist.into_option();
        let editor_opt = request.editor.into_option();
        let publisher_opt = request.publisher.into_option();
        let imprint_opt = request.imprint.into_option();
        let genre_opt = request.genre.into_option();
        let web_opt = request.web.into_option();
        let language_iso_opt = request.language_iso.into_option();
        let format_detail_opt = request.format_detail.into_option();
        let black_and_white_opt = request.black_and_white.into_option();
        let manga_opt = request.manga.into_option();
        let year_opt = request.year.into_option();
        let month_opt = request.month.into_option();
        let day_opt = request.day.into_option();
        let volume_opt = request.volume.into_option();
        let count_opt = request.count.into_option();
        let isbns_opt = request.isbns.into_option();

        let active = book_metadata::ActiveModel {
            id: Set(Uuid::new_v4()),
            book_id: Set(book_id),
            title: Set(None), // Title is not set via metadata replace (use PATCH /books/{id})
            title_sort: Set(None), // Title sort is not set via metadata replace
            number: Set(None), // Number is not set via metadata replace (use PATCH /books/{id})
            summary: Set(summary_opt.clone()),
            writer: Set(writer_opt.clone()),
            penciller: Set(penciller_opt.clone()),
            inker: Set(inker_opt.clone()),
            colorist: Set(colorist_opt.clone()),
            letterer: Set(letterer_opt.clone()),
            cover_artist: Set(cover_artist_opt.clone()),
            editor: Set(editor_opt.clone()),
            publisher: Set(publisher_opt.clone()),
            imprint: Set(imprint_opt.clone()),
            genre: Set(genre_opt.clone()),
            web: Set(web_opt.clone()),
            language_iso: Set(language_iso_opt.clone()),
            format_detail: Set(format_detail_opt.clone()),
            black_and_white: Set(black_and_white_opt),
            manga: Set(manga_opt),
            year: Set(year_opt),
            month: Set(month_opt),
            day: Set(day_opt),
            volume: Set(volume_opt),
            count: Set(count_opt),
            isbns: Set(isbns_opt.clone()),
            // Set locks for non-null fields
            title_lock: Set(false),
            title_sort_lock: Set(false),
            number_lock: Set(false),
            summary_lock: Set(summary_opt.is_some()),
            writer_lock: Set(writer_opt.is_some()),
            penciller_lock: Set(penciller_opt.is_some()),
            inker_lock: Set(inker_opt.is_some()),
            colorist_lock: Set(colorist_opt.is_some()),
            letterer_lock: Set(letterer_opt.is_some()),
            cover_artist_lock: Set(cover_artist_opt.is_some()),
            editor_lock: Set(editor_opt.is_some()),
            publisher_lock: Set(publisher_opt.is_some()),
            imprint_lock: Set(imprint_opt.is_some()),
            genre_lock: Set(genre_opt.is_some()),
            web_lock: Set(web_opt.is_some()),
            language_iso_lock: Set(language_iso_opt.is_some()),
            format_detail_lock: Set(format_detail_opt.is_some()),
            black_and_white_lock: Set(black_and_white_opt.is_some()),
            manga_lock: Set(manga_opt.is_some()),
            year_lock: Set(year_opt.is_some()),
            month_lock: Set(month_opt.is_some()),
            day_lock: Set(day_opt.is_some()),
            volume_lock: Set(volume_opt.is_some()),
            count_lock: Set(count_opt.is_some()),
            isbns_lock: Set(isbns_opt.is_some()),
            created_at: Set(now),
            updated_at: Set(now),
        };

        active
            .insert(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to create metadata: {}", e)))?
    };

    // Emit update event if there were changes
    if has_changes {
        let event = EntityChangeEvent {
            event: EntityEvent::BookUpdated {
                book_id,
                series_id: book.series_id,
                library_id: book.library_id,
                fields: None,
            },
            timestamp: now,
            user_id: Some(auth.user_id),
        };
        let _ = state.event_broadcaster.emit(event);
    }

    Ok(Json(BookMetadataResponse {
        book_id: updated.book_id,
        summary: updated.summary,
        writer: updated.writer,
        penciller: updated.penciller,
        inker: updated.inker,
        colorist: updated.colorist,
        letterer: updated.letterer,
        cover_artist: updated.cover_artist,
        editor: updated.editor,
        publisher: updated.publisher,
        imprint: updated.imprint,
        genre: updated.genre,
        web: updated.web,
        language_iso: updated.language_iso,
        format_detail: updated.format_detail,
        black_and_white: updated.black_and_white,
        manga: updated.manga,
        year: updated.year,
        month: updated.month,
        day: updated.day,
        volume: updated.volume,
        count: updated.count,
        isbns: updated.isbns,
        updated_at: updated.updated_at,
    }))
}

// ============================================================================
// Book Metadata Lock Endpoints
// ============================================================================

use crate::api::routes::v1::dto::{
    BookMetadataLocks, BookUpdateResponse, PatchBookRequest, UpdateBookMetadataLocksRequest,
};

/// Get book metadata lock states
///
/// Returns which metadata fields are locked (protected from automatic updates).
#[utoipa::path(
    get,
    path = "/api/v1/books/{book_id}/metadata/locks",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    responses(
        (status = 200, description = "Lock states retrieved successfully", body = BookMetadataLocks),
        (status = 404, description = "Book or metadata not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn get_book_metadata_locks(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<Json<BookMetadataLocks>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Verify book exists
    BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Get metadata record
    let metadata = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book metadata not found".to_string()))?;

    Ok(Json(BookMetadataLocks {
        summary_lock: metadata.summary_lock,
        writer_lock: metadata.writer_lock,
        penciller_lock: metadata.penciller_lock,
        inker_lock: metadata.inker_lock,
        colorist_lock: metadata.colorist_lock,
        letterer_lock: metadata.letterer_lock,
        cover_artist_lock: metadata.cover_artist_lock,
        editor_lock: metadata.editor_lock,
        publisher_lock: metadata.publisher_lock,
        imprint_lock: metadata.imprint_lock,
        genre_lock: metadata.genre_lock,
        web_lock: metadata.web_lock,
        language_iso_lock: metadata.language_iso_lock,
        format_detail_lock: metadata.format_detail_lock,
        black_and_white_lock: metadata.black_and_white_lock,
        manga_lock: metadata.manga_lock,
        year_lock: metadata.year_lock,
        month_lock: metadata.month_lock,
        day_lock: metadata.day_lock,
        volume_lock: metadata.volume_lock,
        count_lock: metadata.count_lock,
        isbns_lock: metadata.isbns_lock,
    }))
}

/// Update book metadata lock states
///
/// Updates which metadata fields are locked. Only provided fields will be updated.
#[utoipa::path(
    put,
    path = "/api/v1/books/{book_id}/metadata/locks",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body = UpdateBookMetadataLocksRequest,
    responses(
        (status = 200, description = "Lock states updated successfully", body = BookMetadataLocks),
        (status = 404, description = "Book or metadata not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn update_book_metadata_locks(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    Json(request): Json<UpdateBookMetadataLocksRequest>,
) -> Result<Json<BookMetadataLocks>, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Get existing metadata
    let existing = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book metadata not found".to_string()))?;

    // Update locks
    let now = Utc::now();
    let mut active: book_metadata::ActiveModel = existing.into();

    if let Some(v) = request.summary_lock {
        active.summary_lock = Set(v);
    }
    if let Some(v) = request.writer_lock {
        active.writer_lock = Set(v);
    }
    if let Some(v) = request.penciller_lock {
        active.penciller_lock = Set(v);
    }
    if let Some(v) = request.inker_lock {
        active.inker_lock = Set(v);
    }
    if let Some(v) = request.colorist_lock {
        active.colorist_lock = Set(v);
    }
    if let Some(v) = request.letterer_lock {
        active.letterer_lock = Set(v);
    }
    if let Some(v) = request.cover_artist_lock {
        active.cover_artist_lock = Set(v);
    }
    if let Some(v) = request.editor_lock {
        active.editor_lock = Set(v);
    }
    if let Some(v) = request.publisher_lock {
        active.publisher_lock = Set(v);
    }
    if let Some(v) = request.imprint_lock {
        active.imprint_lock = Set(v);
    }
    if let Some(v) = request.genre_lock {
        active.genre_lock = Set(v);
    }
    if let Some(v) = request.web_lock {
        active.web_lock = Set(v);
    }
    if let Some(v) = request.language_iso_lock {
        active.language_iso_lock = Set(v);
    }
    if let Some(v) = request.format_detail_lock {
        active.format_detail_lock = Set(v);
    }
    if let Some(v) = request.black_and_white_lock {
        active.black_and_white_lock = Set(v);
    }
    if let Some(v) = request.manga_lock {
        active.manga_lock = Set(v);
    }
    if let Some(v) = request.year_lock {
        active.year_lock = Set(v);
    }
    if let Some(v) = request.month_lock {
        active.month_lock = Set(v);
    }
    if let Some(v) = request.day_lock {
        active.day_lock = Set(v);
    }
    if let Some(v) = request.volume_lock {
        active.volume_lock = Set(v);
    }
    if let Some(v) = request.count_lock {
        active.count_lock = Set(v);
    }
    if let Some(v) = request.isbns_lock {
        active.isbns_lock = Set(v);
    }

    active.updated_at = Set(now);

    let updated = active
        .update(&state.db)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update locks: {}", e)))?;

    // Emit update event
    let event = EntityChangeEvent {
        event: EntityEvent::BookUpdated {
            book_id,
            series_id: book.series_id,
            library_id: book.library_id,
            fields: Some(vec!["metadata_locks".to_string()]),
        },
        timestamp: now,
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(Json(BookMetadataLocks {
        summary_lock: updated.summary_lock,
        writer_lock: updated.writer_lock,
        penciller_lock: updated.penciller_lock,
        inker_lock: updated.inker_lock,
        colorist_lock: updated.colorist_lock,
        letterer_lock: updated.letterer_lock,
        cover_artist_lock: updated.cover_artist_lock,
        editor_lock: updated.editor_lock,
        publisher_lock: updated.publisher_lock,
        imprint_lock: updated.imprint_lock,
        genre_lock: updated.genre_lock,
        web_lock: updated.web_lock,
        language_iso_lock: updated.language_iso_lock,
        format_detail_lock: updated.format_detail_lock,
        black_and_white_lock: updated.black_and_white_lock,
        manga_lock: updated.manga_lock,
        year_lock: updated.year_lock,
        month_lock: updated.month_lock,
        day_lock: updated.day_lock,
        volume_lock: updated.volume_lock,
        count_lock: updated.count_lock,
        isbns_lock: updated.isbns_lock,
    }))
}

// ============================================================================
// Book Cover Upload Endpoint
// ============================================================================

use crate::events::EntityType;
use axum::extract::Multipart;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Upload a custom cover image for a book
///
/// Accepts a multipart form with an image file. The image will be stored
/// in the uploads directory and used as the book's cover/thumbnail.
#[utoipa::path(
    post,
    path = "/api/v1/books/{book_id}/cover",
    params(
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    request_body(content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Cover uploaded successfully"),
        (status = 400, description = "Bad request - no image file provided or invalid image"),
        (status = 404, description = "Book not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "books"
)]
pub async fn upload_book_cover(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(book_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::BooksWrite)?;

    // Verify book exists and get its library_id/series_id
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

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
    let covers_dir = state
        .thumbnail_service
        .get_uploads_dir()
        .join("covers")
        .join("books");
    fs::create_dir_all(&covers_dir)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create covers directory: {}", e)))?;

    // Save the image with a unique filename
    let filename = format!("{}.jpg", book_id);
    let filepath = covers_dir.join(&filename);

    let mut file = fs::File::create(&filepath)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create cover file: {}", e)))?;

    file.write_all(&image_data)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to write cover file: {}", e)))?;

    // Emit cover updated event
    let event = EntityChangeEvent {
        event: EntityEvent::CoverUpdated {
            entity_type: EntityType::Book,
            entity_id: book_id,
            library_id: Some(book.library_id),
        },
        timestamp: Utc::now(),
        user_id: Some(auth.user_id),
    };
    let _ = state.event_broadcaster.emit(event);

    Ok(StatusCode::OK)
}
