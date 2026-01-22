//! Komga-compatible library routes
//!
//! Defines routes for library-related endpoints in the Komga-compatible API.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{routing::get, Router};
use std::sync::Arc;

/// Create library routes for the Komga-compatible API
///
/// Routes:
/// - `GET /libraries` - List all libraries
/// - `GET /libraries/:library_id` - Get library by ID
/// - `GET /libraries/:library_id/thumbnail` - Get library thumbnail
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/libraries", get(handlers::list_libraries))
        .route("/libraries/:library_id", get(handlers::get_library))
        .route(
            "/libraries/:library_id/thumbnail",
            get(handlers::get_library_thumbnail),
        )
}
