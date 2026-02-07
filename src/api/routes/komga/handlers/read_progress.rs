//! Komga-compatible read progress handlers
//!
//! Handlers for read progress endpoints in the Komga-compatible API.
//! These endpoints allow Komic and other Komga-compatible apps to track
//! and sync reading progress.

use super::super::dto::book::KomgaReadProgressUpdateDto;
use crate::api::{
    error::ApiError,
    extractors::{AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::repositories::{BookRepository, ReadProgressRepository, SeriesRepository};
use crate::require_permission;
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use std::sync::Arc;
use uuid::Uuid;

/// Update reading progress for a book
///
/// Updates the user's reading progress for a specific book.
/// Komic sends: `{ "completed": false, "page": 151 }`
///
/// ## Endpoint
/// `PATCH /{prefix}/api/v1/books/{bookId}/read-progress`
///
/// ## Request Body
/// - `page` - Current page number (1-indexed, optional)
/// - `completed` - Whether book is completed (optional)
/// - `device_id` - Device ID (optional, not used by Komic)
/// - `device_name` - Device name (optional, not used by Komic)
///
/// ## Response
/// - 204 No Content on success (Komga behavior)
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    patch,
    path = "/{prefix}/api/v1/books/{book_id}/read-progress",
    request_body = KomgaReadProgressUpdateDto,
    responses(
        (status = 204, description = "Progress updated successfully"),
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
pub async fn update_progress(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(book_id): Path<Uuid>,
    axum::Json(request): axum::Json<KomgaReadProgressUpdateDto>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let user_id = auth.user_id;

    // Verify book exists
    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Determine completed status and current page together.
    // When completed is explicitly true and no page is provided, set current_page
    // to the book's last page (page_count). This handles Komic sending just
    // { "completed": true } to mark a book as read.
    // When completed is not explicitly true, auto-detect: mark as completed when
    // current_page reaches page_count. This handles readers that send
    // { "completed": false, "page": 178 } on a 178-page book.
    let current_page = request.page.unwrap_or(1).max(1);
    let (current_page, completed) = match request.completed {
        Some(true) => {
            // Explicitly marked as completed — if no page was sent,
            // snap to the last page so the progress looks correct.
            let page = if request.page.is_none() {
                book.page_count
            } else {
                current_page
            };
            (page, true)
        }
        _ => (current_page, current_page >= book.page_count),
    };

    // Update progress using existing repository
    ReadProgressRepository::upsert(&state.db, user_id, book_id, current_page, completed)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update reading progress: {}", e)))?;

    // Komga returns 204 No Content on success
    Ok(StatusCode::NO_CONTENT)
}

/// Delete reading progress for a book (mark as unread)
///
/// Removes all reading progress for a book, effectively marking it as unread.
///
/// ## Endpoint
/// `DELETE /{prefix}/api/v1/books/{bookId}/read-progress`
///
/// ## Response
/// - 204 No Content on success
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    delete,
    path = "/{prefix}/api/v1/books/{book_id}/read-progress",
    responses(
        (status = 204, description = "Progress deleted successfully"),
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
pub async fn delete_progress(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(book_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let user_id = auth.user_id;

    // Verify book exists (optional but provides better error messages)
    let _book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    // Delete progress using existing repository
    ReadProgressRepository::delete(&state.db, user_id, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete reading progress: {}", e)))?;

    // Komga returns 204 No Content on success
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use crate::api::routes::komga::dto::book::KomgaReadProgressUpdateDto;

    #[test]
    fn test_update_dto_deserialization_komic_format() {
        // Test actual Komic request format
        let json = r#"{"completed":false,"page":151}"#;
        let dto: KomgaReadProgressUpdateDto = serde_json::from_str(json).unwrap();
        assert_eq!(dto.page, Some(151));
        assert_eq!(dto.completed, Some(false));
        assert!(dto.device_id.is_none());
        assert!(dto.device_name.is_none());
    }

    #[test]
    fn test_update_dto_deserialization_minimal() {
        // Test with just page
        let json = r#"{"page":42}"#;
        let dto: KomgaReadProgressUpdateDto = serde_json::from_str(json).unwrap();
        assert_eq!(dto.page, Some(42));
        assert!(dto.completed.is_none());
    }

    #[test]
    fn test_update_dto_deserialization_completed_only() {
        // Test with just completed flag
        let json = r#"{"completed":true}"#;
        let dto: KomgaReadProgressUpdateDto = serde_json::from_str(json).unwrap();
        assert!(dto.page.is_none());
        assert_eq!(dto.completed, Some(true));
    }
}

// ============================================================================
// Series Read Progress Handlers
// ============================================================================

/// Mark all books in a series as read
///
/// Marks all books in a series as completed (read) for the current user.
/// This is equivalent to marking each book individually as completed.
///
/// ## Endpoint
/// `POST /{prefix}/api/v1/series/{seriesId}/read-progress`
///
/// ## Response
/// - 204 No Content on success (Komga behavior)
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    post,
    path = "/{prefix}/api/v1/series/{series_id}/read-progress",
    responses(
        (status = 204, description = "Series marked as read"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn mark_series_as_read(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let user_id = auth.user_id;

    // Verify series exists
    let _series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get all books in the series (excluding deleted)
    let books = BookRepository::list_by_series(&state.db, series_id, false)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    // Build list of (book_id, page_count) tuples for marking as read
    let book_data: Vec<(Uuid, i32)> = books.iter().map(|b| (b.id, b.page_count)).collect();

    // Mark all books as read
    ReadProgressRepository::mark_series_as_read(&state.db, user_id, book_data)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to mark series as read: {}", e)))?;

    // Komga returns 204 No Content on success
    Ok(StatusCode::NO_CONTENT)
}

/// Mark all books in a series as unread
///
/// Removes all reading progress for all books in a series, effectively marking
/// the entire series as unread for the current user.
///
/// ## Endpoint
/// `DELETE /{prefix}/api/v1/series/{seriesId}/read-progress`
///
/// ## Response
/// - 204 No Content on success
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    delete,
    path = "/{prefix}/api/v1/series/{series_id}/read-progress",
    responses(
        (status = 204, description = "Series marked as unread"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        ("series_id" = Uuid, Path, description = "Series ID")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn mark_series_as_unread(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path(series_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let user_id = auth.user_id;

    // Verify series exists
    let _series = SeriesRepository::get_by_id(&state.db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Series not found".to_string()))?;

    // Get all books in the series (excluding deleted)
    let books = BookRepository::list_by_series(&state.db, series_id, false)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch books: {}", e)))?;

    // Get book IDs for deletion
    let book_ids: Vec<Uuid> = books.iter().map(|b| b.id).collect();

    // Delete all reading progress for these books
    ReadProgressRepository::mark_series_as_unread(&state.db, user_id, book_ids)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to mark series as unread: {}", e)))?;

    // Komga returns 204 No Content on success
    Ok(StatusCode::NO_CONTENT)
}
