use crate::api::{extractors::AuthState, handlers};
use crate::config::ApiConfig;
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

/// Create the main API router with all routes
///
/// Includes health check and all API v1 endpoints
pub fn create_router(state: Arc<AuthState>, api_config: &ApiConfig) -> Router {
    // Clone the database connection for the health check route
    let db_for_health = state.db.clone();

    let mut router = Router::new()
        // Health check (public, no auth)
        .route("/health", get(handlers::health_check))
        .with_state(db_for_health)
        // API v1 routes
        .nest("/api/v1", api_v1_routes(state));

    // Add CORS middleware if enabled
    if api_config.cors_enabled {
        let cors = if api_config.cors_origins.contains(&"*".to_string()) {
            // Allow all origins
            CorsLayer::permissive()
        } else {
            // Allow specific origins
            let origins: Vec<_> = api_config
                .cors_origins
                .iter()
                .filter_map(|o| o.parse().ok())
                .collect();

            CorsLayer::new()
                .allow_origin(origins)
                .allow_methods(Any)
                .allow_headers(Any)
        };

        router = router.layer(cors);
    }

    router
}

/// Create API v1 routes
fn api_v1_routes(state: Arc<AuthState>) -> Router {
    Router::new()
        // Auth routes (public)
        .route("/auth/login", post(handlers::login))
        .route("/auth/logout", post(handlers::logout))
        // Library routes (protected)
        .route("/libraries", get(handlers::list_libraries))
        .route("/libraries", post(handlers::create_library))
        .route("/libraries/:id", get(handlers::get_library))
        .route("/libraries/:id", put(handlers::update_library))
        .route("/libraries/:id", delete(handlers::delete_library))
        // Series routes (protected)
        .route("/series", get(handlers::list_series))
        .route("/series/search", post(handlers::search_series))
        .route("/series/:id", get(handlers::get_series))
        // Book routes (protected)
        .route("/books", get(handlers::list_books))
        .route("/books/:id", get(handlers::get_book))
        // Page routes (protected)
        .route("/books/:book_id/pages/:page_number", get(handlers::get_page_image))
        // User routes (protected, admin only)
        .route("/users", get(handlers::list_users))
        .route("/users", post(handlers::create_user))
        .route("/users/:id", get(handlers::get_user))
        .route("/users/:id", put(handlers::update_user))
        .route("/users/:id", delete(handlers::delete_user))
        // Add state to all routes
        .with_state(state)
}
