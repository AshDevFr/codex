//! User plugin routes
//!
//! Handles user plugin management: listing, enabling/disabling, OAuth flows.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    Router,
    routing::{get, patch, post},
};
use std::sync::Arc;

/// Create user plugin routes
///
/// All routes are protected (authentication required) except the OAuth callback.
///
/// Routes:
/// - List plugins: GET /user/plugins
/// - Get plugin: GET /user/plugins/:plugin_id
/// - Enable: POST /user/plugins/:plugin_id/enable
/// - Disable: POST /user/plugins/:plugin_id/disable
/// - Update config: PATCH /user/plugins/:plugin_id/config
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
            get(handlers::user_plugins::get_user_plugin)
                .delete(handlers::user_plugins::disconnect_plugin),
        )
        .route(
            "/user/plugins/:plugin_id/config",
            patch(handlers::user_plugins::update_user_plugin_config),
        )
        // User credentials (personal access token)
        .route(
            "/user/plugins/:plugin_id/credentials",
            post(handlers::user_plugins::set_user_credentials),
        )
        // Sync operations
        .route(
            "/user/plugins/:plugin_id/sync",
            post(handlers::user_plugins::trigger_sync),
        )
        .route(
            "/user/plugins/:plugin_id/sync/status",
            get(handlers::user_plugins::get_sync_status),
        )
        // User-scoped plugin tasks (no TasksRead permission required)
        .route(
            "/user/plugins/:plugin_id/tasks",
            get(handlers::user_plugins::get_plugin_tasks),
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
