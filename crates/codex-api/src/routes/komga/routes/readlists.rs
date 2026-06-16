//! Komga-compatible read list routes (read-only).

use super::super::handlers;
use crate::extractors::AppState;
use axum::{Router, routing::get};
use std::sync::Arc;

/// Read list routes shared between Komga API v1 and v2.
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/readlists", get(handlers::list_readlists))
        .route("/readlists/{read_list_id}", get(handlers::get_readlist))
        .route(
            "/readlists/{read_list_id}/books",
            get(handlers::get_readlist_books),
        )
        .route(
            "/readlists/{read_list_id}/thumbnail",
            get(handlers::get_readlist_thumbnail),
        )
        .route(
            "/books/{book_id}/readlists",
            get(handlers::get_book_readlists),
        )
}
