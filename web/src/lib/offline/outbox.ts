/**
 * Page-side outbox helpers for offline write operations.
 *
 * The reading-progress mutation client (and other write paths down the
 * road) wraps its real network call in a try/catch: on offline failure it
 * serialises the request, hands it to {@link enqueueOfflineWrite}, and
 * throws an {@link OfflineQueuedError} so the caller knows the write was
 * deferred rather than lost. The queue is drained automatically on the
 * window `online` event and when the tab returns to `visible`; manual
 * drains are also available for tests and explicit "Retry now" UX.
 *
 * The drain order is sequential and stops at the first failure. Reading
 * progress for the same book must apply in the order the user produced it
 * (we don't want page 20 to overwrite a later page 25), so parallel drain
 * is intentionally avoided.
 */

import {
  drainOutbox as drainOutboxStore,
  enqueueOutbox,
  type OutboxRecord,
} from "./db";

export interface SerialisableRequest {
  url: string;
  method: string;
  /** Optional. Defaults to an empty bag; capture auth headers at enqueue time. */
  headers?: Record<string, string>;
  /**
   * JSON-serialisable body. Will be stringified before storage so it
   * survives reads from IDB intact.
   */
  body?: unknown;
}

/**
 * Thrown by API wrappers after a write has been queued for later delivery.
 * Callers catching this can treat the write as "stored locally" rather than
 * "failed" and avoid surfacing an error to the user.
 */
export class OfflineQueuedError extends Error {
  readonly request: SerialisableRequest;
  constructor(request: SerialisableRequest) {
    super("Request queued for offline delivery");
    this.name = "OfflineQueuedError";
    this.request = request;
  }
}

export function isOfflineQueuedError(err: unknown): err is OfflineQueuedError {
  return err instanceof OfflineQueuedError;
}

/**
 * Heuristic for "the request never reached a server, so queueing it makes
 * sense" versus "the server replied with an error, queueing won't help".
 * Recognises both the project's ApiError shape (`{ error: "Network Error" }`,
 * produced by [api/client.ts](../../api/client.ts) when axios sees no response)
 * and raw axios errors for cases that bypass the interceptor.
 */
export function isOfflineError(err: unknown): boolean {
  if (typeof navigator !== "undefined" && navigator.onLine === false) {
    return true;
  }
  if (!err || typeof err !== "object") return false;
  const e = err as {
    error?: string;
    code?: string;
    response?: unknown;
    message?: string;
  };
  if (e.error === "Network Error") return true;
  if (e.code === "ERR_NETWORK") return true;
  if (e.code === "ECONNABORTED") return true;
  if (e.response === undefined && typeof e.message === "string") {
    const lower = e.message.toLowerCase();
    if (lower.includes("network") || lower.includes("fetch failed")) {
      return true;
    }
  }
  return false;
}

/**
 * Persist a request to the outbox store and return its row id.
 *
 * Headers and body are normalised so the drain step can replay them with a
 * plain `fetch()` call:
 * - `headers` is shallow-copied into a `Record<string, string>`.
 * - `body` is JSON-stringified (undefined remains undefined).
 */
export async function enqueueOfflineWrite(
  request: SerialisableRequest,
): Promise<number> {
  const headers = request.headers ? { ...request.headers } : {};
  const body =
    request.body === undefined ? undefined : JSON.stringify(request.body);
  return enqueueOutbox({
    url: request.url,
    method: request.method.toUpperCase(),
    headers,
    body,
  });
}

/**
 * Sender used by {@link drainOfflineOutbox}. Tests inject a mock; production
 * defaults to {@link defaultDrainSender} which uses plain `fetch()` so the
 * outbox module avoids depending on axios.
 */
export type OutboxSender = (record: OutboxRecord) => Promise<void>;

async function defaultDrainSender(record: OutboxRecord): Promise<void> {
  const init: RequestInit = {
    method: record.request.method,
    headers: record.request.headers,
    credentials: "include",
  };
  if (record.request.body !== undefined) {
    init.body = record.request.body;
  }
  const response = await fetch(record.request.url, init);
  if (!response.ok) {
    throw new Error(
      `HTTP ${response.status} replaying ${record.request.method} ${record.request.url}`,
    );
  }
}

let drainInFlight: Promise<number> | null = null;

/**
 * Drain the outbox sequentially. Concurrent calls share one in-flight
 * promise so a flurry of `online` + `visibilitychange` events do not start
 * overlapping drains.
 *
 * Resolves with the number of records successfully replayed. A drain that
 * fails partway through still resolves (the failing record stays at the
 * head of the queue with its retry count bumped — see
 * [db.ts](./db.ts#drainOutbox)).
 */
export function drainOfflineOutbox(
  send: OutboxSender = defaultDrainSender,
): Promise<number> {
  // Not declared `async` so the returned promise is the same reference for
  // every concurrent call (otherwise the implicit async wrapper produces a
  // fresh promise per invocation and the dedupe contract leaks).
  if (drainInFlight) return drainInFlight;
  drainInFlight = (async () => {
    try {
      return await drainOutboxStore(send);
    } finally {
      drainInFlight = null;
    }
  })();
  return drainInFlight;
}

let listenersInstalled = false;
let installedOnline: (() => void) | null = null;
let installedVisibility: (() => void) | null = null;

/**
 * Install global `online` and `visibilitychange` listeners that drain the
 * outbox automatically. Safe to call more than once: subsequent calls are
 * no-ops and return the same teardown function. Returns the teardown
 * function for tests that need to uninstall.
 */
export function installOutboxDrainListeners(): () => void {
  if (listenersInstalled) return uninstallOutboxDrainListeners;
  if (typeof window === "undefined" || typeof document === "undefined") {
    return uninstallOutboxDrainListeners;
  }

  const onOnline = () => {
    void drainOfflineOutbox().catch(() => {
      // Swallow: drainOutbox already handles per-record retry bookkeeping,
      // and we don't want a transient failure to leak unhandled rejections
      // into the browser console.
    });
  };
  const onVisibility = () => {
    if (document.visibilityState === "visible") {
      void drainOfflineOutbox().catch(() => {});
    }
  };

  window.addEventListener("online", onOnline);
  document.addEventListener("visibilitychange", onVisibility);
  installedOnline = onOnline;
  installedVisibility = onVisibility;
  listenersInstalled = true;

  return uninstallOutboxDrainListeners;
}

export function uninstallOutboxDrainListeners(): void {
  if (!listenersInstalled) return;
  if (installedOnline && typeof window !== "undefined") {
    window.removeEventListener("online", installedOnline);
  }
  if (installedVisibility && typeof document !== "undefined") {
    document.removeEventListener("visibilitychange", installedVisibility);
  }
  installedOnline = null;
  installedVisibility = null;
  listenersInstalled = false;
}

/**
 * Reset transient state. Test-only.
 */
export function _resetOutboxLifecycleForTests(): void {
  uninstallOutboxDrainListeners();
  drainInFlight = null;
}
