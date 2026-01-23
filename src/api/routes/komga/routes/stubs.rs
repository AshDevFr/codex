//! Stub routes for unimplemented Komga endpoints
//!
//! These routes return empty results for endpoints that Komic expects
//! but Codex doesn't fully support. This prevents 404 errors in the client.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{routing::get, Router};
use std::sync::Arc;

/// Create stub routes for the Komga-compatible API (v1)
///
/// Routes:
/// - `GET /collections` - List collections (always empty)
/// - `GET /readlists` - List read lists (always empty)
/// - `GET /genres` - List genres (always empty)
/// - `GET /tags` - List tags (always empty)
/// - `GET /languages` - List languages (always empty)
/// - `GET /publishers` - List publishers (always empty)
/// - `GET /age-ratings` - List age ratings (always empty)
pub fn routes_v1(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/collections", get(handlers::list_collections))
        .route("/readlists", get(handlers::list_readlists))
        .route("/genres", get(handlers::list_genres))
        .route("/tags", get(handlers::list_tags))
        .route("/languages", get(handlers::list_languages))
        .route("/publishers", get(handlers::list_publishers))
        .route("/age-ratings", get(handlers::list_age_ratings))
}

/// Create stub routes for the Komga-compatible API (v2)
///
/// Routes:
/// - `GET /authors` - List authors (always empty)
pub fn routes_v2(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new().route("/authors", get(handlers::list_authors_v2))
}
