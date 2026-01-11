use crate::api::{
    dto::{BookDetailResponse, BookDto, BookListResponse, BookMetadataDto, PaginationParams},
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::repositories::{
    BookMetadataRepository, BookRepository, ReadProgressRepository, SeriesRepository,
};
use crate::require_permission;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Query parameters for listing books
#[derive(Debug, Deserialize)]
pub struct BookListQuery {
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
    path = "/api/v1/series/{id}/books/with-errors",
    params(
        ("id" = Uuid, Path, description = "Series ID"),
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
    path = "/api/v1/books/{id}",
    params(
        ("id" = Uuid, Path, description = "Book ID")
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
    Path(id): Path<Uuid>,
) -> Result<Json<BookDetailResponse>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let book = BookRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Try to fetch metadata
    let metadata = BookMetadataRepository::get_by_book_id(&state.db, id)
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
        None,        // all libraries
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
        None, // all libraries
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
        &state.db, None,  // all libraries
        false, // exclude deleted
        query.page, page_size,
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
}

fn default_recent_limit() -> u64 {
    50
}

/// List recently read books (ordered by last read activity)
#[utoipa::path(
    get,
    path = "/api/v1/books/recently-read",
    params(
        ("limit" = Option<u64>, Query, description = "Maximum number of books to return (default: 50)")
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

    let books_list = BookRepository::list_recently_read(&state.db, auth.user_id, None, query.limit)
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
