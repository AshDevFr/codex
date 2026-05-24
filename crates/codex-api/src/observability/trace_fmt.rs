//! `tracing_subscriber::fmt::FormatEvent` wrapper that prepends the active
//! OpenTelemetry trace and span IDs to every emitted log line.
//!
//! Combined with the `tracing-opentelemetry` layer this makes log → trace
//! correlation a single grep away: the trace_id in a log line is the same
//! one the OTLP backend stores against the span tree.

use std::fmt;

use opentelemetry::trace::TraceContextExt;
use tracing::{Event, Span, Subscriber};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{
    fmt::{
        FmtContext, FormatEvent, FormatFields,
        format::{Format, Writer},
    },
    registry::LookupSpan,
};

/// Wraps the default fmt event formatter to prepend `trace_id` / `span_id`.
///
/// Reads the OTel context from `tracing::Span::current()` so this works for
/// any event emitted inside an active span carrying OTel context (e.g.,
/// anything inside the HTTP request span installed by the OTel axum layer).
pub struct TraceContextFormat<F = Format> {
    inner: F,
}

impl<F> TraceContextFormat<F> {
    pub fn new(inner: F) -> Self {
        Self { inner }
    }
}

impl Default for TraceContextFormat<Format> {
    fn default() -> Self {
        Self::new(Format::default())
    }
}

impl<S, N, F> FormatEvent<S, N> for TraceContextFormat<F>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
    F: FormatEvent<S, N>,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let span = Span::current();
        let otel_ctx = span.context();
        let otel_span = otel_ctx.span();
        let span_ctx = otel_span.span_context();
        if span_ctx.is_valid() {
            write!(
                writer,
                "trace_id={} span_id={} ",
                span_ctx.trace_id(),
                span_ctx.span_id()
            )?;
        }
        self.inner.format_event(ctx, writer, event)
    }
}
