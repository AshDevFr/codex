//! KOReader sync API route definitions

use crate::api::extractors::AppState;
use crate::api::routes::koreader::handlers;
use axum::{
    Router,
    routing::{get, post, put},
};
use std::sync::Arc;

/// Create the KOReader sync API router
///
/// Mounted at `/koreader` when enabled.
///
/// Routes:
/// - `POST /users/create` - Always 403 (registration via Codex)
/// - `GET /users/auth` - Verify authentication
/// - `GET /syncs/progress/:document` - Get progress by KOReader hash
/// - `PUT /syncs/progress` - Update progress
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // User endpoints
        .route("/users/create", post(handlers::auth::create_user))
        .route("/users/auth", get(handlers::auth::authorize))
        // Sync endpoints
        .route(
            "/syncs/progress/{document}",
            get(handlers::sync::get_progress),
        )
        .route("/syncs/progress", put(handlers::sync::update_progress))
        .with_state(state)
}
