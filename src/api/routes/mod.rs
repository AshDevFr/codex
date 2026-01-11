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
        // When allow_credentials is true, we cannot use wildcard (*) for headers or methods
        // Must specify exact headers and methods that are allowed
        use axum::http::Method;
        use tower_http::cors::{AllowHeaders, AllowMethods};

        // Define allowed HTTP methods used by the API
        let allowed_methods = vec![
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS, // Required for CORS preflight requests
        ];

        let cors = if api_config.cors_origins.contains(&"*".to_string()) {
            // Allow all origins (wildcard)
            // NOTE: Cannot use allow_credentials(true) with wildcard origin (*)
            // This is a CORS security restriction. If you need credentials (cookies),
            // you must specify explicit origins instead of using "*"
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(AllowMethods::list(allowed_methods.clone()))
                .allow_headers(AllowHeaders::list([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::ACCEPT,
                ]))
            // Cannot allow credentials with wildcard origin
        } else {
            // Allow specific origins
            let origins: Vec<_> = api_config
                .cors_origins
                .iter()
                .filter_map(|o| o.parse().ok())
                .collect();

            CorsLayer::new()
                .allow_origin(origins)
                .allow_methods(AllowMethods::list(allowed_methods))
                .allow_headers(AllowHeaders::list([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::ACCEPT,
                ]))
                .allow_credentials(true) // IMPORTANT: Required for cookie-based auth
        };

        router = router.layer(cors);
    }

    router
}

/// Create API v1 routes
fn api_v1_routes(state: Arc<AppState>) -> Router {
    Router::new()
        // Setup routes (public - only work when no users exist)
        .route("/setup/status", get(handlers::setup::setup_status))
        .route("/setup/initialize", post(handlers::setup::initialize_setup))
        .route(
            "/setup/settings",
            axum::routing::patch(handlers::setup::configure_initial_settings),
        )
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
        // Library-specific book routes (protected)
        .route(
            "/libraries/:library_id/books",
            get(handlers::list_library_books),
        )
        .route(
            "/libraries/:library_id/books/in-progress",
            get(handlers::list_library_in_progress_books),
        )
        .route(
            "/libraries/:library_id/books/recently-added",
            get(handlers::list_library_recently_added_books),
        )
        .route(
            "/libraries/:library_id/books/on-deck",
            get(handlers::list_library_on_deck_books),
        )
        // Library-specific series routes (protected)
        .route(
            "/libraries/:library_id/series",
            get(handlers::list_library_series),
        )
        .route(
            "/libraries/:library_id/series/in-progress",
            get(handlers::list_library_in_progress_series),
        )
        // Scan routes (protected)
        .route("/libraries/:id/scan", post(handlers::trigger_scan))
        .route("/libraries/:id/scan-status", get(handlers::get_scan_status))
        .route("/libraries/:id/scan/cancel", post(handlers::cancel_scan))
        .route(
            "/libraries/:id/analyze",
            post(handlers::trigger_library_analysis),
        )
        .route(
            "/libraries/:id/analyze-unanalyzed",
            post(handlers::trigger_library_unanalyzed_analysis),
        )
        .route("/scans/active", get(handlers::list_active_scans))
        .route("/scans/stream", get(handlers::scan_progress_stream))
        // Real-time event routes (protected)
        .route("/events/stream", get(handlers::entity_events_stream))
        .route("/tasks/stream", get(handlers::task_progress_stream))
        // Series routes (protected)
        .route("/series", get(handlers::list_series))
        .route("/series/search", post(handlers::search_series))
        .route("/series/:id", get(handlers::get_series))
        .route("/series/:id/books", get(handlers::get_series_books))
        // Series collection routes (protected)
        .route("/series/in-progress", get(handlers::list_in_progress_series))
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
        .route(
            "/series/:id/analyze",
            post(handlers::trigger_series_analysis),
        )
        .route(
            "/series/:id/analyze-unanalyzed",
            post(handlers::trigger_series_unanalyzed_analysis),
        )
        // Book routes (protected)
        .route("/books", get(handlers::list_books))
        .route("/books/:id", get(handlers::get_book))
        .route("/books/:id/thumbnail", get(handlers::get_book_thumbnail))
        .route("/books/:id/analyze", post(handlers::trigger_book_analysis))
        .route(
            "/books/:id/analyze-unanalyzed",
            post(handlers::trigger_book_unanalyzed_analysis),
        )
        // Book collection routes (protected)
        .route("/books/in-progress", get(handlers::list_in_progress_books))
        .route("/books/on-deck", get(handlers::list_on_deck_books))
        .route(
            "/books/recently-added",
            get(handlers::list_recently_added_books),
        )
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
        // Mark as read/unread routes (protected)
        .route("/books/:book_id/read", post(handlers::mark_book_as_read))
        .route(
            "/books/:book_id/unread",
            post(handlers::mark_book_as_unread),
        )
        .route("/series/:id/read", post(handlers::mark_series_as_read))
        .route("/series/:id/unread", post(handlers::mark_series_as_unread))
        // User routes (protected, admin only)
        .route("/users", get(handlers::list_users))
        .route("/users", post(handlers::create_user))
        .route("/users/:id", get(handlers::get_user))
        .route("/users/:id", put(handlers::update_user))
        .route("/users/:id", delete(handlers::delete_user))
        // API key routes (protected)
        .route("/api-keys", get(handlers::api_keys::list_api_keys))
        .route("/api-keys", post(handlers::api_keys::create_api_key))
        .route("/api-keys/:id", get(handlers::api_keys::get_api_key))
        .route("/api-keys/:id", put(handlers::api_keys::update_api_key))
        .route("/api-keys/:id", delete(handlers::api_keys::delete_api_key))
        // Metrics routes (protected)
        .route("/metrics", get(handlers::get_metrics))
        // Task Queue routes (protected) - distributed task queue
        .route("/tasks", get(handlers::task_queue::list_tasks))
        .route("/tasks", post(handlers::task_queue::create_task))
        .route("/tasks/:id", get(handlers::task_queue::get_task))
        .route("/tasks/:id/cancel", post(handlers::task_queue::cancel_task))
        .route("/tasks/:id/unlock", post(handlers::task_queue::unlock_task))
        .route("/tasks/:id/retry", post(handlers::task_queue::retry_task))
        .route("/tasks/stats", get(handlers::task_queue::get_task_stats))
        .route(
            "/tasks/purge",
            delete(handlers::task_queue::purge_old_tasks),
        )
        .route("/tasks/nuke", delete(handlers::task_queue::nuke_all_tasks))
        // Duplicate detection routes (protected)
        .route("/duplicates", get(handlers::list_duplicates))
        .route("/duplicates/scan", post(handlers::trigger_duplicate_scan))
        .route("/duplicates/:id", delete(handlers::delete_duplicate_group))
        // Filesystem routes (protected, admin only)
        .route("/filesystem/browse", get(handlers::browse_filesystem))
        .route("/filesystem/drives", get(handlers::list_drives))
        // Settings routes (protected, admin only)
        .route("/admin/settings", get(handlers::settings::list_settings))
        .route("/admin/settings/:key", get(handlers::settings::get_setting))
        .route(
            "/admin/settings/:key",
            put(handlers::settings::update_setting),
        )
        .route(
            "/admin/settings/bulk",
            post(handlers::settings::bulk_update_settings),
        )
        .route(
            "/admin/settings/:key/reset",
            post(handlers::settings::reset_setting),
        )
        .route(
            "/admin/settings/:key/history",
            get(handlers::settings::get_setting_history),
        )
        // Add state to all routes
        .with_state(state)
}
