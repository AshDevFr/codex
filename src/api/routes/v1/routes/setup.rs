//! Setup routes
//!
//! Handles initial application setup when no users exist.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    Router,
    routing::{get, patch, post},
};
use std::sync::Arc;

/// Create setup routes
///
/// These routes are public but only functional when no users exist in the system.
///
/// Routes:
/// - GET /status - Check if initial setup is needed
/// - POST /initialize - Create initial admin user
/// - PATCH /settings - Configure initial settings
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/status", get(handlers::setup::setup_status))
        .route("/initialize", post(handlers::setup::initialize_setup))
        .route(
            "/settings",
            patch(handlers::setup::configure_initial_settings),
        )
}
