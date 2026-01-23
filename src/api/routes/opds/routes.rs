//! OPDS Route Definitions
//!
//! Defines all OPDS 1.2 catalog routes.

use super::handlers::{
    book_pages, library_series, list_libraries, opensearch_descriptor, root_catalog, search,
    series_books,
};
use crate::api::extractors::AppState;
use axum::{routing::get, Router};
use std::sync::Arc;

/// Create OPDS router with all OPDS endpoints
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(root_catalog))
        .route("/libraries", get(list_libraries))
        .route("/libraries/:library_id", get(library_series))
        .route("/series/:series_id", get(series_books))
        .route("/books/:book_id/pages", get(book_pages))
        .route("/search.xml", get(opensearch_descriptor))
        .route("/search", get(search))
        .with_state(state)
}
