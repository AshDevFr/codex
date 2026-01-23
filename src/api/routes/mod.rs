use crate::api::{extractors::AppState, handlers};
use crate::config::ApiConfig;
use crate::web;
use axum::{
    routing::{delete, get, patch, post, put},
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
    let opds2_state = state.clone();

    let mut router = Router::new()
        // Health check (public, no auth)
        .route("/health", get(handlers::health_check))
        .with_state(db_for_health)
        // OPDS 1.2 catalog routes (protected with auth) - XML format
        .nest("/opds", handlers::opds::opds_routes(opds_state))
        // OPDS 2.0 catalog routes (protected with auth) - JSON format
        .nest("/opds/v2", handlers::opds2::opds2_routes(opds2_state))
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
        .route("/libraries/preview-scan", post(handlers::preview_scan))
        .route("/libraries/:library_id", get(handlers::get_library))
        .route("/libraries/:library_id", patch(handlers::update_library))
        .route("/libraries/:library_id", delete(handlers::delete_library))
        .route(
            "/libraries/:library_id/purge-deleted",
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
        .route(
            "/libraries/:library_id/books/with-errors",
            get(handlers::list_library_books_with_errors),
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
        .route(
            "/libraries/:library_id/series/recently-added",
            get(handlers::list_library_recently_added_series),
        )
        .route(
            "/libraries/:library_id/series/recently-updated",
            get(handlers::list_library_recently_updated_series),
        )
        .route(
            "/libraries/:library_id/books/recently-read",
            get(handlers::list_library_recently_read_books),
        )
        // Scan routes (protected)
        .route("/libraries/:library_id/scan", post(handlers::trigger_scan))
        .route(
            "/libraries/:library_id/scan-status",
            get(handlers::get_scan_status),
        )
        .route(
            "/libraries/:library_id/scan/cancel",
            post(handlers::cancel_scan),
        )
        .route(
            "/libraries/:library_id/analyze",
            post(handlers::trigger_library_analysis),
        )
        .route(
            "/libraries/:library_id/analyze-unanalyzed",
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
        .route("/series/list", post(handlers::list_series_filtered))
        .route(
            "/series/list/alphabetical-groups",
            post(handlers::list_series_alphabetical_groups),
        )
        .route("/series/:series_id", get(handlers::get_series))
        .route("/series/:series_id", patch(handlers::patch_series))
        .route("/series/:series_id/books", get(handlers::get_series_books))
        .route(
            "/series/:series_id/books/with-errors",
            get(handlers::list_series_books_with_errors),
        )
        // Series collection routes (protected)
        .route(
            "/series/in-progress",
            get(handlers::list_in_progress_series),
        )
        .route(
            "/series/recently-added",
            get(handlers::list_recently_added_series),
        )
        .route(
            "/series/recently-updated",
            get(handlers::list_recently_updated_series),
        )
        .route(
            "/series/:series_id/purge-deleted",
            delete(handlers::purge_series_deleted_books),
        )
        .route(
            "/series/:series_id/thumbnail",
            get(handlers::get_series_thumbnail),
        )
        .route(
            "/series/:series_id/cover",
            post(handlers::upload_series_cover),
        )
        .route(
            "/series/:series_id/cover/source",
            patch(handlers::set_series_cover_source),
        )
        // Series covers routes (multi-cover management)
        .route(
            "/series/:series_id/covers",
            get(handlers::list_series_covers),
        )
        .route(
            "/series/:series_id/covers/selected",
            delete(handlers::reset_series_cover),
        )
        .route(
            "/series/:series_id/covers/:cover_id/select",
            put(handlers::select_series_cover),
        )
        .route(
            "/series/:series_id/covers/:cover_id/image",
            get(handlers::get_series_cover_image),
        )
        .route(
            "/series/:series_id/covers/:cover_id",
            delete(handlers::delete_series_cover),
        )
        .route(
            "/series/:series_id/analyze",
            post(handlers::trigger_series_analysis),
        )
        .route(
            "/series/:series_id/analyze-unanalyzed",
            post(handlers::trigger_series_unanalyzed_analysis),
        )
        .route(
            "/series/:series_id/download",
            get(handlers::download_series),
        )
        // Series metadata routes (protected)
        .route(
            "/series/:series_id/metadata",
            put(handlers::replace_series_metadata),
        )
        .route(
            "/series/:series_id/metadata",
            patch(handlers::patch_series_metadata),
        )
        .route(
            "/series/:series_id/metadata/full",
            get(handlers::get_full_series_metadata),
        )
        .route(
            "/series/:series_id/metadata/locks",
            get(handlers::get_metadata_locks),
        )
        .route(
            "/series/:series_id/metadata/locks",
            put(handlers::update_metadata_locks),
        )
        // Series genres routes (protected)
        .route(
            "/series/:series_id/genres",
            get(handlers::get_series_genres),
        )
        .route(
            "/series/:series_id/genres",
            put(handlers::set_series_genres),
        )
        .route(
            "/series/:series_id/genres",
            post(handlers::add_series_genre),
        )
        .route(
            "/series/:series_id/genres/:genre_id",
            delete(handlers::remove_series_genre),
        )
        // Series tags routes (protected)
        .route("/series/:series_id/tags", get(handlers::get_series_tags))
        .route("/series/:series_id/tags", put(handlers::set_series_tags))
        .route("/series/:series_id/tags", post(handlers::add_series_tag))
        .route(
            "/series/:series_id/tags/:tag_id",
            delete(handlers::remove_series_tag),
        )
        // Series sharing tags routes (protected, admin only)
        .route(
            "/series/:series_id/sharing-tags",
            get(handlers::sharing_tags::get_series_sharing_tags),
        )
        .route(
            "/series/:series_id/sharing-tags",
            put(handlers::sharing_tags::set_series_sharing_tags),
        )
        .route(
            "/series/:series_id/sharing-tags",
            post(handlers::sharing_tags::add_series_sharing_tag),
        )
        .route(
            "/series/:series_id/sharing-tags/:tag_id",
            delete(handlers::sharing_tags::remove_series_sharing_tag),
        )
        // Series user rating routes (protected)
        .route(
            "/series/:series_id/rating",
            get(handlers::get_series_rating),
        )
        .route(
            "/series/:series_id/rating",
            put(handlers::set_series_rating),
        )
        .route(
            "/series/:series_id/rating",
            delete(handlers::delete_series_rating),
        )
        // Series alternate titles routes (protected)
        .route(
            "/series/:series_id/alternate-titles",
            get(handlers::get_series_alternate_titles),
        )
        .route(
            "/series/:series_id/alternate-titles",
            post(handlers::create_alternate_title),
        )
        .route(
            "/series/:series_id/alternate-titles/:title_id",
            patch(handlers::update_alternate_title),
        )
        .route(
            "/series/:series_id/alternate-titles/:title_id",
            delete(handlers::delete_alternate_title),
        )
        // Series external ratings routes (protected)
        .route(
            "/series/:series_id/external-ratings",
            get(handlers::get_series_external_ratings),
        )
        .route(
            "/series/:series_id/external-ratings",
            post(handlers::create_external_rating),
        )
        .route(
            "/series/:series_id/external-ratings/:source",
            delete(handlers::delete_external_rating),
        )
        // Series average rating route (protected)
        .route(
            "/series/:series_id/ratings/average",
            get(handlers::get_series_average_rating),
        )
        // Series external links routes (protected)
        .route(
            "/series/:series_id/external-links",
            get(handlers::get_series_external_links),
        )
        .route(
            "/series/:series_id/external-links",
            post(handlers::create_external_link),
        )
        .route(
            "/series/:series_id/external-links/:source",
            delete(handlers::delete_external_link),
        )
        // Global genre routes (protected, cleanup/delete require admin)
        .route("/genres", get(handlers::list_genres))
        .route("/genres/cleanup", post(handlers::cleanup_genres))
        .route("/genres/:genre_id", delete(handlers::delete_genre))
        // Global tag routes (protected, cleanup/delete require admin)
        .route("/tags", get(handlers::list_tags))
        .route("/tags/cleanup", post(handlers::cleanup_tags))
        .route("/tags/:tag_id", delete(handlers::delete_tag))
        // User ratings routes (protected)
        .route("/user/ratings", get(handlers::list_user_ratings))
        // Current user's sharing tags route (protected)
        .route(
            "/user/sharing-tags",
            get(handlers::sharing_tags::get_my_sharing_tags),
        )
        // User preferences routes (protected)
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
        // User integrations routes (protected)
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
        // Book routes (protected)
        .route("/books", get(handlers::list_books))
        .route("/books/list", post(handlers::list_books_filtered))
        .route("/books/:book_id", get(handlers::get_book))
        .route("/books/:book_id", patch(handlers::patch_book))
        .route(
            "/books/:book_id/adjacent",
            get(handlers::get_adjacent_books),
        )
        .route("/books/:book_id/file", get(handlers::get_book_file))
        .route(
            "/books/:book_id/thumbnail",
            get(handlers::get_book_thumbnail),
        )
        .route("/books/:book_id/cover", post(handlers::upload_book_cover))
        .route(
            "/books/:book_id/analyze",
            post(handlers::trigger_book_analysis),
        )
        .route(
            "/books/:book_id/analyze-unanalyzed",
            post(handlers::trigger_book_unanalyzed_analysis),
        )
        // Book metadata routes (protected)
        .route(
            "/books/:book_id/metadata",
            put(handlers::replace_book_metadata),
        )
        .route(
            "/books/:book_id/metadata",
            patch(handlers::patch_book_metadata),
        )
        // Book metadata lock routes (protected)
        .route(
            "/books/:book_id/metadata/locks",
            get(handlers::get_book_metadata_locks),
        )
        .route(
            "/books/:book_id/metadata/locks",
            put(handlers::update_book_metadata_locks),
        )
        // Book collection routes (protected)
        .route("/books/in-progress", get(handlers::list_in_progress_books))
        .route("/books/on-deck", get(handlers::list_on_deck_books))
        .route(
            "/books/recently-added",
            get(handlers::list_recently_added_books),
        )
        .route(
            "/books/recently-read",
            get(handlers::list_recently_read_books),
        )
        .route("/books/with-errors", get(handlers::list_books_with_errors))
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
        .route(
            "/series/:series_id/read",
            post(handlers::mark_series_as_read),
        )
        .route(
            "/series/:series_id/unread",
            post(handlers::mark_series_as_unread),
        )
        // User routes (protected, admin only)
        .route("/users", get(handlers::list_users))
        .route("/users", post(handlers::create_user))
        .route("/users/:user_id", get(handlers::get_user))
        .route("/users/:user_id", patch(handlers::update_user))
        .route("/users/:user_id", delete(handlers::delete_user))
        // User sharing tags routes (protected, admin only)
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
        // API key routes (protected)
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
        // Metrics routes (protected)
        .route("/metrics/inventory", get(handlers::get_inventory_metrics))
        .route(
            "/metrics/tasks",
            get(handlers::task_metrics::get_task_metrics),
        )
        .route(
            "/metrics/tasks",
            delete(handlers::task_metrics::nuke_task_metrics),
        )
        .route(
            "/metrics/tasks/history",
            get(handlers::task_metrics::get_task_metrics_history),
        )
        .route(
            "/metrics/tasks/cleanup",
            post(handlers::task_metrics::trigger_metrics_cleanup),
        )
        // Task Queue routes (protected) - distributed task queue
        .route("/tasks", get(handlers::task_queue::list_tasks))
        .route("/tasks", post(handlers::task_queue::create_task))
        .route("/tasks/:task_id", get(handlers::task_queue::get_task))
        .route(
            "/tasks/:task_id/cancel",
            post(handlers::task_queue::cancel_task),
        )
        .route(
            "/tasks/:task_id/unlock",
            post(handlers::task_queue::unlock_task),
        )
        .route(
            "/tasks/:task_id/retry",
            post(handlers::task_queue::retry_task),
        )
        .route("/tasks/stats", get(handlers::task_queue::get_task_stats))
        .route(
            "/tasks/purge",
            delete(handlers::task_queue::purge_old_tasks),
        )
        .route("/tasks/nuke", delete(handlers::task_queue::nuke_all_tasks))
        // Thumbnail generation routes (protected)
        .route(
            "/thumbnails/generate",
            post(handlers::task_queue::generate_thumbnails),
        )
        .route(
            "/libraries/:library_id/thumbnails/generate",
            post(handlers::task_queue::generate_library_thumbnails),
        )
        .route(
            "/series/:series_id/thumbnails/generate",
            post(handlers::task_queue::generate_series_thumbnails),
        )
        .route(
            "/books/:book_id/thumbnail/generate",
            post(handlers::task_queue::generate_book_thumbnail),
        )
        // Duplicate detection routes (protected)
        .route("/duplicates", get(handlers::list_duplicates))
        .route("/duplicates/scan", post(handlers::trigger_duplicate_scan))
        .route(
            "/duplicates/:duplicate_id",
            delete(handlers::delete_duplicate_group),
        )
        // Filesystem routes (protected, admin only)
        .route("/filesystem/browse", get(handlers::browse_filesystem))
        .route("/filesystem/drives", get(handlers::list_drives))
        // Branding settings route (public, no authentication required)
        .route(
            "/settings/branding",
            get(handlers::settings::get_branding_settings),
        )
        // Public settings routes (protected, all authenticated users)
        .route(
            "/settings/public",
            get(handlers::settings::get_public_settings),
        )
        // Settings routes (protected, admin only)
        .route("/admin/settings", get(handlers::settings::list_settings))
        .route(
            "/admin/settings/:setting_key",
            get(handlers::settings::get_setting),
        )
        .route(
            "/admin/settings/:setting_key",
            put(handlers::settings::update_setting),
        )
        .route(
            "/admin/settings/bulk",
            post(handlers::settings::bulk_update_settings),
        )
        .route(
            "/admin/settings/:setting_key/reset",
            post(handlers::settings::reset_setting),
        )
        .route(
            "/admin/settings/:setting_key/history",
            get(handlers::settings::get_setting_history),
        )
        // System integrations routes (protected, admin only)
        .route(
            "/admin/integrations",
            get(handlers::system_integrations::list_system_integrations),
        )
        .route(
            "/admin/integrations",
            post(handlers::system_integrations::create_system_integration),
        )
        .route(
            "/admin/integrations/:id",
            get(handlers::system_integrations::get_system_integration),
        )
        .route(
            "/admin/integrations/:id",
            patch(handlers::system_integrations::update_system_integration),
        )
        .route(
            "/admin/integrations/:id",
            delete(handlers::system_integrations::delete_system_integration),
        )
        .route(
            "/admin/integrations/:id/enable",
            post(handlers::system_integrations::enable_system_integration),
        )
        .route(
            "/admin/integrations/:id/disable",
            post(handlers::system_integrations::disable_system_integration),
        )
        .route(
            "/admin/integrations/:id/test",
            post(handlers::system_integrations::test_system_integration),
        )
        // Sharing tags routes (protected, admin only)
        .route(
            "/admin/sharing-tags",
            get(handlers::sharing_tags::list_sharing_tags),
        )
        .route(
            "/admin/sharing-tags",
            post(handlers::sharing_tags::create_sharing_tag),
        )
        .route(
            "/admin/sharing-tags/:tag_id",
            get(handlers::sharing_tags::get_sharing_tag),
        )
        .route(
            "/admin/sharing-tags/:tag_id",
            patch(handlers::sharing_tags::update_sharing_tag),
        )
        .route(
            "/admin/sharing-tags/:tag_id",
            delete(handlers::sharing_tags::delete_sharing_tag),
        )
        // Cleanup routes (protected, admin only)
        .route(
            "/admin/cleanup-orphans/stats",
            get(handlers::cleanup::get_orphan_stats),
        )
        .route(
            "/admin/cleanup-orphans",
            post(handlers::cleanup::trigger_cleanup),
        )
        .route(
            "/admin/cleanup-orphans",
            delete(handlers::cleanup::delete_orphans),
        )
        // PDF cache management routes (protected, admin only)
        .route(
            "/admin/pdf-cache/stats",
            get(handlers::pdf_cache::get_pdf_cache_stats),
        )
        .route(
            "/admin/pdf-cache/cleanup",
            post(handlers::pdf_cache::trigger_pdf_cache_cleanup),
        )
        .route(
            "/admin/pdf-cache",
            delete(handlers::pdf_cache::clear_pdf_cache),
        )
        // Add state to all routes
        .with_state(state)
}
