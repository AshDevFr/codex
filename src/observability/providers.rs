//! OTel SDK provider construction and lifetime management.

use std::time::Duration;

use anyhow::{Context, Result};
use opentelemetry::{KeyValue, global, trace::TracerProvider};
use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig, WithTonicConfig};
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
    propagation::TraceContextPropagator,
    trace::{Sampler, SdkTracerProvider, Tracer},
};
use opentelemetry_semantic_conventions::resource::SERVICE_VERSION;

use crate::config::{ObservabilityConfig, OtlpProtocol};

const TRACER_INSTRUMENTATION_NAME: &str = "codex";

/// Owns the OTel providers for the lifetime of the process.
///
/// Drop alone does *not* flush the batch processors; call [`Self::shutdown`]
/// from the serve command on graceful exit to make sure the last spans and
/// metric points are delivered.
pub struct ObservabilityHandle {
    inner: Option<Inner>,
}

struct Inner {
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
    tracer: Option<Tracer>,
}

impl ObservabilityHandle {
    fn disabled() -> Self {
        Self { inner: None }
    }

    /// Returns the SDK tracer used by the `tracing-opentelemetry` bridge.
    ///
    /// `None` when observability is disabled or trace export is off.
    pub fn tracer(&self) -> Option<&Tracer> {
        self.inner.as_ref().and_then(|i| i.tracer.as_ref())
    }

    /// Returns whether trace export is active.
    pub fn traces_enabled(&self) -> bool {
        self.tracer().is_some()
    }

    /// Returns whether metric export is active.
    pub fn metrics_enabled(&self) -> bool {
        self.inner
            .as_ref()
            .and_then(|i| i.meter_provider.as_ref())
            .is_some()
    }

    /// Flush and shut down the providers. Idempotent.
    ///
    /// Logs at warn level on per-provider failure; we never want a flush error
    /// to cascade past process shutdown.
    pub fn shutdown(mut self) {
        let Some(inner) = self.inner.take() else {
            return;
        };
        if let Some(tp) = inner.tracer_provider
            && let Err(e) = tp.shutdown()
        {
            tracing::warn!("Failed to shut down OTel tracer provider: {e}");
        }
        if let Some(mp) = inner.meter_provider
            && let Err(e) = mp.shutdown()
        {
            tracing::warn!("Failed to shut down OTel meter provider: {e}");
        }
    }
}

/// Build providers from config and install them as the OTel globals.
///
/// Returns a handle even when nothing was installed (the disabled / no-op
/// path), so the caller can treat the result uniformly.
pub fn init(config: &ObservabilityConfig) -> Result<ObservabilityHandle> {
    if !config.enabled {
        tracing::debug!("Observability disabled via config");
        return Ok(ObservabilityHandle::disabled());
    }

    if config.otlp.endpoint.trim().is_empty() {
        tracing::warn!(
            "observability.enabled = true but otlp.endpoint is empty; not installing OTel providers"
        );
        return Ok(ObservabilityHandle::disabled());
    }

    // Install the W3C trace-context propagator so incoming `traceparent`
    // headers are honored and outgoing requests can carry the context.
    global::set_text_map_propagator(TraceContextPropagator::new());

    let resource = build_resource(config);

    let tracer_provider = if config.traces.enabled {
        Some(build_tracer_provider(config, resource.clone())?)
    } else {
        None
    };

    let tracer = tracer_provider
        .as_ref()
        .map(|tp| tp.tracer(TRACER_INSTRUMENTATION_NAME));

    if let Some(tp) = tracer_provider.as_ref() {
        global::set_tracer_provider(tp.clone());
    }

    let meter_provider = if config.metrics.enabled {
        Some(build_meter_provider(config, resource)?)
    } else {
        None
    };

    if let Some(mp) = meter_provider.as_ref() {
        global::set_meter_provider(mp.clone());
    }

    tracing::info!(
        endpoint = %config.otlp.endpoint,
        protocol = %config.otlp.protocol.as_str(),
        traces_enabled = config.traces.enabled,
        metrics_enabled = config.metrics.enabled,
        sample_ratio = config.traces.sample_ratio,
        "Initialized OpenTelemetry providers"
    );

    Ok(ObservabilityHandle {
        inner: Some(Inner {
            tracer_provider,
            meter_provider,
            tracer,
        }),
    })
}

