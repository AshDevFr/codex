//! Content visibility for handlers.
//!
//! Every handler that returns, streams or changes something tied to a series
//! or a book goes through these helpers, so what a restricted reader can see
//! is decided in one place. A hidden item answers exactly like a missing one
//! (404), so its existence cannot be probed.
//!
//! The filter comes from the per-process cache (see
//! `codex_services::content_filter_cache`), which matters on hot paths such as
//! page images.

use crate::error::ApiError;
use codex_db::entities::{books, series};
use codex_db::repositories::{BookRepository, SeriesRepository, SeriesVisibility};
use codex_services::content_filter::ContentFilter;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

/// The user's content filter.
pub async fn content_filter(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Arc<ContentFilter>, ApiError> {
    ContentFilter::cached_for_user(db, user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to load content filter: {}", e)))
}

/// The SQL-level visibility constraint for the user, or `None` when they see
/// everything.
pub async fn user_visibility(
    db: &DatabaseConnection,
    user_id: Uuid,
) -> Result<Option<SeriesVisibility>, ApiError> {
    Ok(content_filter(db, user_id).await?.to_visibility())
}

/// Fail with 404 unless the series exists and the user can see it.
pub async fn require_visible_series(
    db: &DatabaseConnection,
    user_id: Uuid,
    series_id: Uuid,
) -> Result<series::Model, ApiError> {
    let series = SeriesRepository::get_by_id(db, series_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or_else(series_not_found)?;
    if !content_filter(db, user_id)
        .await?
        .is_series_visible(series.id)
    {
        return Err(series_not_found());
    }
    Ok(series)
}

/// Fail with 404 unless the book exists and the user can see its series.
pub async fn require_visible_book(
    db: &DatabaseConnection,
    user_id: Uuid,
    book_id: Uuid,
) -> Result<books::Model, ApiError> {
    let book = BookRepository::get_by_id(db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(book_not_found)?;
    if !content_filter(db, user_id)
        .await?
        .is_book_visible(book.series_id)
    {
        return Err(book_not_found());
    }
    Ok(book)
}

/// Fail with 404 unless the user can see this already-loaded book.
pub async fn ensure_book_visible(
    db: &DatabaseConnection,
    user_id: Uuid,
    book: &books::Model,
) -> Result<(), ApiError> {
    if content_filter(db, user_id)
        .await?
        .is_book_visible(book.series_id)
    {
        Ok(())
    } else {
        Err(book_not_found())
    }
}

/// Fail with 404 unless the user can see this series id (without loading it).
pub async fn ensure_series_visible(
    db: &DatabaseConnection,
    user_id: Uuid,
    series_id: Uuid,
) -> Result<(), ApiError> {
    if content_filter(db, user_id)
        .await?
        .is_series_visible(series_id)
    {
        Ok(())
    } else {
        Err(series_not_found())
    }
}

fn series_not_found() -> ApiError {
    ApiError::NotFound("Series not found".to_string())
}

fn book_not_found() -> ApiError {
    ApiError::NotFound("Book not found".to_string())
}
