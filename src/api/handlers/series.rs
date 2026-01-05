use crate::api::{
    dto::{BookDto, SearchSeriesRequest, SeriesDto, SeriesFilter},
    error::ApiError,
    extractors::{AuthContext, AuthState},
    permissions::Permission,
};
use crate::db::repositories::{BookRepository, SeriesRepository};
use crate::require_permission;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// Query parameters for listing books in a series
#[derive(Debug, Deserialize)]
pub struct ListBooksQuery {
    /// Include deleted books in the result
    #[serde(default)]
    pub include_deleted: bool,
}

/// List series with optional library filter
#[utoipa::path(
    get,
    path = "/api/v1/series",
    params(
        ("library_id" = Option<Uuid>, Query, description = "Filter by library ID")
    ),
    responses(
        (status = 200, description = "List of series", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn list_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Query(filter): Query<SeriesFilter>,
) -> Result<Json<Vec<SeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Fetch series based on filter
    let series_list = if let Some(lib_id) = filter.library_id {
        SeriesRepository::list_by_library(&state.db, lib_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
    } else {
        SeriesRepository::list_all(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
    };

    let dtos: Vec<SeriesDto> = series_list
        .into_iter()
        .map(|series| SeriesDto {
            id: series.id,
            library_id: series.library_id,
            name: series.name,
            sort_name: series.sort_name,
            description: series.summary, // Use summary instead of description
            publisher: series.publisher,
            year: series.year,
            book_count: series.book_count as i64, // Convert i32 to i64
            created_at: series.created_at,
            updated_at: series.updated_at,
        })
        .collect();

    Ok(Json(dtos))
}

/// Get series by ID
#[utoipa::path(
    get,
    path = "/api/v1/series/{id}",
    params(
        ("id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "Series details", body = SeriesDto),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn get_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<SeriesDto>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series = SeriesRepository::get_by_id(&state.db, id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let dto = SeriesDto {
        id: series.id,
        library_id: series.library_id,
        name: series.name,
        sort_name: series.sort_name,
        description: series.summary, // Use summary instead of description
        publisher: series.publisher,
        year: series.year,
        book_count: series.book_count as i64, // Convert i32 to i64
        created_at: series.created_at,
        updated_at: series.updated_at,
    };

    Ok(Json(dto))
}

/// Search series by name
#[utoipa::path(
    post,
    path = "/api/v1/series/search",
    request_body = SearchSeriesRequest,
    responses(
        (status = 200, description = "Search results", body = Vec<SeriesDto>),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn search_series(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Json(request): Json<SearchSeriesRequest>,
) -> Result<Json<Vec<SeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let series_list = SeriesRepository::search_by_name(&state.db, &request.query)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to search series: {}", e)))?;

    // Filter by library if specified
    let filtered: Vec<_> = if let Some(lib_id) = request.library_id {
        series_list
            .into_iter()
            .filter(|s| s.library_id == lib_id)
            .collect()
    } else {
        series_list
    };

    let dtos: Vec<SeriesDto> = filtered
        .into_iter()
        .map(|series| SeriesDto {
            id: series.id,
            library_id: series.library_id,
            name: series.name,
            sort_name: series.sort_name,
            description: series.summary, // Use summary instead of description
            publisher: series.publisher,
            year: series.year,
            book_count: series.book_count as i64, // Convert i32 to i64
            created_at: series.created_at,
            updated_at: series.updated_at,
        })
        .collect();

    Ok(Json(dtos))
}

/// Get books in a series
#[utoipa::path(
    get,
    path = "/api/v1/series/{id}/books",
    params(
        ("id" = Uuid, Path, description = "Series ID"),
        ("include_deleted" = Option<bool>, Query, description = "Include deleted books (default: false)")
    ),
    responses(
        (status = 200, description = "List of books in the series", body = Vec<BookDto>),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Series not found"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn get_series_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
    Query(query): Query<ListBooksQuery>,
) -> Result<Json<Vec<BookDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Fetch books
    let books = BookRepository::list_by_series(&state.db, series_id, query.include_deleted)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    // Convert to DTOs
    let dtos: Vec<BookDto> = books
        .into_iter()
        .map(|book| {
            let title = book.title.clone().unwrap_or_else(|| book.file_name.clone());
            BookDto {
                id: book.id,
                series_id: book.series_id,
                title: title.clone(),
                sort_title: book.title,
                file_path: book.file_path,
                file_format: book.format,
                file_size: book.file_size,
                file_hash: book.file_hash,
                page_count: book.page_count,
                number: book
                    .number
                    .map(|n| n.to_string().parse::<i32>().unwrap_or(0)),
                created_at: book.created_at,
                updated_at: book.updated_at,
            }
        })
        .collect();

    Ok(Json(dtos))
}

/// Purge deleted books from a series
#[utoipa::path(
    delete,
    path = "/api/v1/series/{id}/purge-deleted",
    params(
        ("id" = Uuid, Path, description = "Series ID")
    ),
    responses(
        (status = 200, description = "Number of books purged", body = u64),
        (status = 404, description = "Series not found"),
        (status = 403, description = "Forbidden"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "series"
)]
pub async fn purge_series_deleted_books(
    State(state): State<Arc<AuthState>>,
    auth: AuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<u64>, ApiError> {
    require_permission!(auth, Permission::SeriesWrite)?;

    // Verify series exists
    SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Purge deleted books
    let count = BookRepository::purge_deleted_in_series(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to purge deleted books: {}", e)))?;

    Ok(Json(count))
}
