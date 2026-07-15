//! Want-to-read routes
//!
//! Per-user on-deck queue. All routes require authentication; each scopes to the
//! authenticated user.

use super::super::handlers;
use crate::extractors::AppState;
use axum::{
    Router,
    routing::{delete, get, post, put},
};
use std::sync::Arc;

/// Create want-to-read routes.
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/want-to-read", get(handlers::list_want_to_read))
        .route("/want-to-read", post(handlers::add_want_to_read))
        .route("/want-to-read/bulk", post(handlers::bulk_add_want_to_read))
        .route("/want-to-read/order", put(handlers::reorder_want_to_read))
        .route(
            "/want-to-read/series/{series_id}",
            delete(handlers::remove_want_to_read_series),
        )
        .route(
            "/want-to-read/books/{book_id}",
            delete(handlers::remove_want_to_read_book),
        )
}
