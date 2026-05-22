//! Axum HTTP integration for OpenTelemetry.
//!
//! Wraps the `axum-tracing-opentelemetry` layers (which create the server
//! span from incoming `traceparent` and inject the active trace context into
//! responses) in a single helper that becomes a no-op when the `observability`
//! feature is off or when `observability.enabled` is false.

use axum::Router;

use crate::config::ObservabilityConfig;

/// Apply the HTTP server-side OTel layers to the given router.
///
/// Layered outside any rate limiter / CORS / panic-catch so every request
/// gets a server span before downstream middleware runs.
#[cfg(feature = "observability")]
pub fn install_http_layers(router: Router, config: &ObservabilityConfig) -> Router {
    if !config.enabled || !config.traces.enabled || config.otlp.endpoint.trim().is_empty() {
        // Nothing to do: either observability is off globally, traces are off,
        // or the endpoint is unset (init() already logged the warning).
        return router;
    }
    router
        .layer(axum_tracing_opentelemetry::middleware::OtelInResponseLayer)
        .layer(axum_tracing_opentelemetry::middleware::OtelAxumLayer::default())
}

#[cfg(not(feature = "observability"))]
pub fn install_http_layers(router: Router, _config: &ObservabilityConfig) -> Router {
    router
}
