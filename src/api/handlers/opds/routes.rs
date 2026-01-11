use super::{
    opds_book_pages, opds_library_series, opds_list_libraries, opds_search, opds_series_books,
    opensearch_descriptor, root_catalog,
};
use crate::api::extractors::AuthState;
use axum::{routing::get, Router};
use std::sync::Arc;

/// Create OPDS router with all OPDS endpoints
pub fn opds_routes(state: Arc<AuthState>) -> Router {
    Router::new()
        .route("/", get(root_catalog))
        .route("/libraries", get(opds_list_libraries))
        .route("/libraries/:library_id", get(opds_library_series))
        .route("/series/:series_id", get(opds_series_books))
        .route("/books/:book_id/pages", get(opds_book_pages))
        .route("/search.xml", get(opensearch_descriptor))
        .route("/search", get(opds_search))
        .with_state(state)
}