fn build_resource(config: &ObservabilityConfig) -> Resource {
    Resource::builder()
        .with_service_name(config.service_name.clone())
        .with_attribute(KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")))
        .build()
}

fn build_tracer_provider(
    config: &ObservabilityConfig,
    resource: Resource,
) -> Result<SdkTracerProvider> {
    let exporter = build_span_exporter(config)?;
    let sampler = build_sampler(config.traces.sample_ratio);

    Ok(SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .with_sampler(sampler)
        .build())
}

fn build_meter_provider(
    config: &ObservabilityConfig,
    resource: Resource,
) -> Result<SdkMeterProvider> {
    let exporter = build_metric_exporter(config)?;
    let reader = PeriodicReader::builder(exporter)
        .with_interval(Duration::from_millis(config.metrics.export_interval_ms))
        .build();

    Ok(SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(resource)
        .build())
}

fn build_sampler(ratio: f64) -> Sampler {
    // ParentBased so propagated decisions from upstream callers are honored;
    // local roots use the configured ratio.
    let clamped = ratio.clamp(0.0, 1.0);
    let root = if clamped >= 1.0 {
        Sampler::AlwaysOn
    } else if clamped <= 0.0 {
        Sampler::AlwaysOff
    } else {
        Sampler::TraceIdRatioBased(clamped)
    };
    Sampler::ParentBased(Box::new(root))
}

fn build_span_exporter(config: &ObservabilityConfig) -> Result<opentelemetry_otlp::SpanExporter> {
    let timeout = Duration::from_millis(config.otlp.timeout_ms);
    let endpoint = config.otlp.endpoint.clone();
    match config.otlp.protocol {
        OtlpProtocol::Grpc => {
            let mut builder = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .with_timeout(timeout);
            if !config.otlp.headers.is_empty() {
                builder = builder
                    .with_metadata(build_tonic_metadata(&config.otlp.headers).context(
                        "Failed to build gRPC metadata from observability.otlp.headers",
                    )?);
            }
            builder
                .build()
                .context("Failed to build OTLP gRPC span exporter")
        }
        OtlpProtocol::HttpProtobuf | OtlpProtocol::HttpJson => {
            let protocol = match config.otlp.protocol {
                OtlpProtocol::HttpJson => Protocol::HttpJson,
                _ => Protocol::HttpBinary,
            };
            opentelemetry_otlp::SpanExporter::builder()
                .with_http()
                .with_protocol(protocol)
                .with_endpoint(endpoint)
                .with_timeout(timeout)
                .with_headers(config.otlp.headers.clone())
                .build()
                .context("Failed to build OTLP HTTP span exporter")
        }
    }
}

fn build_metric_exporter(
    config: &ObservabilityConfig,
) -> Result<opentelemetry_otlp::MetricExporter> {
    let timeout = Duration::from_millis(config.otlp.timeout_ms);
    let endpoint = config.otlp.endpoint.clone();
    match config.otlp.protocol {
        OtlpProtocol::Grpc => {
            let mut builder = opentelemetry_otlp::MetricExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .with_timeout(timeout);
            if !config.otlp.headers.is_empty() {
                builder = builder
                    .with_metadata(build_tonic_metadata(&config.otlp.headers).context(
                        "Failed to build gRPC metadata from observability.otlp.headers",
                    )?);
            }
            builder
                .build()
                .context("Failed to build OTLP gRPC metric exporter")
        }
        OtlpProtocol::HttpProtobuf | OtlpProtocol::HttpJson => {
            let protocol = match config.otlp.protocol {
                OtlpProtocol::HttpJson => Protocol::HttpJson,
                _ => Protocol::HttpBinary,
            };
            opentelemetry_otlp::MetricExporter::builder()
                .with_http()
                .with_protocol(protocol)
                .with_endpoint(endpoint)
                .with_timeout(timeout)
                .with_headers(config.otlp.headers.clone())
                .build()
                .context("Failed to build OTLP HTTP metric exporter")
        }
    }
}

fn build_tonic_metadata(
    headers: &std::collections::HashMap<String, String>,
) -> Result<tonic::metadata::MetadataMap> {
    let mut map = tonic::metadata::MetadataMap::with_capacity(headers.len());
    for (k, v) in headers {
        let key: tonic::metadata::MetadataKey<tonic::metadata::Ascii> = k
            .parse()
            .with_context(|| format!("invalid OTLP header name: {k}"))?;
        let value: tonic::metadata::MetadataValue<tonic::metadata::Ascii> = v
            .parse()
            .with_context(|| format!("invalid OTLP header value for {k}"))?;
        map.insert(key, value);
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config() -> ObservabilityConfig {
        ObservabilityConfig {
            enabled: true,
            service_name: "codex-test".to_string(),
            otlp: crate::config::OtlpConfig {
                endpoint: "http://127.0.0.1:14318".to_string(),
                protocol: OtlpProtocol::HttpProtobuf,
                headers: Default::default(),
                timeout_ms: 1000,
            },
            traces: crate::config::ObservabilityTracesConfig {
                enabled: true,
                sample_ratio: 1.0,
            },
            metrics: crate::config::ObservabilityMetricsConfig {
                enabled: true,
                export_interval_ms: 1000,
            },
            browser: Default::default(),
        }
    }

    #[test]
    fn init_disabled_returns_noop() {
        let mut cfg = base_config();
        cfg.enabled = false;
        let handle = init(&cfg).unwrap();
        assert!(!handle.traces_enabled());
        assert!(!handle.metrics_enabled());
        handle.shutdown();
    }

    #[test]
    fn init_empty_endpoint_returns_noop() {
        let mut cfg = base_config();
        cfg.otlp.endpoint.clear();
        let handle = init(&cfg).unwrap();
        assert!(!handle.traces_enabled());
        assert!(!handle.metrics_enabled());
        handle.shutdown();
    }

    #[tokio::test]
    async fn init_with_fake_endpoint_builds_providers_and_shuts_down() {
        // The exporter is constructed lazily; it does not require the endpoint
        // to be reachable at init time. Shutdown is what proves the providers
        // and exporters are wired up cleanly.
        let cfg = base_config();
        let handle = init(&cfg).unwrap();
        assert!(handle.traces_enabled());
        assert!(handle.metrics_enabled());
        handle.shutdown();
    }

    #[test]
    fn sampler_clamps_ratio() {
        // Just exercising the helper for the corner values; we trust the SDK
        // implementation of TraceIdRatioBased itself.
        assert!(matches!(build_sampler(-1.0), Sampler::ParentBased(_)));
        assert!(matches!(build_sampler(2.0), Sampler::ParentBased(_)));
        assert!(matches!(build_sampler(0.5), Sampler::ParentBased(_)));
    }

    #[test]
    fn service_name_in_resource() {
        let cfg = base_config();
        let resource = build_resource(&cfg);
        let attrs: Vec<_> = resource.iter().collect();
        let has_service_name = attrs.iter().any(|(k, v)| {
            k.as_str() == opentelemetry_semantic_conventions::resource::SERVICE_NAME
                && v.to_string() == "codex-test"
        });
        assert!(
            has_service_name,
            "service.name attribute not set: {attrs:?}"
        );
    }
}
