//! OPDS Route Definitions
//!
//! Defines all OPDS 1.2 catalog routes.

use super::handlers::{
    book_page_image, book_pages, collection_series, library_series, list_collections,
    list_libraries, list_readlists, opensearch_descriptor, readlist_books, root_catalog, search,
    series_books,
};
use crate::extractors::AppState;
use axum::{Router, routing::get};
use std::sync::Arc;

/// Create OPDS router with all OPDS endpoints
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(root_catalog))
        .route("/libraries", get(list_libraries))
        .route("/libraries/{library_id}", get(library_series))
        .route("/series/{series_id}", get(series_books))
        .route("/collections", get(list_collections))
        .route("/collections/{collection_id}", get(collection_series))
        .route("/readlists", get(list_readlists))
        .route("/readlists/{read_list_id}", get(readlist_books))
        .route("/books/{book_id}/pages", get(book_pages))
        .route("/books/{book_id}/pages/{page_number}", get(book_page_image))
        .route("/search.xml", get(opensearch_descriptor))
        .route("/search", get(search))
        .with_state(state)
}
