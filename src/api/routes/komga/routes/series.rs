//! Komga-compatible series routes
//!
//! Defines routes for series-related endpoints in the Komga-compatible API.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{routing::get, Router};
use std::sync::Arc;

/// Create series routes for the Komga-compatible API
///
/// Routes:
/// - `GET /series` - List all series (paginated)
/// - `GET /series/new` - Get recently added series
/// - `GET /series/updated` - Get recently updated series
/// - `GET /series/:series_id` - Get series by ID
/// - `GET /series/:series_id/thumbnail` - Get series thumbnail
/// - `GET /series/:series_id/books` - Get books in series
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/series", get(handlers::list_series))
        .route("/series/new", get(handlers::get_series_new))
        .route("/series/updated", get(handlers::get_series_updated))
        .route("/series/:series_id", get(handlers::get_series))
        .route(
            "/series/:series_id/thumbnail",
            get(handlers::get_series_thumbnail),
        )
        .route("/series/:series_id/books", get(handlers::get_series_books))
}
