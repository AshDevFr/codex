//! HTTP integration tests for the OpenTelemetry middleware wiring.
//!
//! Phase 1 of the OTLP plan installs the `axum-tracing-opentelemetry` layers
//! into the router behind a config flag. These tests cover the wiring
//! decisions we make, not the end-to-end propagation behavior of the layers
//! themselves (which require a real SDK runtime + collector to observe
//! correctly and are validated by the manual SigNoz smoke test in the plan).
//!
//! What we DO test here:
//!  - When observability is disabled in config, no OTel response headers
//!    appear (the layers are absent).
//!  - The OTel layer + tracer bridge attaches a valid trace context to a
//!    span when scoped through `with_default`. This confirms our provider
//!    construction is correct without polluting the global subscriber slot,
//!    which would conflict with other tests' use of `tracing_test`.

#![cfg(feature = "observability")]

#[path = "../common/mod.rs"]
mod common;

use codex::api::routes::create_router;
use codex::config::{Config, ObservabilityConfig, OtlpConfig, OtlpProtocol};
use common::*;
use hyper::StatusCode;
use tracing_subscriber::layer::SubscriberExt;

fn base_observability_cfg(enabled: bool) -> ObservabilityConfig {
    ObservabilityConfig {
        enabled,
        service_name: "codex-tests".to_string(),
        otlp: OtlpConfig {
            // Unreachable endpoint by design: tests only verify layer wiring,
            // not real export.
            endpoint: "http://127.0.0.1:1".to_string(),
            protocol: OtlpProtocol::HttpProtobuf,
            headers: Default::default(),
            timeout_ms: 100,
            proxy_endpoint: None,
        },
        traces: codex::config::ObservabilityTracesConfig {
            enabled: true,
            sample_ratio: 1.0,
        },
        metrics: codex::config::ObservabilityMetricsConfig {
            enabled: false,
            export_interval_ms: 60_000,
        },
        browser: Default::default(),
    }
}

fn config_with_observability(enabled: bool) -> Config {
    let mut config = create_test_config();
    config.observability = base_observability_cfg(enabled);
    config
}

#[tokio::test]
async fn disabled_router_does_not_inject_traceparent() {
    let (db, _temp_dir) = setup_test_db().await;
    let (state, _router) = setup_test_app(db).await;

    let config = config_with_observability(false);
    let app = create_router(state, &config);

    let incoming_traceparent = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";
    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/health")
        .header("traceparent", incoming_traceparent)
        .body(String::new())
        .unwrap();

    let (status, headers, _body) = make_full_request(app, request).await;
    assert_eq!(status, StatusCode::OK);

    assert!(
        headers.get("traceparent").is_none(),
        "no traceparent should appear in response when observability is disabled"
    );
}

#[tokio::test]
async fn enabled_router_health_still_responds() {
    // Confirms layers don't break basic request handling when observability
    // is enabled. End-to-end traceparent propagation is validated manually
    // against a live collector (see Phase 1 manual verification task).
    let handle = codex::observability::init(&base_observability_cfg(true))
        .expect("init OTel providers for the enabled-router smoke test");

    let (db, _temp_dir) = setup_test_db().await;
    let (state, _router) = setup_test_app(db).await;

    let config = config_with_observability(true);
    let app = create_router(state, &config);

    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/health")
        .header(
            "traceparent",
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
        )
        .body(String::new())
        .unwrap();

    let (status, _headers, _body) = make_full_request(app, request).await;
    assert_eq!(status, StatusCode::OK);
    handle.shutdown();
}

#[tokio::test]
async fn otel_bridge_attaches_valid_trace_context_to_spans() {
    use opentelemetry::trace::TraceContextExt;
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    // We deliberately scope the subscriber with `with_default` instead of
    // calling `init()` because installing a global subscriber from a test
    // would conflict with other tests (e.g. `#[tracing_test::traced_test]`)
    // that need to install their own.
    let handle = codex::observability::init(&base_observability_cfg(true))
        .expect("init OTel providers for the bridge test");
    let tracer = handle.tracer().cloned().expect("tracer should exist");
    let subscriber =
        tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));

    tracing::subscriber::with_default(subscriber, || {
        // Mirror the call OtelAxumLayer makes internally (TRACE level on the
        // "otel::tracing" target). If the bridge is wired the span carries a
        // valid OTel SpanContext with a non-INVALID trace_id.
        let span = tracing::span!(
            target: "otel::tracing",
            tracing::Level::TRACE,
            "phase1_smoke"
        );
        let _entered = span.enter();
        let ctx = tracing::Span::current().context();
        let span = ctx.span();
        let span_ctx = span.span_context();
        assert!(
            span_ctx.trace_id() != opentelemetry::trace::TraceId::INVALID,
            "tracer + tracing-opentelemetry bridge must produce a valid trace ID"
        );
        assert!(
            span_ctx.span_id() != opentelemetry::trace::SpanId::INVALID,
            "tracer + tracing-opentelemetry bridge must produce a valid span ID"
        );
    });

    handle.shutdown();
}
