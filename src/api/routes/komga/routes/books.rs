//! Komga-compatible book routes
//!
//! Defines routes for book-related endpoints in the Komga-compatible API.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;

/// Create book routes for the Komga-compatible API
///
/// Routes:
/// - `GET /books/ondeck` - Get books currently in-progress (continue reading)
/// - `POST /books/list` - Search/filter books with request body
/// - `GET /books/{book_id}` - Get book by ID
/// - `GET /books/{book_id}/thumbnail` - Get book thumbnail
/// - `GET /books/{book_id}/file` - Download book file
/// - `GET /books/{book_id}/next` - Get next book in series
/// - `GET /books/{book_id}/previous` - Get previous book in series
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Special endpoints must come before parameterized routes
        .route("/books/ondeck", get(handlers::get_books_ondeck))
        .route("/books/list", post(handlers::search_books))
        // Parameterized book routes
        .route("/books/{book_id}", get(handlers::get_book))
        .route(
            "/books/{book_id}/thumbnail",
            get(handlers::get_book_thumbnail),
        )
        .route("/books/{book_id}/file", get(handlers::download_book_file))
        .route("/books/{book_id}/next", get(handlers::get_next_book))
        .route(
            "/books/{book_id}/previous",
            get(handlers::get_previous_book),
        )
        .route(
            "/books/{book_id}/manifest",
            get(handlers::get_epub_manifest),
        )
        .route(
            "/books/{book_id}/manifest/epub",
            get(handlers::get_epub_manifest),
        )
        .route(
            "/books/{book_id}/resource/{*resource}",
            get(handlers::get_epub_resource),
        )
}
