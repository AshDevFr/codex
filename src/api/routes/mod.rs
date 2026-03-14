pub mod komga;
pub mod koreader;
pub mod opds;
pub mod opds2;
pub mod v1;

use crate::api::docs::ApiDoc;
use crate::api::extractors::AppState;
use crate::api::middleware::{RateLimitLayer, create_trace_layer};
use crate::config::Config;
use crate::web;
use axum::{Router, routing::get};
use std::sync::Arc;
use tower_http::catch_panic::CatchPanicLayer;
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
        let komga_path = format!("/{}", config.komga_api.prefix);
        tracing::info!(
            "Komga-compatible API enabled at {}/api/v1 and {}/api/v2 (prefix: {})",
            komga_path,
            komga_path,
            config.komga_api.prefix
        );
        router = router.nest(&komga_path, komga::router(state.clone()));
    }

    // Conditionally mount KOReader sync API if enabled
    if config.koreader_api.enabled {
        tracing::info!("KOReader sync API enabled at /koreader");
        router = router.nest("/koreader", koreader::router(state.clone()));
    }

    // Conditionally mount Scalar API docs if enabled
    if config.api.enable_api_docs {
        tracing::info!("API docs (Scalar) enabled at {}", config.api.api_docs_path);
        // Scalar needs a 'static string, so we leak it
        // This is acceptable since it's created once at server startup
        let api_docs_path: &'static str =
            Box::leak(config.api.api_docs_path.clone().into_boxed_str());

        // Custom HTML template to set the page title to "Codex API" instead of "Scalar"
        let scalar = Scalar::with_url(api_docs_path, ApiDoc::openapi()).custom_html(
            r#"<!doctype html>
<html>
  <head>
    <title>Codex API</title>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
  </head>
  <body>
    <script id="api-reference" type="application/json">
      $spec
    </script>
    <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
  </body>
</html>"#,
        );
        router = router.merge(scalar);
    }

    // Add fallback route for frontend static files (must be last)
    router = router.fallback(get(web::serve_static));

    // Apply rate limiting middleware if enabled
    // Rate limiting is applied before CORS so that:
    // 1. CORS preflight (OPTIONS) requests pass through CORS layer first
    // 2. Rate limit responses get CORS headers added by the CORS layer
    // Note: Static files are exempt via the exempt_paths configuration
    if let Some(rate_limiter) = &state.rate_limiter_service {
        let layer =
            RateLimitLayer::new(rate_limiter.clone(), config.rate_limit.exempt_paths.clone());
        router = router.layer(layer);
        tracing::info!(
            "Rate limiting enabled (exempt paths: {:?})",
            config.rate_limit.exempt_paths
        );
    }

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

    // Catch panics in handlers and return 500 instead of dropping the connection
    router = router.layer(CatchPanicLayer::custom(
        |_err: Box<dyn std::any::Any + Send>| {
            tracing::error!("Handler panicked, returning 500");
            axum::http::Response::builder()
                .status(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(axum::body::Body::from("Internal Server Error"))
                .unwrap()
        },
    ));

    // Add request tracing middleware (outermost layer)
    // This logs all HTTP requests/responses with method, path, status, and latency
    // Logs at debug level for normal requests, error level for 5xx responses
    router = router.layer(create_trace_layer());

    router
}
