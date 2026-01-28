//! Plugin routes (user-facing)
//!
//! Handles user-facing plugin operations:
//! - Plugin action discovery
//! - Plugin method execution

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

/// Create plugin routes
///
/// Routes:
/// - GET /plugins/actions - Get available plugin actions for a scope
/// - POST /plugins/:id/execute - Execute a plugin method
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/plugins/actions",
            get(handlers::plugin_actions::get_plugin_actions),
        )
        .route(
            "/plugins/:id/execute",
            post(handlers::plugin_actions::execute_plugin),
        )
}
