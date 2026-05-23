//! Observability DTOs
//!
//! Describes the configuration the browser-side OpenTelemetry SDK needs to
//! bootstrap itself. Secrets (collector auth headers, endpoint hostnames)
//! stay server-side — this payload only carries enough info for the SDK to
//! decide whether to start and where on the Codex origin to POST batches.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Browser RUM bootstrap configuration returned by
/// `GET /api/v1/observability/config`.
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BrowserObservabilityConfigDto {
    /// Whether the browser SDK should initialize. False means the SDK
    /// bootstrap is a no-op even if the script is loaded.
    pub enabled: bool,

    /// `service.name` resource attribute the browser SDK should set on
    /// every span (matches the backend service name unless the operator
    /// overrode it specifically for the browser).
    #[schema(example = "codex-web")]
    pub service_name: String,

    /// Same-origin path prefix on the Codex server where the browser SDK
    /// should POST OTLP batches. The SDK appends `/v1/traces` and
    /// `/v1/metrics` to this base.
    #[schema(example = "/api/v1/observability/otlp")]
    pub proxy_path: String,

    /// Parent-based sampling ratio applied client-side. Browsers are noisy;
    /// default low.
    #[schema(example = 0.1)]
    pub sample_ratio: f64,
}
