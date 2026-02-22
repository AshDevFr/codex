//! Task queue routes
//!
//! Handles task queue operations and thumbnail generation tasks.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    Router,
    routing::{delete, get, post},
};
use std::sync::Arc;

/// Create task queue routes
///
/// All routes are protected (authentication required).
///
/// Routes:
/// - Tasks: /tasks, /tasks/{id}, /tasks/stats
/// - Task operations: /tasks/{id}/cancel, /tasks/{id}/retry, /tasks/{id}/unlock
/// - Task stream: /tasks/stream
/// - Book thumbnails: /books/thumbnails/generate, /books/{id}/thumbnail/generate, /libraries/{id}/books/thumbnails/generate
/// - Series thumbnails: /series/thumbnails/generate, /series/{id}/thumbnail/generate, /libraries/{id}/series/thumbnails/generate
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Task Queue routes - distributed task queue
        .route("/tasks", get(handlers::task_queue::list_tasks))
        .route("/tasks", post(handlers::task_queue::create_task))
        .route("/tasks/{task_id}", get(handlers::task_queue::get_task))
        .route(
            "/tasks/{task_id}/cancel",
            post(handlers::task_queue::cancel_task),
        )
        .route(
            "/tasks/{task_id}/unlock",
            post(handlers::task_queue::unlock_task),
        )
        .route(
            "/tasks/{task_id}/retry",
            post(handlers::task_queue::retry_task),
        )
        .route("/tasks/stats", get(handlers::task_queue::get_task_stats))
        .route(
            "/tasks/purge",
            delete(handlers::task_queue::purge_old_tasks),
        )
        .route("/tasks/nuke", delete(handlers::task_queue::nuke_all_tasks))
        // Task progress stream
        .route("/tasks/stream", get(handlers::task_progress_stream))
        // Book thumbnail generation routes
        .route(
            "/books/thumbnails/generate",
            post(handlers::task_queue::generate_book_thumbnails),
        )
        .route(
            "/books/{book_id}/thumbnail/generate",
            post(handlers::task_queue::generate_book_thumbnail),
        )
        .route(
            "/libraries/{library_id}/books/thumbnails/generate",
            post(handlers::task_queue::generate_library_book_thumbnails),
        )
        // Series thumbnail generation routes
        .route(
            "/series/thumbnails/generate",
            post(handlers::task_queue::generate_series_thumbnails),
        )
        .route(
            "/series/{series_id}/thumbnail/generate",
            post(handlers::task_queue::generate_series_thumbnail),
        )
        .route(
            "/libraries/{library_id}/series/thumbnails/generate",
            post(handlers::task_queue::generate_library_series_thumbnails),
        )
}
