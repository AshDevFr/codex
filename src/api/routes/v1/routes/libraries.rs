//! Library routes
//!
//! Handles library management including CRUD operations, scanning, and library-specific
//! book/series listings.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    Router,
    routing::{delete, get, patch, post},
};
use std::sync::Arc;

/// Create library routes
///
/// All routes are protected (authentication required).
///
/// Routes include:
/// - Library CRUD: /libraries, /libraries/:id
/// - Scan operations: /libraries/:id/scan, /libraries/:id/scan-status
/// - Library books: /libraries/:id/books, /libraries/:id/books/in-progress, etc.
/// - Library series: /libraries/:id/series, /libraries/:id/series/in-progress, etc.
/// - Thumbnail generation: /libraries/:id/thumbnails/generate
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Library CRUD routes
        .route("/libraries", get(handlers::list_libraries))
        .route("/libraries", post(handlers::create_library))
        .route("/libraries/preview-scan", post(handlers::preview_scan))
        .route("/libraries/:library_id", get(handlers::get_library))
        .route("/libraries/:library_id", patch(handlers::update_library))
        .route("/libraries/:library_id", delete(handlers::delete_library))
        .route(
            "/libraries/:library_id/purge-deleted",
            delete(handlers::purge_deleted_books),
        )
        // Library-specific book routes
        .route(
            "/libraries/:library_id/books",
            get(handlers::list_library_books),
        )
        .route(
            "/libraries/:library_id/books/in-progress",
            get(handlers::list_library_in_progress_books),
        )
        .route(
            "/libraries/:library_id/books/recently-added",
            get(handlers::list_library_recently_added_books),
        )
        .route(
            "/libraries/:library_id/books/on-deck",
            get(handlers::list_library_on_deck_books),
        )
        .route(
            "/libraries/:library_id/books/recently-read",
            get(handlers::list_library_recently_read_books),
        )
        // Library-specific series routes
        .route(
            "/libraries/:library_id/series",
            get(handlers::list_library_series),
        )
        .route(
            "/libraries/:library_id/series/in-progress",
            get(handlers::list_library_in_progress_series),
        )
        .route(
            "/libraries/:library_id/series/recently-added",
            get(handlers::list_library_recently_added_series),
        )
        .route(
            "/libraries/:library_id/series/recently-updated",
            get(handlers::list_library_recently_updated_series),
        )
        // Scan routes
        .route("/libraries/:library_id/scan", post(handlers::trigger_scan))
        .route(
            "/libraries/:library_id/scan-status",
            get(handlers::get_scan_status),
        )
        .route(
            "/libraries/:library_id/scan/cancel",
            post(handlers::cancel_scan),
        )
        // Analysis routes
        .route(
            "/libraries/:library_id/analyze",
            post(handlers::trigger_library_analysis),
        )
        .route(
            "/libraries/:library_id/analyze-unanalyzed",
            post(handlers::trigger_library_unanalyzed_analysis),
        )
        // Plugin auto-match for library (Phase 5.5)
        .route(
            "/libraries/:library_id/metadata/auto-match/task",
            post(handlers::plugin_actions::enqueue_library_auto_match_tasks),
        )
        // Series title reprocessing
        .route(
            "/libraries/:library_id/series/titles/reprocess",
            post(handlers::task_queue::reprocess_library_series_titles),
        )
}
