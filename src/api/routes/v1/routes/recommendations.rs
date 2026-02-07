//! Recommendation routes
//!
//! Handles recommendation endpoints: get, refresh, and dismiss.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;

/// Create recommendation routes
///
/// All routes require authentication.
///
/// Routes:
/// - GET /user/recommendations - Get personalized recommendations
/// - POST /user/recommendations/refresh - Refresh cached recommendations
/// - POST /user/recommendations/:external_id/dismiss - Dismiss a recommendation
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/user/recommendations",
            get(handlers::recommendations::get_recommendations),
        )
        .route(
            "/user/recommendations/refresh",
            post(handlers::recommendations::refresh_recommendations),
        )
        .route(
            "/user/recommendations/:external_id/dismiss",
            post(handlers::recommendations::dismiss_recommendation),
        )
}
