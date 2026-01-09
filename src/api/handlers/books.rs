use crate::api::{
    dto::{BookDetailResponse, BookDto, BookListResponse, BookMetadataDto, PaginationParams},
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::repositories::{BookMetadataRepository, BookRepository};
use crate::require_permission;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
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
}

fn default_page_size() -> u64 {
    20
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
        let end = (start + page_size as usize).min(books.len());
        let paginated = books[start..end].to_vec();

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

    let dtos: Vec<BookDto> = books_list
        .into_iter()
        .map(|book| BookDto {
            id: book.id,
            series_id: book.series_id,
            title: book.title.clone().unwrap_or_default(),
            sort_title: book.title.clone(), // No sort_title field, use title (Option<String>)
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
        })
        .collect();

    let response = BookListResponse::new(dtos, query.page, page_size, total);

    Ok(Json(response))
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

    let book_dto = BookDto {
        id: book.id,
        series_id: book.series_id,
        title: book.title.clone().unwrap_or_default(),
        sort_title: book.title.clone(), // No sort_title field, use title (Option<String>)
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
    };

    let response = BookDetailResponse {
        book: book_dto,
        metadata,
    };

    Ok(Json(response))
}
