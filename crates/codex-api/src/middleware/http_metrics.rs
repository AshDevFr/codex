//! HTTP request metrics middleware.
//!
//! Emits an OTel histogram measurement (`http.server.request.duration` in
//! seconds) with `method`, `route`, and `status_code` attributes for every
//! HTTP request. The route comes from Axum's `MatchedPath` extractor so the
//! attribute carries the template (`/api/v1/series/:id`) rather than the
//! resolved URL — otherwise cardinality would explode per series ID.
//!
//! Layered alongside the existing `axum-tracing-opentelemetry` span layers;
//! that crate focuses on spans, this layer focuses on metrics.

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;

/// Record request duration after the inner service responds.
pub async fn http_metrics_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "unmatched".to_string());

    let start = Instant::now();
    let response = next.run(request).await;
    let elapsed = start.elapsed().as_secs_f64();

    crate::observability::metrics::record_http_request(
        method.as_str(),
        &route,
        response.status().as_u16(),
        elapsed,
    );

    response
}
