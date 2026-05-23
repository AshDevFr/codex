// Heavyweight bootstrap for the OTel web SDK. Only imported when the
// server config flag turns RUM on — see ../observability/index.ts for
// the gated entry point.

import { ZoneContextManager } from "@opentelemetry/context-zone";
import { OTLPTraceExporter } from "@opentelemetry/exporter-trace-otlp-http";
import { registerInstrumentations } from "@opentelemetry/instrumentation";
import { DocumentLoadInstrumentation } from "@opentelemetry/instrumentation-document-load";
import { FetchInstrumentation } from "@opentelemetry/instrumentation-fetch";
import { UserInteractionInstrumentation } from "@opentelemetry/instrumentation-user-interaction";
import { resourceFromAttributes } from "@opentelemetry/resources";
import {
  BatchSpanProcessor,
  ParentBasedSampler,
  TraceIdRatioBasedSampler,
} from "@opentelemetry/sdk-trace-base";
import { WebTracerProvider } from "@opentelemetry/sdk-trace-web";
import {
  ATTR_SERVICE_NAME,
  ATTR_SERVICE_VERSION,
} from "@opentelemetry/semantic-conventions";
import type { BrowserObservabilityConfig } from ".";

const APP_VERSION = (import.meta.env.PACKAGE_VERSION as string) || "unknown";

let started = false;

/**
 * Register the OTel web tracer provider with the document-load, fetch,
 * and user-interaction instrumentations. Idempotent; second + later
 * calls are no-ops.
 */
export function startTracer(config: BrowserObservabilityConfig): void {
  if (started) {
    return;
  }
  started = true;

  const tracesUrl = `${trimTrailingSlash(config.proxyPath)}/v1/traces`;

  const provider = new WebTracerProvider({
    resource: resourceFromAttributes({
      [ATTR_SERVICE_NAME]: config.serviceName || "codex-web",
      [ATTR_SERVICE_VERSION]: APP_VERSION,
    }),
    sampler: new ParentBasedSampler({
      root: new TraceIdRatioBasedSampler(clampRatio(config.sampleRatio)),
    }),
    spanProcessors: [
      new BatchSpanProcessor(
        new OTLPTraceExporter({
          url: tracesUrl,
          // The proxy is same-origin; cookies / bearer headers go along
          // for free. We deliberately do NOT set custom Authorization
          // headers here — the server proxy adds the upstream auth.
        }),
        {
          // Modest defaults: flush every ~5s or 512 spans, whichever first.
          maxExportBatchSize: 512,
          maxQueueSize: 2048,
          scheduledDelayMillis: 5000,
        },
      ),
    ],
  });

  provider.register({
    // ZoneContextManager preserves the active span across async
    // callbacks (setTimeout, fetch promises, etc.) on browsers without
    // AsyncContext support.
    contextManager: new ZoneContextManager(),
  });

  registerInstrumentations({
    instrumentations: [
      new DocumentLoadInstrumentation(),
      new FetchInstrumentation({
        // Only inject traceparent on same-origin (Codex API) requests.
        // We don't want to leak trace context to third-party CDNs.
        propagateTraceHeaderCorsUrls: [
          new RegExp(`^${escapeRegExp(window.location.origin)}/`),
        ],
      }),
      // Default event set is hover-heavy. Restrict to clicks + key
      // presses so the trace volume stays sane on busy pages.
      new UserInteractionInstrumentation({
        eventNames: ["click", "submit"],
      }),
    ],
  });

  // Flush on the tab going away. The OTel BatchSpanProcessor wires its
  // own `pagehide` / `visibilitychange` listeners internally, but we
  // also kick `forceFlush` to be explicit during a hot reload.
  window.addEventListener("pagehide", () => {
    void provider.forceFlush();
  });
}

function trimTrailingSlash(s: string): string {
  return s.endsWith("/") ? s.slice(0, -1) : s;
}

function clampRatio(r: number): number {
  if (!Number.isFinite(r)) {
    return 0;
  }
  if (r < 0) {
    return 0;
  }
  if (r > 1) {
    return 1;
  }
  return r;
}

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
