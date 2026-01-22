//! Current user routes
//!
//! Handles current user's preferences, integrations, ratings, and API keys.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use std::sync::Arc;

/// Create current user routes
///
/// All routes are protected (authentication required).
///
/// Routes:
/// - Preferences: /user/preferences
/// - Integrations: /user/integrations
/// - Ratings: /user/ratings
/// - Sharing tags: /user/sharing-tags
/// - API keys: /api-keys
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // User ratings routes
        .route("/user/ratings", get(handlers::list_user_ratings))
        // Current user's sharing tags route
        .route(
            "/user/sharing-tags",
            get(handlers::sharing_tags::get_my_sharing_tags),
        )
        // User preferences routes
        .route(
            "/user/preferences",
            get(handlers::user_preferences::get_all_preferences),
        )
        .route(
            "/user/preferences",
            put(handlers::user_preferences::set_bulk_preferences),
        )
        .route(
            "/user/preferences/:key",
            get(handlers::user_preferences::get_preference),
        )
        .route(
            "/user/preferences/:key",
            put(handlers::user_preferences::set_preference),
        )
        .route(
            "/user/preferences/:key",
            delete(handlers::user_preferences::delete_preference),
        )
        // User integrations routes
        .route(
            "/user/integrations",
            get(handlers::user_integrations::list_user_integrations),
        )
        .route(
            "/user/integrations",
            post(handlers::user_integrations::connect_integration),
        )
        .route(
            "/user/integrations/:name",
            get(handlers::user_integrations::get_user_integration),
        )
        .route(
            "/user/integrations/:name",
            patch(handlers::user_integrations::update_integration_settings),
        )
        .route(
            "/user/integrations/:name",
            delete(handlers::user_integrations::disconnect_integration),
        )
        .route(
            "/user/integrations/:name/callback",
            post(handlers::user_integrations::oauth_callback),
        )
        .route(
            "/user/integrations/:name/sync",
            post(handlers::user_integrations::trigger_sync),
        )
        // API key routes
        .route("/api-keys", get(handlers::api_keys::list_api_keys))
        .route("/api-keys", post(handlers::api_keys::create_api_key))
        .route(
            "/api-keys/:api_key_id",
            get(handlers::api_keys::get_api_key),
        )
        .route(
            "/api-keys/:api_key_id",
            patch(handlers::api_keys::update_api_key),
        )
        .route(
            "/api-keys/:api_key_id",
            delete(handlers::api_keys::delete_api_key),
        )
}
