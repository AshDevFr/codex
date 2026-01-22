//! Komga-compatible page routes
//!
//! Defines routes for page-related endpoints in the Komga-compatible API.
//! These routes handle page listing, streaming, and thumbnail generation.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{routing::get, Router};
use std::sync::Arc;

/// Create page routes for the Komga-compatible API
///
/// Routes:
/// - `GET /books/:book_id/pages` - List all pages for a book
/// - `GET /books/:book_id/pages/:page_number` - Get a specific page image
/// - `GET /books/:book_id/pages/:page_number/thumbnail` - Get a page thumbnail
///
/// Note: These routes are nested under `/books` but defined separately
/// for organizational clarity.
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // List all pages for a book
        .route("/books/:book_id/pages", get(handlers::list_pages))
        // Get a specific page image (must come before thumbnail route)
        .route(
            "/books/:book_id/pages/:page_number",
            get(handlers::get_page),
        )
        // Get a page thumbnail
        .route(
            "/books/:book_id/pages/:page_number/thumbnail",
            get(handlers::get_page_thumbnail),
        )
}
