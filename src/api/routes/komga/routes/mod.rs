//! Komga-compatible API route definitions
//!
//! This module assembles all routes for the Komga-compatible API.
//! Routes are organized by resource type and composed into separate v1 and v2 routers.

mod books;
mod libraries;
mod pages;
mod read_progress;
mod series;
mod stubs;
mod users;

use crate::api::extractors::AppState;
use axum::Router;
use std::sync::Arc;

/// Create routes shared between v1 and v2
///
/// These routes are identical in both API versions:
/// - `/libraries` - Library listing and details
/// - `/series` - Series listing, search, and details
/// - `/books` - Book listing, details, file downloads, page streaming, and read progress
/// - `/users/me` - Current user information
fn shared_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Library routes
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
}

/// Create the Komga-compatible API v1 router
///
/// Mounted at `/{prefix}/api/v1/` where the prefix is configurable (default: `komga`).
///
/// Routes:
/// - All shared routes (libraries, series, books, users)
/// - `/collections` - Collections (stub - always empty)
/// - `/readlists` - Read lists (stub - always empty)
/// - `/genres` - Genres (stub - always empty)
/// - `/tags` - Tags (stub - always empty)
pub fn create_v1_router(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(shared_routes(state.clone()))
        // V1-specific stub routes
        .merge(stubs::routes_v1(state.clone()))
        .with_state(state)
}

/// Create the Komga-compatible API v2 router
///
/// Mounted at `/{prefix}/api/v2/` where the prefix is configurable (default: `komga`).
/// Komic app uses `/api/v2/users/me` for connection testing.
///
/// Routes:
/// - All shared routes (libraries, series, books, users)
/// - `/authors` - Authors (stub - always empty)
pub fn create_v2_router(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(shared_routes(state.clone()))
        // V2-specific stub routes
        .merge(stubs::routes_v2(state.clone()))
        .with_state(state)
}
