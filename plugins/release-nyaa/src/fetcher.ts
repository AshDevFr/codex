/**
 * Nyaa.si RSS fetcher.
 *
 * Wraps `fetch` with conditional GET (`If-None-Match` from a stored ETag, plus
 * `If-Modified-Since` from a stored Last-Modified header) and a hard timeout.
 *
 * Nyaa exposes two feed shapes we care about:
 *   - User feed:   `https://nyaa.si/?page=rss&u=<username>`
 *   - Search feed: `https://nyaa.si/?page=rss&q=<query>` (with optional
 *                  filters; the plugin keeps it simple and lets aliases
 *                  do the matching)
 *
 * Returns a discriminated result so the caller can:
 *   - act on `200`: parse the body, persist the new ETag.
 *   - skip parse on `304`: nothing changed since last poll.
 *   - report `429` / `5xx` upstream-status codes back to the host so the
 *     per-host backoff layer can react.
 *
 * Network is the only side effect; nothing in here touches storage, the host,
 * or process state. That keeps it trivially testable: pass a mocked `fetch`
 * implementation and assert.
 */

/** Discriminated fetch result. */
export type FetchResult =
  | { kind: "ok"; body: string; etag: string | null; lastModified: string | null; status: 200 }
  | { kind: "notModified"; status: 304 }
  | { kind: "error"; status: number; message: string };

export interface FetcherOptions {
  /** Custom `fetch` impl (for testing). Defaults to global `fetch`. */
  fetchImpl?: typeof fetch;
  /** Per-request timeout. Defaults to 10s. */
  timeoutMs?: number;
  /** Override base URL (for tests / mirrors). Defaults to `https://nyaa.si`. */
  baseUrl?: string;
}

/** Default Nyaa base URL. */
export const NYAA_BASE_URL = "https://nyaa.si";

/**
 * One uploader subscription entry. Either a Nyaa username (`kind: "user"`) or
 * an arbitrary search query (`kind: "query"`) for groups without an account.
 */
export type UploaderSubscription =
  | { kind: "user"; identifier: string }
  | { kind: "query"; identifier: string };

/**
 * Parse a single uploader subscription token.
 *
 * Tokens look like:
 *   - `1r0n`               → user
 *   - `q:LuminousScans`    → query
 *   - `query:Manga Group`  → query (long form)
 *
 * Empty / whitespace-only tokens return null (caller should drop them).
 */
export function parseSubscriptionToken(raw: string): UploaderSubscription | null {
  const trimmed = raw.trim();
  if (trimmed.length === 0) return null;

  // `q:` / `query:` prefix → arbitrary search query. We match the prefix
  // separately from the body so an empty query (`q:`, `query:   `) returns
  // null rather than falling through to "user".
  const prefixMatch = trimmed.match(/^(q|query):(.*)$/i);
  if (prefixMatch) {
    const q = (prefixMatch[2] ?? "").trim();
    if (q.length === 0) return null;
    return { kind: "query", identifier: q };
  }

  // Plain identifier → username feed.
  return { kind: "user", identifier: trimmed };
}

/**
 * Parse the admin `uploaders` CSV into a clean list of subscriptions.
 * Skips empty tokens; preserves order; deduplicates.
 */
export function parseSubscriptionList(raw: unknown): UploaderSubscription[] {
  if (typeof raw !== "string") return [];
  const seen = new Set<string>();
  const out: UploaderSubscription[] = [];
  for (const token of raw.split(",")) {
    const sub = parseSubscriptionToken(token);
    if (sub === null) continue;
    const key = `${sub.kind}:${sub.identifier.toLowerCase()}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(sub);
  }
  return out;
}

/** Build the per-subscription RSS URL. */
export function feedUrl(
  subscription: UploaderSubscription,
  baseUrl: string = NYAA_BASE_URL,
): string {
  const base = baseUrl.replace(/\/+$/, "");
  if (subscription.kind === "user") {
    return `${base}/?page=rss&u=${encodeURIComponent(subscription.identifier)}`;
  }
  return `${base}/?page=rss&q=${encodeURIComponent(subscription.identifier)}`;
}

/**
 * Conditional GET against an uploader-subscription RSS feed.
 *
 * @param subscription - The uploader subscription to fetch.
 * @param previousEtag - The ETag from the previous successful poll (if any).
 * @param previousLastModified - Optional Last-Modified header from the previous
 *   poll. Nyaa often returns one but doesn't always honor `If-None-Match`;
 *   sending both maximizes 304 hit rate.
 * @param opts - Fetcher options (custom fetch, timeout, base URL override).
 */
export async function fetchSubscriptionFeed(
  subscription: UploaderSubscription,
  previousEtag: string | null,
  previousLastModified: string | null,
  opts: FetcherOptions = {},
): Promise<FetchResult> {
  const fetchImpl = opts.fetchImpl ?? globalThis.fetch;
  const timeoutMs = opts.timeoutMs ?? 10_000;
  const baseUrl = opts.baseUrl ?? NYAA_BASE_URL;

  const url = feedUrl(subscription, baseUrl);
  const headers: Record<string, string> = {
    Accept: "application/rss+xml, application/xml;q=0.9, */*;q=0.5",
    "User-Agent": "Codex-ReleaseTracker/1.0 (+https://github.com/AshDevFr/codex)",
  };
  if (previousEtag) {
    headers["If-None-Match"] = previousEtag;
  }
  if (previousLastModified) {
    headers["If-Modified-Since"] = previousLastModified;
  }

  const signal = AbortSignal.timeout(timeoutMs);

  let resp: Response;
  try {
    resp = await fetchImpl(url, { method: "GET", headers, signal });
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Unknown fetch error";
    return { kind: "error", status: 0, message: msg };
  }

  if (resp.status === 304) {
    return { kind: "notModified", status: 304 };
  }

  if (resp.status === 200) {
    const body = await resp.text();
    const etag = resp.headers.get("etag");
    const lastModified = resp.headers.get("last-modified");
    return { kind: "ok", body, etag, lastModified, status: 200 };
  }

  return {
    kind: "error",
    status: resp.status,
    message: `upstream returned ${resp.status} ${resp.statusText}`,
  };
}
