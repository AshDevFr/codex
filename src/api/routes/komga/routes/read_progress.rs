//! Komga-compatible read progress routes
//!
//! Defines routes for read progress endpoints in the Komga-compatible API.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    routing::{patch, post},
    Router,
};
use std::sync::Arc;

/// Create read progress routes for the Komga-compatible API
///
/// Routes:
/// - `PATCH /books/:book_id/read-progress` - Update reading progress
/// - `DELETE /books/:book_id/read-progress` - Delete reading progress (mark as unread)
/// - `POST /series/:series_id/read-progress` - Mark all books in series as read
/// - `DELETE /series/:series_id/read-progress` - Mark all books in series as unread
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/books/:book_id/read-progress",
            patch(handlers::update_progress).delete(handlers::delete_progress),
        )
        .route(
            "/series/:series_id/read-progress",
            post(handlers::mark_series_as_read).delete(handlers::mark_series_as_unread),
        )
}
