/**
 * MangaUpdates per-series RSS fetcher.
 *
 * Wraps `fetch` with conditional GET (`If-None-Match` from a stored ETag) and
 * a hard timeout. Returns a discriminated result so the caller can:
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
  | { kind: "ok"; body: string; etag: string | null; status: 200 }
  | { kind: "notModified"; status: 304 }
  | { kind: "error"; status: number; message: string };

export interface FetcherOptions {
  /** Custom `fetch` impl (for testing). Defaults to global `fetch`. */
  fetchImpl?: typeof fetch;
  /** Per-request timeout. Defaults to 10s. */
  timeoutMs?: number;
}

/** Public base URL for MangaUpdates' v1 RSS API. */
export const MANGAUPDATES_RSS_BASE = "https://api.mangaupdates.com/v1/series";

/**
 * Normalize a MangaUpdates series ID to its numeric form for API calls.
 *
 * MangaUpdates uses two interchangeable representations of the same ID:
 *
 *   - **Numeric** (e.g. `15180124327`) — the internal primary key. Every
 *     `/v1/series/...` API endpoint requires this form.
 *   - **Base36 slug** (e.g. `6z1uqw7`) — a base36 encoding of the numeric
 *     ID, used in public URLs only (`mangaupdates.com/series/6z1uqw7/...`).
 *     The API rejects this form with a 405.
 *
 * Metadata sources (MangaBaka, etc.) typically scrape the public URL and
 * store the slug, so the value we receive on `entry.externalIds.mangaupdates`
 * is whatever the source happened to grab. Decode here so callers don't
 * have to know.
 *
 * Returns the input unchanged when it's already an all-digit string;
 * `null` when the input contains characters outside the base36 alphabet
 * (caller should surface as a configuration error).
 */
export function normalizeMangaUpdatesId(raw: string): string | null {
  const trimmed = raw.trim();
  if (trimmed.length === 0) return null;
  if (/^\d+$/.test(trimmed)) return trimmed;
  if (!/^[0-9a-z]+$/i.test(trimmed)) return null;
  // parseInt('6z1uqw7', 36) = 15180124327. JS numbers are precise for
  // integers up to 2^53; MangaUpdates IDs sit well below that.
  const decoded = Number.parseInt(trimmed, 36);
  if (!Number.isFinite(decoded) || decoded <= 0) return null;
  return String(decoded);
}

/**
 * Build the per-series RSS URL. Accepts either the numeric ID or the
 * base36 slug — see `normalizeMangaUpdatesId` for the rationale.
 */
export function feedUrl(mangaUpdatesId: string): string {
  const normalized = normalizeMangaUpdatesId(mangaUpdatesId) ?? mangaUpdatesId;
  return `${MANGAUPDATES_RSS_BASE}/${normalized}/rss`;
}

/**
 * Conditional GET against a per-series RSS feed.
 *
 * @param mangaUpdatesId - The MangaUpdates series ID.
 * @param previousEtag - The ETag from the previous successful poll (if any).
 * @param opts - Fetcher options (custom fetch, timeout).
 */
export async function fetchSeriesFeed(
  mangaUpdatesId: string,
  previousEtag: string | null,
  opts: FetcherOptions = {},
): Promise<FetchResult> {
  const fetchImpl = opts.fetchImpl ?? globalThis.fetch;
  const timeoutMs = opts.timeoutMs ?? 10_000;

  const url = feedUrl(mangaUpdatesId);
  const headers: Record<string, string> = {
    Accept: "application/rss+xml, application/xml;q=0.9, */*;q=0.5",
    "User-Agent": "Codex-ReleaseTracker/1.0 (+https://github.com/AshDevFr/codex)",
  };
  if (previousEtag) {
    headers["If-None-Match"] = previousEtag;
  }

  // AbortSignal.timeout is the cleanest path. Falling back to a manual
  // controller would add complexity without value (we already require Node
  // 22+).
  const signal = AbortSignal.timeout(timeoutMs);

  let resp: Response;
  try {
    resp = await fetchImpl(url, { method: "GET", headers, signal });
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Unknown fetch error";
    // Treat aborts and other transport-level failures as 0/unavailable so
    // the host's per-host backoff layer can detect "this domain is sad
    // right now" without us having to invent a fake HTTP status.
    return { kind: "error", status: 0, message: msg };
  }

  if (resp.status === 304) {
    return { kind: "notModified", status: 304 };
  }

  if (resp.status === 200) {
    const body = await resp.text();
    const etag = resp.headers.get("etag");
    return { kind: "ok", body, etag, status: 200 };
  }

  // Pass through 429 / 5xx so the host's backoff layer sees the real status.
  return {
    kind: "error",
    status: resp.status,
    message: `upstream returned ${resp.status} ${resp.statusText}`,
  };
}
