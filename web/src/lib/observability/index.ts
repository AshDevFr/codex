// Lightweight entry point: ask the server whether RUM is enabled, then
// dynamically import the SDK bundle only if we need it. The full SDK
// pulls in ~120 KB of JS (gzipped) and we do not want that cost on every
// page load when observability is off (the default).

const CONFIG_URL = "/api/v1/observability/config";

export interface BrowserObservabilityConfig {
  enabled: boolean;
  serviceName: string;
  proxyPath: string;
  sampleRatio: number;
}

let initPromise: Promise<void> | null = null;

/**
 * Fetch the server-side bootstrap config and, if RUM is enabled, lazily
 * import and start the OTel web SDK. Safe to call multiple times — only
 * the first invocation actually does work.
 *
 * Failures are logged and swallowed: observability must never break the
 * SPA. If the server is unreachable or the user is not yet authenticated,
 * we just leave the SDK uninitialized and the app keeps working.
 */
export function initObservability(): Promise<void> {
  if (initPromise) {
    return initPromise;
  }
  initPromise = (async () => {
    let config: BrowserObservabilityConfig | null = null;
    try {
      const res = await fetch(CONFIG_URL, {
        credentials: "include",
        headers: { Accept: "application/json" },
      });
      if (!res.ok) {
        return;
      }
      config = (await res.json()) as BrowserObservabilityConfig;
    } catch {
      // Network error, server not reachable, etc. Stay silent.
      return;
    }

    if (!config?.enabled) {
      return;
    }

    try {
      const { startTracer } = await import("./tracer");
      startTracer(config);
    } catch (err) {
      // SDK import failed — possibly a code split error. Log to console
      // for debugging; do not surface to the user.
      console.warn("[observability] failed to start OTel web SDK", err);
    }
  })();
  return initPromise;
}
