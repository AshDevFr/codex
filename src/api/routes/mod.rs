pub mod komga;
pub mod opds;
pub mod opds2;
pub mod v1;

use crate::api::docs::ApiDoc;
use crate::api::extractors::AppState;
use crate::config::Config;
use crate::web;
use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

/// Create the main API router with all routes
///
/// Includes health check, OPDS catalog, API v1 endpoints, and optional Komga-compatible API.
/// The Komga API is mounted at `/{prefix}/api/v1/` when enabled in config.
pub fn create_router(state: Arc<AppState>, config: &Config) -> Router {
    // Clone the database connection for the health check route
    let db_for_health = state.db.clone();

    // Clone state for OPDS routes
    let opds_state = state.clone();
    let opds2_state = state.clone();

    let mut router = Router::new()
        // Health check (public, no auth)
        .route("/health", get(v1::handlers::health_check))
        .with_state(db_for_health)
        // OPDS 1.2 catalog routes (protected with auth) - XML format
        .nest("/opds", opds::router(opds_state))
        // OPDS 2.0 catalog routes (protected with auth) - JSON format
        .nest("/opds/v2", opds2::router(opds2_state))
        // API v1 routes - using modular sub-routers
        .nest("/api/v1", v1::routes(state.clone()));

    // Conditionally mount Komga-compatible API if enabled
    if config.komga_api.enabled {
        let komga_path = format!("/{}/api/v1", config.komga_api.prefix);
        tracing::info!(
            "Komga-compatible API enabled at {} (prefix: {})",
            komga_path,
            config.komga_api.prefix
        );
        router = router.nest(&komga_path, komga::router(state.clone()));
    }

    // Conditionally mount Scalar API docs if enabled
    if config.api.enable_api_docs {
        tracing::info!("API docs (Scalar) enabled at {}", config.api.api_docs_path);
        // Scalar needs a 'static string, so we leak it
        // This is acceptable since it's created once at server startup
        let api_docs_path: &'static str =
            Box::leak(config.api.api_docs_path.clone().into_boxed_str());
        router = router.merge(Scalar::with_url(api_docs_path, ApiDoc::openapi()));
    }

    // Add fallback route for frontend static files (must be last)
    router = router.fallback(get(web::serve_static));

    // Add CORS middleware if enabled
    if config.api.cors_enabled {
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

        let cors = if config.api.cors_origins.contains(&"*".to_string()) {
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
            let origins: Vec<axum::http::HeaderValue> = config
                .api
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
