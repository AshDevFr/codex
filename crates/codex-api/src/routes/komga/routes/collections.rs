//! Komga-compatible collection routes (read-only).

use super::super::handlers;
use crate::extractors::AppState;
use axum::{Router, routing::get};
use std::sync::Arc;

/// Collection routes shared between Komga API v1 and v2.
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/collections", get(handlers::list_collections))
        .route(
            "/collections/{collection_id}",
            get(handlers::get_collection),
        )
        .route(
            "/collections/{collection_id}/series",
            get(handlers::get_collection_series),
        )
        .route(
            "/collections/{collection_id}/thumbnail",
            get(handlers::get_collection_thumbnail),
        )
        .route(
            "/series/{series_id}/collections",
            get(handlers::get_series_collections),
        )
}
