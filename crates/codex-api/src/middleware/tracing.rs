//! HTTP request/response tracing middleware
//!
//! Provides request logging using tower-http's TraceLayer with customized
//! span creation and response classification.
//!
//! Each request gets an `info`-level span carrying method, URI, and HTTP
//! version. The response boundary emits exactly one event per request: `info`
//! for 2xx/3xx, `debug` for 4xx (client errors are noisy and already detailed
//! in `ApiError`), `error` for 5xx. Request entry is logged at `debug` for
//! operators who want full lifecycle detail.

use axum::http::{Request, Response};
use std::time::Duration;
use tower_http::{
    classify::{ServerErrorsAsFailures, SharedClassifier},
    trace::{MakeSpan, TraceLayer},
};
use tracing::Span;

/// Custom span maker that creates spans with request information
#[derive(Clone, Debug)]
pub struct RequestSpan;

impl<B> MakeSpan<B> for RequestSpan {
    fn make_span(&mut self, request: &Request<B>) -> Span {
        // `info_span!` so the span (and its method/uri fields) is enabled at
        // the default log level. Child events emitted in this span inherit
        // these fields.
        tracing::info_span!(
            "http_request",
            method = %request.method(),
            uri = %request.uri().path(),
            version = ?request.version(),
        )
    }
}

/// Custom response handler that logs based on status code
#[derive(Clone, Debug)]
pub struct ResponseLogger;

impl<B> tower_http::trace::OnResponse<B> for ResponseLogger {
    fn on_response(self, response: &Response<B>, latency: Duration, _span: &Span) {
        let status = response.status().as_u16();
        let latency_ms = latency.as_millis();

        // One event per request at a level appropriate to the outcome.
        match status {
            // 2xx - Success. Visible at default `info` level.
            200..=299 => {
                tracing::info!(status = status, latency_ms = latency_ms, "Response sent");
            }
            // 3xx - Redirects. Visible at default `info` level.
            300..=399 => {
                tracing::info!(
                    status = status,
                    latency_ms = latency_ms,
                    "Redirect response"
                );
            }
            // 4xx - Client errors. Details are already logged by ApiError; keep
            // this at debug to avoid noise from probing clients.
            400..=499 => {
                tracing::debug!(
                    status = status,
                    latency_ms = latency_ms,
                    "Client error response"
                );
            }
            // 5xx - Server errors (always visible).
            500..=599 => {
                tracing::error!(
                    status = status,
                    latency_ms = latency_ms,
                    "Server error response"
                );
            }
            _ => {
                tracing::warn!(
                    status = status,
                    latency_ms = latency_ms,
                    "Unknown status code"
                );
            }
        }
    }
}

/// Custom request handler that logs incoming requests
#[derive(Clone, Debug)]
pub struct RequestLogger;

impl<B> tower_http::trace::OnRequest<B> for RequestLogger {
    fn on_request(&mut self, request: &Request<B>, _span: &Span) {
        // Detail event; the response boundary is the user-facing log.
        tracing::debug!(
            method = %request.method(),
            uri = %request.uri(),
            "Request received"
        );
    }
}

/// Create a configured TraceLayer for HTTP request/response logging
///
/// This layer:
/// - Creates an `info`-level span with method, URI, and HTTP version
/// - Logs a `debug` event when each request is received
/// - Logs exactly one boundary event per response: `info` for 2xx/3xx,
///   `debug` for 4xx, `error` for 5xx
pub fn create_trace_layer()
-> TraceLayer<SharedClassifier<ServerErrorsAsFailures>, RequestSpan, RequestLogger, ResponseLogger>
{
    TraceLayer::new_for_http()
        .make_span_with(RequestSpan)
        .on_request(RequestLogger)
        .on_response(ResponseLogger)
}
