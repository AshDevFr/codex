//! Browser RUM bootstrap + OTLP forwarding proxy handlers.
//!
//! The browser SDK runs server-side configuration on startup
//! ([`get_browser_config`]) and then POSTs OTLP/HTTP batches to
//! [`proxy_traces`] / [`proxy_metrics`]. The proxy forwards the body
//! verbatim to the operator-configured upstream collector with the
//! operator-configured headers attached, avoiding CORS hops and keeping
//! collector auth tokens out of the browser.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use tokio::sync::OnceCell;

use crate::{
    error::ApiError,
    extractors::{AppState, FlexibleAuthContext},
};
use codex_config::ObservabilityConfig;

use super::super::dto::BrowserObservabilityConfigDto;

/// Maximum accepted body size for a single OTLP POST. 4 MiB matches the
/// default Collector grpc/HTTP receiver limit and is well above any
/// reasonable browser batch (default batch flushes at 512 spans, ~50 KB).
const MAX_OTLP_BODY_BYTES: usize = 4 * 1024 * 1024;

/// Reusable HTTP client for the upstream OTLP forward.
///
/// Built lazily on first use so the timeout matches whatever
/// `observability.otlp.timeout_ms` was configured at startup. A single
/// client serves every forward — its connection pool is the reason we
/// don't construct one per request.
static UPSTREAM_CLIENT: OnceCell<reqwest::Client> = OnceCell::const_new();

async fn upstream_client(
    config: &ObservabilityConfig,
) -> Result<&'static reqwest::Client, ApiError> {
    UPSTREAM_CLIENT
        .get_or_try_init(|| async {
            reqwest::Client::builder()
                .timeout(Duration::from_millis(config.otlp.timeout_ms))
                .build()
                .map_err(|e| {
                    ApiError::Internal(format!(
                        "Failed to build observability proxy HTTP client: {e}"
                    ))
                })
        })
        .await
}

/// Return the configuration the browser SDK needs to bootstrap itself.
///
/// Authenticated to keep the response (which leaks the sample ratio /
/// proxy path / service name) inside the existing trust boundary;
/// everything sensitive (endpoint, headers) stays server-side.
#[utoipa::path(
    get,
    path = "/api/v1/observability/config",
    responses(
        (status = 200, description = "Browser SDK bootstrap config", body = BrowserObservabilityConfigDto),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Observability"
)]
pub async fn get_browser_config(
    State(state): State<Arc<AppState>>,
    _auth: FlexibleAuthContext,
) -> Json<BrowserObservabilityConfigDto> {
    let cfg = &state.observability_config;
    Json(BrowserObservabilityConfigDto {
        enabled: cfg.browser.enabled && !cfg.otlp.endpoint.trim().is_empty(),
        service_name: cfg.service_name.clone(),
        proxy_path: cfg.browser.proxy_path.clone(),
        sample_ratio: cfg.browser.sample_ratio,
    })
}

/// Forward a batched OTLP/HTTP traces payload to the configured upstream.
#[utoipa::path(
    post,
    path = "/api/v1/observability/otlp/v1/traces",
    request_body(content_type = "application/x-protobuf", description = "OTLP/HTTP traces payload (protobuf or JSON)"),
    responses(
        (status = 200, description = "Forwarded successfully"),
        (status = 400, description = "Payload too large"),
        (status = 401, description = "Unauthorized"),
        (status = 502, description = "Upstream collector error"),
        (status = 503, description = "Browser observability disabled"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Observability"
)]
pub async fn proxy_traces(
    state: State<Arc<AppState>>,
    auth: FlexibleAuthContext,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    forward_otlp(state, auth, headers, body, "v1/traces").await
}

/// Forward a batched OTLP/HTTP metrics payload to the configured upstream.
#[utoipa::path(
    post,
    path = "/api/v1/observability/otlp/v1/metrics",
    request_body(content_type = "application/x-protobuf", description = "OTLP/HTTP metrics payload (protobuf or JSON)"),
    responses(
        (status = 200, description = "Forwarded successfully"),
        (status = 400, description = "Payload too large"),
        (status = 401, description = "Unauthorized"),
        (status = 502, description = "Upstream collector error"),
        (status = 503, description = "Browser observability disabled"),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Observability"
)]
pub async fn proxy_metrics(
    state: State<Arc<AppState>>,
    auth: FlexibleAuthContext,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    forward_otlp(state, auth, headers, body, "v1/metrics").await
}

async fn forward_otlp(
    State(state): State<Arc<AppState>>,
    _auth: FlexibleAuthContext,
    headers: HeaderMap,
    body: Bytes,
    signal_suffix: &'static str,
) -> Result<Response, ApiError> {
    let cfg = state.observability_config.clone();

    if !cfg.browser.enabled {
        return Err(ApiError::ServiceUnavailable(
            "Browser observability is disabled".to_string(),
        ));
    }

    let upstream_base = cfg.otlp.endpoint.trim();
    if upstream_base.is_empty() {
        return Err(ApiError::ServiceUnavailable(
            "OTLP endpoint not configured".to_string(),
        ));
    }

    if body.len() > MAX_OTLP_BODY_BYTES {
        return Err(ApiError::BadRequest(format!(
            "OTLP payload exceeds {}-byte limit",
            MAX_OTLP_BODY_BYTES
        )));
    }

    // Preserve the inbound content-type so the upstream can parse
    // protobuf vs. JSON correctly. Default to protobuf since that's what
    // the OTel JS exporter uses by default.
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/x-protobuf")
        .to_string();

    let client = upstream_client(&cfg).await?;
    let upstream_url = format!("{}/{}", upstream_base.trim_end_matches('/'), signal_suffix);

    let mut req = client
        .post(&upstream_url)
        .header(header::CONTENT_TYPE, content_type)
        .body(body);

    // Layer the operator-configured headers last so they win over any
    // header that might have come from the browser. Browser-supplied
    // headers (other than content-type, which we set explicitly above)
    // are intentionally dropped.
    for (k, v) in cfg.otlp.headers.iter() {
        req = req.header(k, v);
    }

    let upstream_response = req.send().await.map_err(|e| {
        tracing::warn!(error = %e, url = %upstream_url, "OTLP forward failed");
        ApiError::Internal(format!("Failed to reach OTLP upstream: {e}"))
    })?;

    let status = upstream_response.status();
    let upstream_body = upstream_response.bytes().await.unwrap_or_default();

    if !status.is_success() {
        tracing::warn!(
            status = %status,
            url = %upstream_url,
            "OTLP upstream returned non-success"
        );
        return Ok((
            StatusCode::BAD_GATEWAY,
            [(header::CONTENT_TYPE, "application/octet-stream")],
            upstream_body,
        )
            .into_response());
    }

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/octet-stream")],
        upstream_body,
    )
        .into_response())
}
