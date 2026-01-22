//! Komga-compatible read progress routes
//!
//! Defines routes for read progress endpoints in the Komga-compatible API.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{routing::patch, Router};
use std::sync::Arc;

/// Create read progress routes for the Komga-compatible API
///
/// Routes:
/// - `PATCH /books/:book_id/read-progress` - Update reading progress
/// - `DELETE /books/:book_id/read-progress` - Delete reading progress (mark as unread)
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new().route(
        "/books/:book_id/read-progress",
        patch(handlers::update_progress).delete(handlers::delete_progress),
    )
}
