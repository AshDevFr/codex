//! Read list routes
//!
//! Shared, ordered groupings of books across series. Reads are available to all
//! roles; create/modify/delete are gated by the read-list permissions.

use super::super::handlers;
use crate::extractors::AppState;
use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};
use std::sync::Arc;

/// Create read list routes.
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/readlists", get(handlers::list_readlists))
        .route("/readlists", post(handlers::create_readlist))
        .route("/readlists/{read_list_id}", get(handlers::get_readlist))
        .route(
            "/readlists/{read_list_id}",
            patch(handlers::update_readlist),
        )
        .route(
            "/readlists/{read_list_id}",
            delete(handlers::delete_readlist),
        )
        .route(
            "/readlists/{read_list_id}/books",
            get(handlers::get_readlist_books),
        )
        .route(
            "/readlists/{read_list_id}/books",
            post(handlers::add_readlist_books),
        )
        .route(
            "/readlists/{read_list_id}/books",
            put(handlers::reorder_readlist_books),
        )
        .route(
            "/readlists/{read_list_id}/books/{book_id}",
            delete(handlers::remove_readlist_book),
        )
        .route(
            "/readlists/{read_list_id}/thumbnail",
            get(handlers::get_readlist_thumbnail),
        )
        // Reverse lookup: read lists that contain a given book.
        .route(
            "/books/{book_id}/readlists",
            get(handlers::get_book_readlists),
        )
}
