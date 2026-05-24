//! Observability routes
//!
//! Routes for the browser RUM bootstrap configuration endpoint and the
//! OTLP/HTTP forwarding proxy. The OTLP routes accept raw bodies (JSON or
//! protobuf) and forward them to the operator-configured upstream
//! collector.

use super::super::handlers;
use crate::extractors::AppState;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};
use std::sync::Arc;

/// 4 MiB upper bound on inbound OTLP POST bodies. Mirrors the default
/// collector receiver limit; the OTLP-JS exporter flushes well below this
/// (default batch hits ~50 KB). Anything above this is almost certainly
/// abuse, so we reject at the body extractor instead of forwarding.
const MAX_PROXY_BODY_BYTES: usize = 4 * 1024 * 1024;

/// Routes:
/// - GET  /observability/config           - Browser SDK bootstrap config
/// - POST /observability/otlp/v1/traces   - Forward traces to upstream OTLP
/// - POST /observability/otlp/v1/metrics  - Forward metrics to upstream OTLP
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/observability/config",
            get(handlers::observability::get_browser_config),
        )
        .route(
            "/observability/otlp/v1/traces",
            post(handlers::observability::proxy_traces)
                .layer(DefaultBodyLimit::max(MAX_PROXY_BODY_BYTES)),
        )
        .route(
            "/observability/otlp/v1/metrics",
            post(handlers::observability::proxy_metrics)
                .layer(DefaultBodyLimit::max(MAX_PROXY_BODY_BYTES)),
        )
}
