//! Collection routes
//!
//! Shared, ordered groupings of series. Reads are available to all roles;
//! create/modify/delete are gated by the collection permissions.

use super::super::handlers;
use crate::extractors::AppState;
use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};
use std::sync::Arc;

/// Create collection routes.
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/collections", get(handlers::list_collections))
        .route("/collections", post(handlers::create_collection))
        .route(
            "/collections/{collection_id}",
            get(handlers::get_collection),
        )
        .route(
            "/collections/{collection_id}",
            patch(handlers::update_collection),
        )
        .route(
            "/collections/{collection_id}",
            delete(handlers::delete_collection),
        )
        .route(
            "/collections/{collection_id}/series",
            get(handlers::get_collection_series),
        )
        .route(
            "/collections/{collection_id}/series",
            post(handlers::add_collection_series),
        )
        .route(
            "/collections/{collection_id}/series",
            put(handlers::reorder_collection_series),
        )
        .route(
            "/collections/{collection_id}/series/{series_id}",
            delete(handlers::remove_collection_series),
        )
        .route(
            "/collections/{collection_id}/thumbnail",
            get(handlers::get_collection_thumbnail),
        )
        // Reverse lookup: collections that contain a given series.
        .route(
            "/series/{series_id}/collections",
            get(handlers::get_series_collections),
        )
}
