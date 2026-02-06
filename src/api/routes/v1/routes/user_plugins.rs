//! User plugin routes
//!
//! Handles user plugin management: listing, enabling/disabling, OAuth flows.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    Router,
    routing::{delete, get, post},
};
use std::sync::Arc;

/// Create user plugin routes
///
/// All routes are protected (authentication required) except the OAuth callback.
///
/// Routes:
/// - List plugins: GET /user/plugins
/// - Enable: POST /user/plugins/:plugin_id/enable
/// - Disable: POST /user/plugins/:plugin_id/disable
/// - Disconnect: DELETE /user/plugins/:plugin_id
/// - OAuth start: POST /user/plugins/:plugin_id/oauth/start
/// - OAuth callback: GET /user/plugins/oauth/callback (no auth - receives redirect)
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // User plugin management
        .route(
            "/user/plugins",
            get(handlers::user_plugins::list_user_plugins),
        )
        .route(
            "/user/plugins/:plugin_id/enable",
            post(handlers::user_plugins::enable_plugin),
        )
        .route(
            "/user/plugins/:plugin_id/disable",
            post(handlers::user_plugins::disable_plugin),
        )
        .route(
            "/user/plugins/:plugin_id",
            delete(handlers::user_plugins::disconnect_plugin),
        )
        // OAuth flow
        .route(
            "/user/plugins/:plugin_id/oauth/start",
            post(handlers::user_plugins::oauth_start),
        )
        .route(
            "/user/plugins/oauth/callback",
            get(handlers::user_plugins::oauth_callback),
        )
}
