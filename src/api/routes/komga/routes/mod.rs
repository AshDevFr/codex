//! Komga-compatible API route definitions
//!
//! This module assembles all routes for the Komga-compatible API.
//! Routes are organized by resource type and composed into a single router.

mod books;
mod libraries;
mod pages;
mod read_progress;
mod series;
mod users;

use crate::api::extractors::AppState;
use axum::Router;
use std::sync::Arc;

/// Create the combined Komga-compatible API router
///
/// All routes are mounted under `/{prefix}/api/v1/` where the prefix
/// is configurable (default: `komgav1`).
///
/// Routes:
/// - `/libraries` - Library listing and details
/// - `/series` - Series listing, search, and details
/// - `/books` - Book listing, details, file downloads, page streaming, and read progress
/// - `/users/me` - Current user information
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Library routes (merged - path prefix included in module)
        .merge(libraries::routes(state.clone()))
        // Series routes
        .merge(series::routes(state.clone()))
        // Book routes
        .merge(books::routes(state.clone()))
        // Page routes (nested under /books but separate for organization)
        .merge(pages::routes(state.clone()))
        // Read progress routes
        .merge(read_progress::routes(state.clone()))
        // User routes
        .merge(users::routes(state.clone()))
        // Apply state to all routes
        .with_state(state)
}
