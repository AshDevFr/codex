use crate::api::{
    dto::{
        AdjacentBooksResponse, BookDetailResponse, BookDto, BookListRequest, BookListResponse,
        BookMetadataDto, PaginationParams,
    },
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::repositories::{
    BookMetadataRepository, BookRepository, ReadProgressRepository, SeriesRepository,
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
    // Collect unique series IDs
    let series_ids: Vec<Uuid> = books
        .iter()
        .map(|b| b.series_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Fetch all series in one query
    let mut series_map = HashMap::new();
    for series_id in series_ids {
        if let Ok(Some(series)) = SeriesRepository::get_by_id(db, series_id).await {
            series_map.insert(series_id, series.name);
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
            let series_name = series_map
                .get(&book.series_id)
                .cloned()
                .unwrap_or_else(|| "Unknown Series".to_string());

            // Use title if available, otherwise use file_name
            let title = book.title.clone().unwrap_or_else(|| {
                // Extract filename without extension
                let file_name = &book.file_name;
                if let Some(pos) = file_name.rfind('.') {
                    file_name[..pos].to_string()
                } else {
                    file_name.clone()
                }
            });

            let read_progress = progress_map.get(&book.id).cloned();

            BookDto {
                id: book.id,
                series_id: book.series_id,
                series_name,
                title,
                sort_title: book.title.clone(),
                file_path: book.file_path,
                file_format: book.format,
                file_size: book.file_size,
                file_hash: book.file_hash,
                page_count: book.page_count,
                number: book
                    .number
                    .map(|d| d.to_string().parse::<i32>().unwrap_or(0)),
                created_at: book.created_at,
                updated_at: book.updated_at,
                read_progress,
                analysis_error: book.analysis_error,
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

    // Fetch books based on filter
    let (books_list, total) = if let Some(ser_id) = query.series_id {
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
        // List all books with pagination
        BookRepository::list_all(
            &state.db, false, // exclude deleted
            query.page, page_size,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?
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

    // If there's a condition, evaluate it to get matching book IDs
    let filtered_ids: Option<HashSet<Uuid>> = if let Some(ref condition) = request.condition {
        let matching = FilterService::get_matching_books(&state.db, condition, None)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to evaluate filter: {}", e)))?;
        Some(matching)
    } else {
        None
    };

    // Fetch books based on filter results
    let (books_list, total) = if let Some(ref ids) = filtered_ids {
        if ids.is_empty() {
            // No matches, return empty response
            (vec![], 0)
        } else {
            // Fetch books by IDs with pagination
            let id_vec: Vec<Uuid> = ids.iter().cloned().collect();
            BookRepository::list_by_ids(&state.db, &id_vec, request.page, page_size)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?
        }
    } else {
        // No filter, fetch all books
        BookRepository::list_all(&state.db, false, request.page, page_size)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?
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

/// Apply sorting to books list
fn apply_book_sorting(books_list: &mut [crate::db::entities::books::Model], sort_param: &str) {
    let parts: Vec<&str> = sort_param.split(',').collect();
    if parts.len() != 2 {
        return; // Invalid format, skip sorting
    }

    let field = parts[0];
    let direction = parts[1];
    let ascending = direction == "asc";

    match field {
        "title" => {
            books_list.sort_by(|a, b| {
                let a_title = a.title.as_deref().unwrap_or(&a.file_name);
                let b_title = b.title.as_deref().unwrap_or(&b.file_name);
                let cmp = a_title.cmp(b_title);
                if ascending {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        }
        "created_at" => {
            books_list.sort_by(|a, b| {
                let cmp = a.created_at.cmp(&b.created_at);
                if ascending {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        }
        "release_date" => {
            books_list.sort_by(|a, b| {
                // Handle None values - put them at the end
                match (&a.number, &b.number) {
                    (Some(a_num), Some(b_num)) => {
                        let cmp = a_num
                            .partial_cmp(b_num)
                            .unwrap_or(std::cmp::Ordering::Equal);
                        if ascending {
                            cmp
                        } else {
                            cmp.reverse()
                        }
                    }
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            });
        }
        "chapter_number" => {
            books_list.sort_by(|a, b| match (&a.number, &b.number) {
                (Some(a_num), Some(b_num)) => {
                    let cmp = a_num
                        .partial_cmp(b_num)
                        .unwrap_or(std::cmp::Ordering::Equal);
                    if ascending {
                        cmp
                    } else {
                        cmp.reverse()
                    }
                }
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            });
        }
        _ => {} // Unknown field, skip sorting
    }
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

    // Try to fetch metadata
    let metadata = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .ok()
        .flatten()
        .map(|meta| BookMetadataDto {
            id: meta.id,
            book_id: meta.book_id,
            title: None,  // No title field in metadata
            series: None, // No series field in metadata
            number: None, // No number field in metadata
            summary: meta.summary,
            publisher: meta.publisher,
            imprint: meta.imprint,
            genre: meta.genre,
            page_count: None, // No page_count field in metadata
            language_iso: meta.language_iso,
            release_date: None, // No release_date field in metadata
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

    // Fetch books by library
    let (mut books_list, total) = BookRepository::list_by_library(
        &state.db, library_id, false, // exclude deleted
        query.page, page_size,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch library books: {}", e)))?;

    // Apply sorting if specified (Note: pagination already applied by repository, so this only sorts the current page)
    if let Some(sort_param) = &query.sort {
        apply_book_sorting(&mut books_list, sort_param);
    }

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

    let books_list = BookRepository::list_recently_read(&state.db, auth.user_id, query.library_id, query.limit)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch recently read books: {}", e)))?;

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
    auth: AuthContext,
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

use crate::api::dto::{BookMetadataResponse, PatchBookMetadataRequest, ReplaceBookMetadataRequest};
use crate::db::entities::book_metadata_records;
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
        let mut active: book_metadata_records::ActiveModel = existing.into();

        active.summary = Set(request.summary);
        active.writer = Set(request.writer);
        active.penciller = Set(request.penciller);
        active.inker = Set(request.inker);
        active.colorist = Set(request.colorist);
        active.letterer = Set(request.letterer);
        active.cover_artist = Set(request.cover_artist);
        active.editor = Set(request.editor);
        active.publisher = Set(request.publisher);
        active.imprint = Set(request.imprint);
        active.genre = Set(request.genre);
        active.web = Set(request.web);
        active.language_iso = Set(request.language_iso);
        active.format_detail = Set(request.format_detail);
        active.black_and_white = Set(request.black_and_white);
        active.manga = Set(request.manga);
        active.year = Set(request.year);
        active.month = Set(request.month);
        active.day = Set(request.day);
        active.volume = Set(request.volume);
        active.count = Set(request.count);
        active.isbns = Set(request.isbns);
        active.updated_at = Set(now);

        active
            .update(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to update metadata: {}", e)))?
    } else {
        // Create new record
        let active = book_metadata_records::ActiveModel {
            id: Set(Uuid::new_v4()),
            book_id: Set(book_id),
            summary: Set(request.summary),
            writer: Set(request.writer),
            penciller: Set(request.penciller),
            inker: Set(request.inker),
            colorist: Set(request.colorist),
            letterer: Set(request.letterer),
            cover_artist: Set(request.cover_artist),
            editor: Set(request.editor),
            publisher: Set(request.publisher),
            imprint: Set(request.imprint),
            genre: Set(request.genre),
            web: Set(request.web),
            language_iso: Set(request.language_iso),
            format_detail: Set(request.format_detail),
            black_and_white: Set(request.black_and_white),
            manga: Set(request.manga),
            year: Set(request.year),
            month: Set(request.month),
            day: Set(request.day),
            volume: Set(request.volume),
            count: Set(request.count),
            isbns: Set(request.isbns),
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
        // Partial update existing record
        let mut active: book_metadata_records::ActiveModel = existing.into();

        if let Some(opt) = request.summary.to_active_value() {
            active.summary = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.writer.to_active_value() {
            active.writer = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.penciller.to_active_value() {
            active.penciller = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.inker.to_active_value() {
            active.inker = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.colorist.to_active_value() {
            active.colorist = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.letterer.to_active_value() {
            active.letterer = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.cover_artist.to_active_value() {
            active.cover_artist = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.editor.to_active_value() {
            active.editor = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.publisher.to_active_value() {
            active.publisher = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.imprint.to_active_value() {
            active.imprint = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.genre.to_active_value() {
            active.genre = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.web.to_active_value() {
            active.web = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.language_iso.to_active_value() {
            active.language_iso = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.format_detail.to_active_value() {
            active.format_detail = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.black_and_white.to_active_value() {
            active.black_and_white = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.manga.to_active_value() {
            active.manga = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.year.to_active_value() {
            active.year = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.month.to_active_value() {
            active.month = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.day.to_active_value() {
            active.day = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.volume.to_active_value() {
            active.volume = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.count.to_active_value() {
            active.count = Set(opt);
            has_changes = true;
        }
        if let Some(opt) = request.isbns.to_active_value() {
            active.isbns = Set(opt);
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
        // Create new record with provided fields
        has_changes = true;
        let active = book_metadata_records::ActiveModel {
            id: Set(Uuid::new_v4()),
            book_id: Set(book_id),
            summary: Set(request.summary.into_option()),
            writer: Set(request.writer.into_option()),
            penciller: Set(request.penciller.into_option()),
            inker: Set(request.inker.into_option()),
            colorist: Set(request.colorist.into_option()),
            letterer: Set(request.letterer.into_option()),
            cover_artist: Set(request.cover_artist.into_option()),
            editor: Set(request.editor.into_option()),
            publisher: Set(request.publisher.into_option()),
            imprint: Set(request.imprint.into_option()),
            genre: Set(request.genre.into_option()),
            web: Set(request.web.into_option()),
            language_iso: Set(request.language_iso.into_option()),
            format_detail: Set(request.format_detail.into_option()),
            black_and_white: Set(request.black_and_white.into_option()),
            manga: Set(request.manga.into_option()),
            year: Set(request.year.into_option()),
            month: Set(request.month.into_option()),
            day: Set(request.day.into_option()),
            volume: Set(request.volume.into_option()),
            count: Set(request.count.into_option()),
            isbns: Set(request.isbns.into_option()),
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
