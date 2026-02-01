//! Book routes
//!
//! Handles book operations including CRUD, metadata, pages, reading progress,
//! and file downloads.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use std::sync::Arc;

/// Create book routes
///
/// All routes are protected (authentication required).
///
/// Routes include:
/// - Book CRUD: /books, /books/:id
/// - Book collections: /books/in-progress, /books/recently-added, etc.
/// - Metadata: /books/:id/metadata
/// - Pages: /books/:id/pages/:page_number
/// - Progress: /books/:id/progress, /books/:id/read, /books/:id/unread
/// - Files: /books/:id/file, /books/:id/thumbnail
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Book CRUD routes
        .route("/books", get(handlers::list_books))
        .route("/books/list", post(handlers::list_books_filtered))
        .route("/books/:book_id", get(handlers::get_book))
        .route("/books/:book_id", patch(handlers::patch_book))
        .route(
            "/books/:book_id/adjacent",
            get(handlers::get_adjacent_books),
        )
        .route("/books/:book_id/file", get(handlers::get_book_file))
        .route(
            "/books/:book_id/thumbnail",
            get(handlers::get_book_thumbnail),
        )
        .route("/books/:book_id/cover", post(handlers::upload_book_cover))
        // Book analysis routes
        .route(
            "/books/:book_id/analyze",
            post(handlers::trigger_book_analysis),
        )
        .route(
            "/books/:book_id/analyze-unanalyzed",
            post(handlers::trigger_book_unanalyzed_analysis),
        )
        // Book metadata routes
        .route(
            "/books/:book_id/metadata",
            put(handlers::replace_book_metadata),
        )
        .route(
            "/books/:book_id/metadata",
            patch(handlers::patch_book_metadata),
        )
        // Book metadata lock routes
        .route(
            "/books/:book_id/metadata/locks",
            get(handlers::get_book_metadata_locks),
        )
        .route(
            "/books/:book_id/metadata/locks",
            put(handlers::update_book_metadata_locks),
        )
        // Book collection routes
        .route("/books/in-progress", get(handlers::list_in_progress_books))
        .route("/books/on-deck", get(handlers::list_on_deck_books))
        .route(
            "/books/recently-added",
            get(handlers::list_recently_added_books),
        )
        .route(
            "/books/recently-read",
            get(handlers::list_recently_read_books),
        )
        // Error endpoints (grouped with retry)
        .route("/books/errors", get(handlers::list_books_with_errors))
        .route("/books/:book_id/retry", post(handlers::retry_book_errors))
        .route(
            "/books/retry-all-errors",
            post(handlers::retry_all_book_errors),
        )
        // Page routes
        .route(
            "/books/:book_id/pages/:page_number",
            get(handlers::get_page_image),
        )
        // Reading progress routes
        .route(
            "/books/:book_id/progress",
            put(handlers::update_reading_progress),
        )
        .route(
            "/books/:book_id/progress",
            get(handlers::get_reading_progress),
        )
        .route(
            "/books/:book_id/progress",
            delete(handlers::delete_reading_progress),
        )
        .route("/progress", get(handlers::get_user_progress))
        // Mark as read/unread routes
        .route("/books/:book_id/read", post(handlers::mark_book_as_read))
        .route(
            "/books/:book_id/unread",
            post(handlers::mark_book_as_unread),
        )
        // Bulk operations
        .route("/books/bulk/read", post(handlers::bulk_mark_books_as_read))
        .route(
            "/books/bulk/unread",
            post(handlers::bulk_mark_books_as_unread),
        )
        .route("/books/bulk/analyze", post(handlers::bulk_analyze_books))
        .route(
            "/books/bulk/thumbnails/generate",
            post(handlers::bulk_generate_book_thumbnails),
        )
}
