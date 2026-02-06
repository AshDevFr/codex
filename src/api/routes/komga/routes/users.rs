//! Komga-compatible user routes
//!
//! Defines routes for user endpoints in the Komga-compatible API.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{Router, routing::get};
use std::sync::Arc;

/// Create user routes for the Komga-compatible API
///
/// Routes:
/// - `GET /users/me` - Get current user information
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new().route("/users/me", get(handlers::get_current_user))
}
