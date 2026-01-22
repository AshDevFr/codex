//! Komga-compatible series handlers
//!
//! Handlers for series-related endpoints in the Komga-compatible API.

use super::super::dto::book::KomgaBookDto;
use super::super::dto::pagination::KomgaPage;
use super::super::dto::series::{
    codex_to_komga_reading_direction, codex_to_komga_status, KomgaBooksMetadataAggregationDto,
    KomgaSeriesDto, KomgaSeriesMetadataDto,
};
use super::libraries::{extract_page_image, generate_thumbnail};
use crate::api::{
    error::ApiError,
    extractors::{AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::repositories::{
    BookRepository, ReadProgressRepository, SeriesCoversRepository, SeriesMetadataRepository,
    SeriesRepository,
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
use tokio::fs;
use uuid::Uuid;

/// Query parameters for paginated series endpoints
#[derive(Debug, Deserialize)]
pub struct SeriesPaginationQuery {
    /// Page number (0-indexed, Komga-style)
    #[serde(default)]
    pub page: i32,
    /// Page size (default: 20)
    #[serde(default = "default_page_size")]
    pub size: i32,
    /// Filter by library ID
    pub library_id: Option<Uuid>,
    /// Search query
    pub search: Option<String>,
}

fn default_page_size() -> i32 {
    20
}

/// List all series (paginated)
///
/// Returns all series in Komga-compatible format with pagination.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/series`
///
/// ## Query Parameters
/// - `page` - Page number (0-indexed, default: 0)
/// - `size` - Page size (default: 20)
/// - `library_id` - Optional filter by library UUID
/// - `search` - Optional search query
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
pub async fn list_series(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<SeriesPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaSeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let user_id = Some(auth.user_id);
    let page = query.page.max(0) as u64;
    let size = query.size.max(1).min(500) as u64;
    let offset = page * size;

    // Get series based on filters
    let series_list = if let Some(library_id) = query.library_id {
        SeriesRepository::list_by_library(&state.db, library_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
    } else if let Some(search) = &query.search {
        SeriesRepository::search_by_name(&state.db, search)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to search series: {}", e)))?
    } else {
        SeriesRepository::list_all(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
    };

    let total = series_list.len() as i64;

    // Apply pagination
    let paginated: Vec<_> = series_list
        .into_iter()
        .skip(offset as usize)
        .take(size as usize)
        .collect();

    // Convert to DTOs
    let mut dtos = Vec::with_capacity(paginated.len());
    for series in paginated {
        let dto = build_series_dto(&state, &series, user_id).await?;
        dtos.push(dto);
    }

    Ok(Json(KomgaPage::new(dtos, query.page, query.size, total)))
}

/// Get recently added series
///
/// Returns series sorted by created date descending (newest first).
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/series/new`
///
/// ## Query Parameters
/// - `page` - Page number (0-indexed, default: 0)
/// - `size` - Page size (default: 20)
/// - `library_id` - Optional filter by library UUID
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
pub async fn get_series_new(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<SeriesPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaSeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let user_id = Some(auth.user_id);
    let page = query.page.max(0) as u64;
    let size = query.size.max(1).min(500) as u64;

    // Get recently added series
    // For now, we fetch a larger batch to enable pagination
    // A more efficient approach would be to add count + offset to the repository
    let series_list = SeriesRepository::list_recently_added(
        &state.db,
        query.library_id,
        (page + 1) * size, // Fetch enough for this page
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    // Get total count for pagination info
    let total = if let Some(library_id) = query.library_id {
        SeriesRepository::count_by_library(&state.db, library_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to count series: {}", e)))?
    } else {
        SeriesRepository::list_all(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to count series: {}", e)))?
            .len() as i64
    };

    // Apply pagination offset
    let offset = page * size;
    let paginated: Vec<_> = series_list
        .into_iter()
        .skip(offset as usize)
        .take(size as usize)
        .collect();

    // Convert to DTOs
    let mut dtos = Vec::with_capacity(paginated.len());
    for series in paginated {
        let dto = build_series_dto(&state, &series, user_id).await?;
        dtos.push(dto);
    }

    Ok(Json(KomgaPage::new(dtos, query.page, query.size, total)))
}

/// Get recently updated series
///
/// Returns series sorted by last modified date descending (most recently updated first).
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/series/updated`
///
/// ## Query Parameters
/// - `page` - Page number (0-indexed, default: 0)
/// - `size` - Page size (default: 20)
/// - `library_id` - Optional filter by library UUID
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
pub async fn get_series_updated(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Query(query): Query<SeriesPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaSeriesDto>>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let user_id = Some(auth.user_id);
    let page = query.page.max(0) as u64;
    let size = query.size.max(1).min(500) as u64;

    // Get recently updated series
    let series_list = SeriesRepository::list_recently_updated(
        &state.db,
        query.library_id,
        (page + 1) * size, // Fetch enough for this page
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    // Get total count for pagination info
    let total = if let Some(library_id) = query.library_id {
        SeriesRepository::count_by_library(&state.db, library_id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to count series: {}", e)))?
    } else {
        SeriesRepository::list_all(&state.db)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to count series: {}", e)))?
            .len() as i64
    };

    // Apply pagination offset
    let offset = page * size;
    let paginated: Vec<_> = series_list
        .into_iter()
        .skip(offset as usize)
        .take(size as usize)
        .collect();

    // Convert to DTOs
    let mut dtos = Vec::with_capacity(paginated.len());
    for series in paginated {
        let dto = build_series_dto(&state, &series, user_id).await?;
        dtos.push(dto);
    }

    Ok(Json(KomgaPage::new(dtos, query.page, query.size, total)))
}

/// Get series by ID
///
/// Returns a single series in Komga-compatible format.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/series/{seriesId}`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
pub async fn get_series(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Json<KomgaSeriesDto>, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    let user_id = Some(auth.user_id);

    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    let dto = build_series_dto(&state, &series, user_id).await?;
    Ok(Json(dto))
}

/// Get series thumbnail
///
/// Returns a thumbnail image for the series.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/series/{seriesId}/thumbnail`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
pub async fn get_series_thumbnail(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::SeriesRead)?;

    // Verify series exists
    let _series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get the series cover - try selected cover first
    let image_data = if let Some(cover) = SeriesCoversRepository::get_selected(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch cover: {}", e)))?
    {
        fs::read(&cover.path).await.map_err(|e| {
            ApiError::Internal(format!("Failed to read cover from {}: {}", cover.path, e))
        })?
    } else {
        // Fall back to first book's first page
        get_default_series_cover(&state, series_id).await?
    };

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

/// Get books in a series
///
/// Returns all books in a series with pagination.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/series/{seriesId}/books`
///
/// ## Query Parameters
/// - `page` - Page number (0-indexed, default: 0)
/// - `size` - Page size (default: 20)
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
pub async fn get_series_books(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(series_id): Path<Uuid>,
    Query(query): Query<SeriesPaginationQuery>,
) -> Result<Json<KomgaPage<KomgaBookDto>>, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let user_id = Some(auth.user_id);
    let page = query.page.max(0) as u64;
    let size = query.size.max(1).min(500) as u64;
    let offset = page * size;

    // Verify series exists and get its data
    let series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get series metadata for the series title
    let metadata = SeriesMetadataRepository::get_by_series_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series metadata: {}", e)))?;

    let series_title = metadata
        .as_ref()
        .map(|m| m.title.clone())
        .unwrap_or_else(|| series.name.clone());

    // Get books in series (excluding deleted)
    let books = BookRepository::list_by_series(&state.db, series_id, false)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    let total = books.len() as i64;

    // Apply pagination
    let paginated: Vec<_> = books
        .into_iter()
        .skip(offset as usize)
        .take(size as usize)
        .collect();

    // Convert to DTOs
    let mut dtos = Vec::with_capacity(paginated.len());
    for (idx, book) in paginated.into_iter().enumerate() {
        // Get read progress for this book and user
        let read_progress = if let Some(uid) = user_id {
            ReadProgressRepository::get_by_user_and_book(&state.db, uid, book.id)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        let dto = KomgaBookDto::from_codex(
            &book,
            &series_title,
            (offset as i32) + (idx as i32) + 1, // 1-indexed number
            read_progress.as_ref(),
        );
        dtos.push(dto);
    }

    Ok(Json(KomgaPage::new(dtos, query.page, query.size, total)))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Build a KomgaSeriesDto from a series entity
async fn build_series_dto(
    state: &Arc<AuthState>,
    series: &crate::db::entities::series::Model,
    user_id: Option<Uuid>,
) -> Result<KomgaSeriesDto, ApiError> {
    // Get metadata
    let metadata = SeriesMetadataRepository::get_by_series_id(&state.db, series.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series metadata: {}", e)))?;

    // Get book counts
    let book_count = SeriesRepository::get_book_count(&state.db, series.id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get book count: {}", e)))?
        as i32;

    // Get reading stats for user
    let (books_read_count, books_in_progress_count) = if let Some(uid) = user_id {
        get_series_read_stats(&state.db, series.id, uid).await?
    } else {
        (0, 0)
    };

    let books_unread_count = (book_count - books_read_count - books_in_progress_count).max(0);

    // Build metadata DTO
    let now = chrono::Utc::now().to_rfc3339();
    let series_metadata = if let Some(ref m) = metadata {
        KomgaSeriesMetadataDto {
            status: codex_to_komga_status(m.status.as_deref()),
            status_lock: m.status_lock,
            title: m.title.clone(),
            title_lock: m.title_lock,
            title_sort: m.title_sort.clone().unwrap_or_else(|| m.title.clone()),
            title_sort_lock: m.title_sort_lock,
            summary: m.summary.clone().unwrap_or_default(),
            summary_lock: m.summary_lock,
            reading_direction: codex_to_komga_reading_direction(m.reading_direction.as_deref()),
            reading_direction_lock: m.reading_direction_lock,
            publisher: m.publisher.clone().unwrap_or_default(),
            publisher_lock: m.publisher_lock,
            age_rating: m.age_rating,
            age_rating_lock: m.age_rating_lock,
            language: m.language.clone().unwrap_or_default(),
            language_lock: m.language_lock,
            genres: Vec::new(), // TODO: Add genres from series_genres
            genres_lock: m.genres_lock,
            tags: Vec::new(), // TODO: Add tags from series_tags
            tags_lock: m.tags_lock,
            total_book_count: m.total_book_count,
            total_book_count_lock: m.total_book_count_lock,
            sharing_labels: Vec::new(),
            sharing_labels_lock: false,
            links: Vec::new(),
            links_lock: false,
            alternate_titles: Vec::new(),
            alternate_titles_lock: false,
            created: m.created_at.to_rfc3339(),
            last_modified: m.updated_at.to_rfc3339(),
        }
    } else {
        KomgaSeriesMetadataDto {
            status: "ONGOING".to_string(),
            title: series.name.clone(),
            title_sort: series.name.clone(),
            created: now.clone(),
            last_modified: now.clone(),
            ..Default::default()
        }
    };

    // Build aggregation DTO (simplified)
    let books_metadata = KomgaBooksMetadataAggregationDto {
        authors: Vec::new(), // TODO: Aggregate from book metadata
        tags: Vec::new(),
        release_date: None,
        summary: series_metadata.summary.clone(),
        summary_number: String::new(),
        created: series.created_at.to_rfc3339(),
        last_modified: series.updated_at.to_rfc3339(),
    };

    Ok(KomgaSeriesDto {
        id: series.id.to_string(),
        library_id: series.library_id.to_string(),
        name: metadata
            .as_ref()
            .map(|m| m.title.clone())
            .unwrap_or_else(|| series.name.clone()),
        url: series.path.clone(),
        created: series.created_at.to_rfc3339(),
        last_modified: series.updated_at.to_rfc3339(),
        file_last_modified: series.updated_at.to_rfc3339(), // Use updated_at as proxy
        books_count: book_count,
        books_read_count,
        books_unread_count,
        books_in_progress_count,
        metadata: series_metadata,
        books_metadata,
        deleted: false,
        oneshot: book_count == 1,
    })
}

/// Get series reading stats for a user
async fn get_series_read_stats(
    db: &sea_orm::DatabaseConnection,
    series_id: Uuid,
    user_id: Uuid,
) -> Result<(i32, i32), ApiError> {
    // Get all books in series
    let books = BookRepository::list_by_series(db, series_id, false)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    let mut read_count = 0;
    let mut in_progress_count = 0;

    for book in books {
        if let Ok(Some(progress)) =
            ReadProgressRepository::get_by_user_and_book(db, user_id, book.id).await
        {
            if progress.completed {
                read_count += 1;
            } else if progress.current_page > 0 {
                in_progress_count += 1;
            }
        }
    }

    Ok((read_count, in_progress_count))
}

/// Get default series cover from first book's first page
async fn get_default_series_cover(
    state: &Arc<AuthState>,
    series_id: Uuid,
) -> Result<Vec<u8>, ApiError> {
    // Get the first book in the series
    let books = BookRepository::list_by_series(&state.db, series_id, false)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    let first_book = books
        .first()
        .ok_or_else(|| ApiError::NotFound("Series has no books".to_string()))?;

    // Check if book has pages
    if first_book.page_count == 0 {
        return Err(ApiError::NotFound("First book has no pages".to_string()));
    }

    // Extract first page from the book
    extract_page_image(&first_book.file_path, &first_book.format, 1)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to extract cover image: {}", e)))
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
        let query: SeriesPaginationQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(query.page, 0);
        assert_eq!(query.size, 20);
        assert!(query.library_id.is_none());
        assert!(query.search.is_none());
    }
}
