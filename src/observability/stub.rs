//! No-op stubs used when the `observability` feature is disabled.

use anyhow::Result;
use std::fmt;
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    fmt::{
        FmtContext, FormatEvent, FormatFields,
        format::{Format, Writer},
    },
    registry::LookupSpan,
};

use crate::config::ObservabilityConfig;

/// Empty handle. All accessors return as if observability is disabled.
pub struct ObservabilityHandle;

impl ObservabilityHandle {
    pub fn traces_enabled(&self) -> bool {
        false
    }
    pub fn metrics_enabled(&self) -> bool {
        false
    }
    pub fn shutdown(self) {}
}

/// Init is a no-op when the feature is off.
///
/// Logs a hint at info level if the operator asked for observability so they
/// realize the binary was built without the feature.
pub fn init(config: &ObservabilityConfig) -> Result<ObservabilityHandle> {
    if config.enabled {
        tracing::info!(
            "observability.enabled = true but binary was built without the `observability` feature; ignoring"
        );
    }
    Ok(ObservabilityHandle)
}

/// Identity formatter that delegates to the inner formatter unchanged.
///
/// Mirrors the real `TraceContextFormat` so [`crate::commands::common::init_tracing`]
/// can use the same type name regardless of feature state.
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
        writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        self.inner.format_event(ctx, writer, event)
    }
}
