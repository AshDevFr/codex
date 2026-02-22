//! OPDS 2.0 Route Definitions
//!
//! Defines all OPDS 2.0 catalog routes (JSON-based).

use super::handlers::{libraries, library_series, recent, root, search, series_books};
use crate::api::extractors::AuthState;
use axum::{Router, routing::get};
use std::sync::Arc;

/// Create OPDS 2.0 router with all OPDS 2.0 endpoints
///
/// All endpoints return JSON (application/opds+json) instead of XML
pub fn create_router(state: Arc<AuthState>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/libraries", get(libraries))
        .route("/libraries/{library_id}", get(library_series))
        .route("/series/{series_id}", get(series_books))
        .route("/recent", get(recent))
        .route("/search", get(search))
        .with_state(state)
}
