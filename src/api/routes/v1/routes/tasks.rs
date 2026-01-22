//! Task queue routes
//!
//! Handles task queue operations and thumbnail generation tasks.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;

/// Create task queue routes
///
/// All routes are protected (authentication required).
///
/// Routes:
/// - Tasks: /tasks, /tasks/:id, /tasks/stats
/// - Task operations: /tasks/:id/cancel, /tasks/:id/retry, /tasks/:id/unlock
/// - Thumbnails: /thumbnails/generate, /libraries/:id/thumbnails/generate, etc.
/// - Task stream: /tasks/stream
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Task Queue routes - distributed task queue
        .route("/tasks", get(handlers::task_queue::list_tasks))
        .route("/tasks", post(handlers::task_queue::create_task))
        .route("/tasks/:task_id", get(handlers::task_queue::get_task))
        .route(
            "/tasks/:task_id/cancel",
            post(handlers::task_queue::cancel_task),
        )
        .route(
            "/tasks/:task_id/unlock",
            post(handlers::task_queue::unlock_task),
        )
        .route(
            "/tasks/:task_id/retry",
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
        // Thumbnail generation routes
        .route(
            "/thumbnails/generate",
            post(handlers::task_queue::generate_thumbnails),
        )
        .route(
            "/libraries/:library_id/thumbnails/generate",
            post(handlers::task_queue::generate_library_thumbnails),
        )
        .route(
            "/series/:series_id/thumbnails/generate",
            post(handlers::task_queue::generate_series_thumbnails),
        )
        .route(
            "/books/:book_id/thumbnail/generate",
            post(handlers::task_queue::generate_book_thumbnail),
        )
}
