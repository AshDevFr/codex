//! HTTP request/response tracing middleware
//!
//! Provides request logging using tower-http's TraceLayer with customized
//! span creation and response classification for better observability.

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
        // Create a span with useful request information
        // Using debug_span so these only show up at debug level
        tracing::debug_span!(
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

        // Log at different levels based on status code
        match status {
            // 2xx - Success (debug level, these are normal)
            200..=299 => {
                tracing::debug!(status = status, latency_ms = latency_ms, "Response sent");
            }
            // 3xx - Redirects (debug level)
            300..=399 => {
                tracing::debug!(
                    status = status,
                    latency_ms = latency_ms,
                    "Redirect response"
                );
            }
            // 4xx - Client errors (debug level - details logged in ApiError)
            400..=499 => {
                tracing::debug!(
                    status = status,
                    latency_ms = latency_ms,
                    "Client error response"
                );
            }
            // 5xx - Server errors (error level)
            500..=599 => {
                tracing::error!(
                    status = status,
                    latency_ms = latency_ms,
                    "Server error response"
                );
            }
            // Unknown status codes
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
/// - Creates spans with request method, URI, and version
/// - Logs incoming requests at debug level
/// - Logs responses with status code and latency
/// - Uses appropriate log levels based on response status
pub fn create_trace_layer(
) -> TraceLayer<SharedClassifier<ServerErrorsAsFailures>, RequestSpan, RequestLogger, ResponseLogger>
{
    TraceLayer::new_for_http()
        .make_span_with(RequestSpan)
        .on_request(RequestLogger)
        .on_response(ResponseLogger)
}
