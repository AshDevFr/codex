//! API v1 Route Definitions
//!
//! This module contains all route definitions for API v1.
//! Each sub-module defines routes for a specific domain.

mod admin;
mod auth;
mod books;
mod libraries;
mod misc;
mod oidc;
mod plugins;
mod recommendations;
mod series;
mod setup;
mod tasks;
mod user;
mod user_plugins;
mod users;

use crate::api::extractors::AppState;
use axum::Router;
use std::sync::Arc;

/// Create the combined API v1 router from all route modules
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Nested routes (with path prefix handled by nest)
        .nest("/auth", auth::routes(state.clone()))
        .nest("/auth/oidc", oidc::routes(state.clone()))
        .nest("/setup", setup::routes(state.clone()))
        // Merged routes (path prefix included in each module)
        .merge(libraries::routes(state.clone()))
        .merge(series::routes(state.clone()))
        .merge(books::routes(state.clone()))
        .merge(users::routes(state.clone()))
        .merge(user::routes(state.clone()))
        .merge(admin::routes(state.clone()))
        .merge(tasks::routes(state.clone()))
        .merge(misc::routes(state.clone()))
        .merge(plugins::routes(state.clone()))
        .merge(user_plugins::routes(state.clone()))
        .merge(recommendations::routes(state.clone()))
        // Apply state to all routes
        .with_state(state)
}
