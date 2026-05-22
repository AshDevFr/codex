//! OpenTelemetry instrumentation glue.
//!
//! Gated by the `observability` Cargo feature. When the feature is enabled
//! and `ObservabilityConfig::enabled` is true, [`init`] starts an OTLP tracer
//! and meter provider, wires them into the OTel globals, and returns a guard
//! that owns the providers for shutdown.
//!
//! When the feature is disabled (or `enabled` is false), every entry point is
//! a no-op so the rest of the codebase can stay cfg-free at call sites.

#[cfg(feature = "observability")]
mod providers;
#[cfg(feature = "observability")]
mod trace_fmt;

#[cfg(not(feature = "observability"))]
mod stub;

#[cfg(feature = "observability")]
pub use providers::{ObservabilityHandle, init};

#[cfg(feature = "observability")]
pub use trace_fmt::TraceContextFormat;

#[cfg(not(feature = "observability"))]
pub use stub::{ObservabilityHandle, TraceContextFormat, init};

mod http;
pub use http::install_http_layers;
