use crate::api::{extractors::AppState, handlers};
use crate::config::ApiConfig;
use crate::web;
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

/// Create the main API router with all routes
///
/// Includes health check, OPDS catalog, and all API v1 endpoints
pub fn create_router(state: Arc<AppState>, api_config: &ApiConfig) -> Router {
    // Clone the database connection for the health check route
    let db_for_health = state.db.clone();

    // Clone state for OPDS routes (AuthState is an alias for AppState)
    let opds_state = state.clone();

    let mut router = Router::new()
        // Health check (public, no auth)
        .route("/health", get(handlers::health_check))
        .with_state(db_for_health)
        // OPDS catalog routes (protected with auth)
        .nest("/opds", handlers::opds::opds_routes(opds_state))
        // API v1 routes
        .nest("/api/v1", api_v1_routes(state.clone()))
        // Frontend static files (fallback route - must be last)
        // Serves the React app at the root path
        .fallback(get(web::serve_static));

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
fn api_v1_routes(state: Arc<AppState>) -> Router {
    Router::new()
        // Auth routes (public)
        .route("/auth/login", post(handlers::login))
        .route("/auth/register", post(handlers::register))
        .route("/auth/logout", post(handlers::logout))
        .route("/auth/verify-email", post(handlers::verify_email))
        .route(
            "/auth/resend-verification",
            post(handlers::resend_verification),
        )
        // Library routes (protected)
        .route("/libraries", get(handlers::list_libraries))
        .route("/libraries", post(handlers::create_library))
        .route("/libraries/:id", get(handlers::get_library))
        .route("/libraries/:id", put(handlers::update_library))
        .route("/libraries/:id", delete(handlers::delete_library))
        .route(
            "/libraries/:id/purge-deleted",
            delete(handlers::purge_deleted_books),
        )
        // Scan routes (protected)
        .route("/libraries/:id/scan", post(handlers::trigger_scan))
        .route("/libraries/:id/scan-status", get(handlers::get_scan_status))
        .route("/libraries/:id/scan/cancel", post(handlers::cancel_scan))
        .route("/scans/active", get(handlers::list_active_scans))
        // Series routes (protected)
        .route("/series", get(handlers::list_series))
        .route("/series/search", post(handlers::search_series))
        .route("/series/:id", get(handlers::get_series))
        .route("/series/:id/books", get(handlers::get_series_books))
        .route(
            "/series/:id/purge-deleted",
            delete(handlers::purge_series_deleted_books),
        )
        .route("/series/:id/thumbnail", get(handlers::get_series_thumbnail))
        .route("/series/:id/cover", post(handlers::upload_series_cover))
        .route(
            "/series/:id/cover/source",
            put(handlers::set_series_cover_source),
        )
        // Book routes (protected)
        .route("/books", get(handlers::list_books))
        .route("/books/:id", get(handlers::get_book))
        .route("/books/:id/thumbnail", get(handlers::get_book_thumbnail))
        // Page routes (protected)
        .route(
            "/books/:book_id/pages/:page_number",
            get(handlers::get_page_image),
        )
        // Reading progress routes (protected)
        .route(
            "/books/:book_id/progress",
            put(handlers::update_reading_progress),
        )
        .route(
            "/books/:book_id/progress",
            get(handlers::get_reading_progress),
        )
        .route(
            "/books/:book_id/progress",
            delete(handlers::delete_reading_progress),
        )
        .route("/progress", get(handlers::get_user_progress))
        .route(
            "/progress/currently-reading",
            get(handlers::get_currently_reading),
        )
        // User routes (protected, admin only)
        .route("/users", get(handlers::list_users))
        .route("/users", post(handlers::create_user))
        .route("/users/:id", get(handlers::get_user))
        .route("/users/:id", put(handlers::update_user))
        .route("/users/:id", delete(handlers::delete_user))
        // Metrics routes (protected)
        .route("/metrics", get(handlers::get_metrics))
        // Task management routes (protected)
        .route("/tasks", get(handlers::list_tasks))
        .route("/tasks/:task_id", get(handlers::get_task))
        .route("/tasks/:task_id/cancel", post(handlers::cancel_task))
        // Add state to all routes
        .with_state(state)
}
