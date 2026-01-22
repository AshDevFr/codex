//! User management routes (admin only)
//!
//! Handles user administration including CRUD operations and sharing tag assignments.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use std::sync::Arc;

/// Create user management routes
///
/// All routes are protected and require admin permissions.
///
/// Routes:
/// - GET /users - List all users
/// - POST /users - Create a new user
/// - GET /users/:id - Get user details
/// - PATCH /users/:id - Update user
/// - DELETE /users/:id - Delete user
/// - User sharing tags: /users/:id/sharing-tags
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // User CRUD routes (admin only)
        .route("/users", get(handlers::list_users))
        .route("/users", post(handlers::create_user))
        .route("/users/:user_id", get(handlers::get_user))
        .route("/users/:user_id", patch(handlers::update_user))
        .route("/users/:user_id", delete(handlers::delete_user))
        // User sharing tags routes (admin only)
        .route(
            "/users/:user_id/sharing-tags",
            get(handlers::sharing_tags::get_user_sharing_tags),
        )
        .route(
            "/users/:user_id/sharing-tags",
            put(handlers::sharing_tags::set_user_sharing_tag),
        )
        .route(
            "/users/:user_id/sharing-tags/:tag_id",
            delete(handlers::sharing_tags::remove_user_sharing_tag),
        )
}
