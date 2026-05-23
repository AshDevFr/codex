---
sidebar_position: 16
---

# Observability (OpenTelemetry)

Codex ships an opt-in OpenTelemetry pipeline that emits **traces** and **metrics** over OTLP, plus an optional **browser RUM** layer that posts spans from the SPA through a same-origin proxy. Logs continue to flow through the existing `tracing-subscriber` stdout/file appender, with trace IDs injected on every line for correlation.

The exporter is vendor-neutral. Anything that speaks OTLP works without code changes: [SigNoz](https://signoz.io/), [Grafana Tempo](https://grafana.com/oss/tempo/) + [Mimir](https://grafana.com/oss/mimir/), [Honeycomb](https://www.honeycomb.io/), [Uptrace](https://uptrace.dev/), the [DataDog Agent](https://docs.datadoghq.com/opentelemetry/) OTLP receiver, and more.

:::tip Default state
Observability is **disabled by default**. Nothing is exported until an operator opts in. This is intentional for a self-hosted product: no telemetry leaves the box without explicit configuration.
:::

## Quickstart (Docker dev environment)

The bundled dev compose ships a Jaeger all-in-one sidecar on the `dev` profile and overrides the Codex config to point at it via env vars. `make dev-up` brings the whole stack up with observability already on — no YAML edit, no restart.

```bash
make dev-up
```

Jaeger exposes its UI at [http://localhost:16686](http://localhost:16686). Hit a few endpoints in the Codex app, then pick **codex** from the service dropdown in Jaeger. Traces should appear within a few seconds.

The env overrides live in `docker-compose.yml` under the `codex-dev` and `codex-dev-worker` services:

```yaml
CODEX_OBSERVABILITY_ENABLED: "true"
CODEX_OBSERVABILITY_SERVICE_NAME: codex
CODEX_OBSERVABILITY_OTLP_ENDPOINT: http://jaeger:4317
CODEX_OBSERVABILITY_OTLP_PROTOCOL: grpc
CODEX_OBSERVABILITY_BROWSER_ENABLED: "true"   # codex-dev only; enables RUM proxy
```

`config/config.docker.yaml` itself ships with the `observability:` block commented out so a production deployment using the same config doesn't quietly start exporting telemetry — the dev override is intentionally local to the compose file.

:::warning Evaluation use only
Jaeger all-in-one stores spans in memory (lost on restart) and the UI has no auth. It is appropriate for local dev and evaluation. For long-term storage, metrics, or a full APM UI in production, point Codex at a real OTLP backend (SigNoz, Grafana Tempo + Mimir, Honeycomb, Uptrace, etc.) per the backend matrix below.
:::

## Quickstart (outside the dev compose)

If you're running Codex outside of `docker-compose.yml`, any OTLP backend works. The smallest standalone setup is the same Jaeger all-in-one image:

```bash
docker run -d --name codex-jaeger \
  -e COLLECTOR_OTLP_ENABLED=true \
  -p 16686:16686 -p 4317:4317 -p 4318:4318 \
  jaegertracing/all-in-one:1.62.0
```

Then enable `observability` in your config file with `otlp.endpoint: http://localhost:4317`.

## Configuration

The full schema is documented in the [Configuration reference](./configuration#observability-configuration). At a minimum, an enabled deployment needs:

- `observability.enabled: true`
- `observability.otlp.endpoint` set to an OTLP collector URL
- `observability.otlp.headers` populated if your backend requires auth (e.g. `signoz-access-token`, `x-honeycomb-team`)

```yaml
observability:
  enabled: true
  otlp:
    endpoint: https://ingest.eu.signoz.cloud:443
    protocol: grpc
    headers:
      signoz-access-token: "your-token-here"
    timeout_ms: 5000
```

### Choosing a backend

| Backend                | Endpoint shape                          | Protocol  | Notes                                                                 |
| ---------------------- | --------------------------------------- | --------- | --------------------------------------------------------------------- |
| Self-hosted SigNoz     | `http://signoz-otel-collector:4317`     | `grpc`    | Easiest local setup. Use the bundled compose file below.              |
| SigNoz Cloud           | `https://ingest.<region>.signoz.cloud`  | `grpc`    | Requires `signoz-access-token` header.                                |
| Grafana Tempo (local)  | `http://tempo:4317`                     | `grpc`    | Pair with Mimir for metrics. Grafana renders both.                    |
| Honeycomb              | `https://api.honeycomb.io`              | `grpc`    | Requires `x-honeycomb-team` (and optionally `x-honeycomb-dataset`).   |
| Uptrace                | `https://otlp.uptrace.dev:4317`         | `grpc`    | Requires `uptrace-dsn` header.                                        |
| DataDog (OTLP receive) | `http://datadog-agent:4317`             | `grpc`    | Agent must have `otlp_config.receiver.protocols.grpc` enabled.        |
| HTTP-only environments | `http://collector:4318`                 | `http/protobuf` | Use when load balancers don't terminate gRPC.                   |

### Choosing a protocol

`grpc` is the default and the right choice in most environments: smaller payloads, persistent connections, lower overhead. Switch to `http/protobuf` only when something between Codex and the collector (a managed load balancer, a strict egress proxy) blocks gRPC. `http/json` exists for parity but produces noticeably larger payloads; prefer `http/protobuf` over it whenever both are an option.

### Sampling guidance

Codex uses a **parent-based** sampler. Practically: if an incoming request already carries a `traceparent`, that decision is honored; otherwise the configured `sample_ratio` decides whether to sample at the root.

| Workload                                  | Recommended `traces.sample_ratio` | Reasoning                                                              |
| ----------------------------------------- | --------------------------------- | ---------------------------------------------------------------------- |
| Local development                         | `1.0`                             | You want every trace while iterating.                                  |
| Small home server (1–5 active users)      | `1.0`                             | Volume is low; full traces are cheap.                                  |
| Medium deployment (10–50 active users)    | `0.25`–`0.5`                      | Keep tail latency debuggable without flooding the collector.           |
| Large/multi-tenant (100+ active users)    | `0.05`–`0.1`                      | Pair with backend-side tail sampling if your collector supports it.    |
| Diagnosing a specific incident            | `1.0` temporarily                 | Crank up while reproducing, then back off.                             |

Browser RUM defaults to `browser.sample_ratio: 0.1` because a busy SPA can produce many spans per user session. Raise it cautiously: a noisy front end can dwarf backend traffic at the collector.

:::note Sample ratio decisions are local
The Rust SDK samples at the root span. If a downstream service (e.g. a plugin subprocess in a future iteration) makes its own decision, it does so independently. There is no global coordination.
:::

## What Codex sends

### Trace spans

- **HTTP server spans** — every request, named by matched route template (e.g. `GET /api/v1/series/:id`, not the resolved URL). Standard `http.*` semantic-convention attributes.
- **Repository spans** — `db.<entity>.<op>` for hot-path operations on books, series, libraries, users, and plugin records. Carry `db.system`, `db.operation`, and the entity ID as an attribute (never in the span name).
- **Plugin RPC spans** — `plugin.<method>` around every JSON-RPC call to a plugin subprocess. Internal `plugin.rpc.write` / `plugin.rpc.wait` child spans break down the round-trip into stdio write vs. response wait.
- **Scanner spans** — `scanner.scan_library` / `scanner.analyze_book` as root spans for background work.
- **Task worker spans** — `task.execute` per claimed task, carrying `task.id` and `task.type`.

### Metrics

Two flavors land in the OTLP pipeline:

- **Counters and histograms** — dual-written from the in-process plugin and task metrics services. Histograms (not just averages) let p95/p99 be queried server-side.
- **Observable gauges** — inventory snapshot (libraries, series, books, users, pages), refreshed every 30s; process CPU/memory; task in-flight count.

Concrete metric names:

| Metric                            | Type                | Attributes                                              |
| --------------------------------- | ------------------- | ------------------------------------------------------- |
| `codex.plugin.requests.total`     | Counter             | `plugin_id`, `method`, `outcome`                        |
| `codex.plugin.duration_ms`        | Histogram (ms)      | `plugin_id`, `method`, `outcome`                        |
| `codex.task.completed.total`      | Counter             | `task_type`, `outcome`                                  |
| `codex.task.duration_ms`          | Histogram (ms)      | `task_type`, `outcome`                                  |
| `codex.task.queue_wait_ms`        | Histogram (ms)      | `task_type`                                             |
| `codex.task.in_flight`            | Observable gauge    | (none)                                                  |
| `codex.inventory.libraries`       | Observable gauge    | (none)                                                  |
| `codex.inventory.series`          | Observable gauge    | (none)                                                  |
| `codex.inventory.books`           | Observable gauge    | (none)                                                  |
| `codex.inventory.users`           | Observable gauge    | (none)                                                  |
| `codex.inventory.pages`           | Observable gauge    | (none)                                                  |
| `http.server.request.duration`    | Histogram (seconds) | `http.request.method`, `http.route`, `http.response.status_code` |
| `process.cpu.time`                | Observable gauge    | (none)                                                  |
| `process.memory.usage`            | Observable gauge    | (none)                                                  |
| `process.memory.virtual`          | Observable gauge    | (none)                                                  |

The existing [`/api/v1/metrics/plugins`](./api) dashboard endpoint is unchanged. The in-app store is still authoritative for that view; OTLP is a parallel consumer.

### What Codex does **not** send

- **Logs.** Stdout / file logging is unchanged. Trace IDs are injected on every line so you can ship logs separately (Vector, Filebeat, Loki, etc.) and correlate by trace ID.
- **Resource bodies.** Span attributes carry IDs and operation names, not titles, file contents, or query strings.
- **User-identifying browser data.** The browser SDK emits document-load, fetch, click, and submit spans. There is no session replay, no DOM capture, no PII enrichment.
- **Cross-process plugin spans.** Plugin RPC spans wrap the manager-side call; `traceparent` is not propagated into plugin subprocesses in this release. Plugins remain black boxes from a tracing perspective.

## Browser RUM

When `observability.browser.enabled: true`:

1. The SPA fetches `GET /api/v1/observability/config` on startup. If the server flag is on **and** an OTLP endpoint is configured, the heavyweight OTel browser SDK is dynamically imported. Otherwise the chunk is never downloaded.
2. The SDK registers `document-load`, `fetch`, `user-interaction` (click + submit only), and `xml-http-request` instrumentations.
3. Spans are batched in memory (flush every 5s or 512 spans, max queue 2048) and POSTed to `/api/v1/observability/otlp/v1/traces`.
4. Codex forwards the OTLP body verbatim to the configured collector, swapping in the operator-configured `otlp.headers`. Browser-supplied headers are dropped except for `Content-Type`.
5. On `pagehide`, the SDK uses `navigator.sendBeacon()` to flush the final batch so spans survive navigation.

`FetchInstrumentation.propagateTraceHeaderCorsUrls` is anchored to `window.location.origin`, so `traceparent` is injected only on Codex API calls and never leaked to third-party CDNs or external metadata sources.

### Why the proxy?

The proxy exists for three reasons:

1. **No CORS configuration on the collector.** The SPA always POSTs to its own origin.
2. **No collector credentials in the browser.** Auth tokens stay on the server.
3. **Reuses existing session auth.** The proxy is `FlexibleAuthContext`-gated, so the cookie or bearer the SPA already carries authenticates the export. The OTel JS exporter does not need custom auth wiring.

The proxy is a thin pass-through. It does not buffer, batch, transform, or sample. Body size is capped at 4 MiB and per-session rate limits apply.

## Trace ID correlation in logs

When observability is enabled, log lines pick up trace context:

```
2026-05-22T18:02:11.034Z  INFO trace_id=4bf92f3577b34da6a3ce929d0e0e4736 span_id=00f067aa0ba902b7 codex::services::plugin::manager: plugin.search_series finished plugin_id=anilist duration_ms=412
```

Ship the log file to any backend that can index by `trace_id` and you can pivot from a slow log line to the SigNoz trace and back.

## Performance impact

Codex's success criteria for this feature are:

- **&lt; 2% added request latency when observability is disabled** (the default).
- **&lt; 5% added request latency when enabled with default sampling.**

The disabled-path overhead is effectively zero: the OTel layer is not installed in the `tracing-subscriber` registry, repository `#[instrument]` attributes compile to inert spans without a subscriber, and metric instruments resolve to no-op implementations from `metrics_stub.rs` under `--no-default-features`. With observability enabled at `sample_ratio: 1.0` on a representative endpoint, measured overhead falls inside the 5% budget (see the benchmark in the implementation notes for the methodology).

If you need to validate on your own deployment:

```bash
# Baseline (observability disabled)
ab -n 1000 -c 10 -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/series?page=1

# Then enable observability, restart, and re-run with the same args.
# Compare p50/p95/p99 in the ab output.
```

## Disabling observability

Three ways, in order of granularity:

1. **Full off** — set `observability.enabled: false` (the default) and restart. No providers initialize, no telemetry leaves the process.
2. **Per-signal off** — keep `observability.enabled: true` but set `observability.traces.enabled: false` or `observability.metrics.enabled: false`. Useful when one pipeline needs maintenance.
3. **Sampling to zero** — `observability.traces.sample_ratio: 0.0` keeps the layer installed (so incoming `traceparent` is still extracted for logging) but no new traces start at the root. Cheaper than restarting if you need to drop trace volume without redeploying.

Browser RUM has its own switch: `observability.browser.enabled: false` disables the proxy endpoint and the SPA's config payload reports `enabled: false`, so the SDK chunk is never downloaded.

## Troubleshooting

**Traces don't appear in the backend.**

- Check the Codex logs for `otel_status_code=ERROR` lines or `failed to export` warnings.
- Confirm `observability.enabled` is `true` **and** `observability.otlp.endpoint` is non-empty. An enabled config with an empty endpoint is treated as a misconfiguration and the OTel layer is not installed.
- For gRPC endpoints, the URL scheme matters: `http://host:4317` for cleartext, `https://host:4317` for TLS.
- For HTTP/protobuf endpoints, the SDK appends `/v1/traces` and `/v1/metrics` to the base URL. Configure `http://collector:4318`, not `http://collector:4318/v1/traces`.

**Metrics arrive but with the wrong tenant / project / dataset.**

- Headers configured under `observability.otlp.headers` apply to **both** traces and metrics exports. Most multi-tenant backends use a single header (e.g. `x-honeycomb-team`); for backends that route by dataset, set the dataset header at the OTLP level too.

**Browser traces don't show up.**

- Confirm `GET /api/v1/observability/config` returns `enabled: true` in the response body. If it returns `enabled: false` while you have `browser.enabled: true` in YAML, the OTLP endpoint is probably empty.
- Open the network panel. Successful proxy POSTs to `/api/v1/observability/otlp/v1/traces` return `204 No Content`. A `503` means the proxy is disabled.
- The `tracer-*.js` chunk is loaded asynchronously. If it never appears in the network panel, the bootstrap probe failed or the chunk was blocked by an extension.

**`cargo build --no-default-features` after enabling observability.**

- The `observability` feature is in `default = ["rar", "observability"]`. `--no-default-features` compiles against the stub module: all instrumentation calls become no-ops and the OTel crates are not linked. There is no runtime config change required.

## Reference

- [Configuration reference](./configuration#observability-configuration) — full schema and environment variable list
- [`docker-compose.yml`](https://github.com/AshDevFr/codex/blob/main/docker-compose.yml) — bundled Jaeger sidecar lives on the `dev` profile
- [OpenTelemetry Rust](https://github.com/open-telemetry/opentelemetry-rust) — SDK source
- [OpenTelemetry JS browser SDK](https://opentelemetry.io/docs/languages/js/) — browser SDK source
- [W3C Trace Context](https://www.w3.org/TR/trace-context/) — the propagation format used end-to-end
