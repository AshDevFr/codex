//! OPDS 2.0 Route Definitions
//!
//! Defines all OPDS 2.0 catalog routes (JSON-based).

use super::handlers::{
    opds2_libraries, opds2_library_series, opds2_recent, opds2_root, opds2_search,
    opds2_series_books,
};
use crate::api::extractors::AuthState;
use axum::{routing::get, Router};
use std::sync::Arc;

/// Create OPDS 2.0 router with all OPDS 2.0 endpoints
///
/// All endpoints return JSON (application/opds+json) instead of XML
pub fn create_router(state: Arc<AuthState>) -> Router {
    Router::new()
        .route("/", get(opds2_root))
        .route("/libraries", get(opds2_libraries))
        .route("/libraries/:library_id", get(opds2_library_series))
        .route("/series/:series_id", get(opds2_series_books))
        .route("/recent", get(opds2_recent))
        .route("/search", get(opds2_search))
        .with_state(state)
}
