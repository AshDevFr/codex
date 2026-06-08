/**
 * Tsundoku series-feed fetcher.
 *
 * Wraps `fetch` against `GET {baseUrl}/api/v1/series/feed` with a hard
 * timeout and JSON parsing, returning a discriminated result so the caller
 * can act on a parsed page (`ok`) or surface the upstream status back to the
 * host's per-host backoff layer (`error`).
 *
 * The feed is keyset-paginated: pass the previous response's `nextCursor`
 * back as `cursor` and walk while `hasMore` is true. Network and parsing are
 * the only side effects; nothing here touches storage, the host, or process
 * state, which keeps it trivially testable with a mocked `fetch`.
 */

// =============================================================================
// Wire types (mirror Tsundoku's SeriesFeedResponse / SeriesFeedItem)
// =============================================================================

/** One provider mapping on a feed item (e.g. `{ provider: "mangabaka", ... }`). */
export interface FeedExternalId {
  provider: string;
  externalId: string;
  /** Epoch seconds the mapping was last fetched upstream. */
  fetchedAt: number;
}

/** One inclusive `[start, end]` coverage range (single values are `start === end`). */
export interface FeedCoverageSpan {
  start: number;
  end: number;
}

/** One series in the incremental release feed. */
export interface FeedItem {
  seriesId: number;
  canonicalTitle: string;
  /** Provider mappings the consumer matches on. */
  externalIds: FeedExternalId[];
  /** Merged available volume ranges (sorted, gaps preserved). */
  volumeCoverage: FeedCoverageSpan[];
  /** Merged available chapter ranges (sorted, gaps preserved). */
  chapterCoverage: FeedCoverageSpan[];
  /** Max end of `volumeCoverage`, or null when there is none. */
  highestVolume: number | null;
  /** Max end of `chapterCoverage`, or null when there is none. */
  highestChapter: number | null;
  /** Epoch seconds this series' coverage last changed (the cursor key). */
  updatedAt: number;
}

/** One page of the feed. */
export interface FeedResponse {
  items: FeedItem[];
  /** `true` when more series remain after this page (fetch again now). */
  hasMore: boolean;
  /** Opaque cursor at the last item, or null/absent when the page is empty. */
  nextCursor?: string | null;
}

// =============================================================================
// Fetch result + options
// =============================================================================

/** Discriminated fetch result. */
export type FeedFetchResult =
  | { kind: "ok"; data: FeedResponse; status: 200 }
  | { kind: "error"; status: number; message: string };

export interface FeedFetcherOptions {
  /** Custom `fetch` impl (for testing). Defaults to global `fetch`. */
  fetchImpl?: typeof fetch;
  /** Per-request timeout. Defaults to 10s. */
  timeoutMs?: number;
}

/** Feed endpoint path appended to the configured base URL. */
export const FEED_PATH = "/api/v1/series/feed";

const DEFAULT_TIMEOUT_MS = 10_000;

/**
 * Build the feed URL for a page. Defensively strips trailing slashes off
 * `baseUrl` so callers don't have to. `limit` is always sent; `cursor` is
 * sent only when non-empty (its absence starts the walk from the beginning).
 */
export function feedUrl(baseUrl: string, cursor: string | null, limit: number): string {
  const base = baseUrl.replace(/\/+$/, "");
  const params = new URLSearchParams();
  params.set("limit", String(limit));
  if (cursor) {
    params.set("cursor", cursor);
  }
  return `${base}${FEED_PATH}?${params.toString()}`;
}

/**
 * Fetch one page of the Tsundoku series feed.
 *
 * @param baseUrl - Tsundoku instance base URL (trailing slash tolerated).
 * @param cursor - Cursor from the previous page, or null to start over.
 * @param limit - Page size (the caller is responsible for clamping to 1..=500).
 * @param opts - Fetcher options (custom fetch, timeout).
 */
export async function fetchFeedPage(
  baseUrl: string,
  cursor: string | null,
  limit: number,
  opts: FeedFetcherOptions = {},
): Promise<FeedFetchResult> {
  const fetchImpl = opts.fetchImpl ?? globalThis.fetch;
  const timeoutMs = opts.timeoutMs ?? DEFAULT_TIMEOUT_MS;

  const url = feedUrl(baseUrl, cursor, limit);
  const headers: Record<string, string> = {
    Accept: "application/json",
    "User-Agent": "Codex-ReleaseTracker/1.0 (+https://github.com/AshDevFr/codex)",
  };

  // AbortSignal.timeout is the cleanest path; we already require Node 22+.
  const signal = AbortSignal.timeout(timeoutMs);

  let resp: Response;
  try {
    resp = await fetchImpl(url, { method: "GET", headers, signal });
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Unknown fetch error";
    // Aborts and transport-level failures map to 0/unavailable so the host's
    // per-host backoff can react without us inventing a fake HTTP status.
    return { kind: "error", status: 0, message: msg };
  }

  if (resp.status !== 200) {
    // Pass through 429 / 5xx so the host's backoff layer sees the real status.
    return {
      kind: "error",
      status: resp.status,
      message: `upstream returned ${resp.status} ${resp.statusText}`.trim(),
    };
  }

  let parsed: unknown;
  try {
    parsed = await resp.json();
  } catch (err) {
    const msg = err instanceof Error ? err.message : "invalid JSON";
    return { kind: "error", status: 200, message: `failed to parse feed JSON: ${msg}` };
  }

  if (!isFeedResponse(parsed)) {
    return { kind: "error", status: 200, message: "malformed feed response: missing items[]" };
  }

  return { kind: "ok", data: parsed, status: 200 };
}

/**
 * Minimal structural guard: a valid page must carry an `items` array and a
 * boolean `hasMore`. We don't deep-validate each item — the matcher tolerates
 * missing fields per-item rather than failing the whole page.
 */
function isFeedResponse(value: unknown): value is FeedResponse {
  if (value === null || typeof value !== "object") return false;
  const obj = value as Record<string, unknown>;
  return Array.isArray(obj.items) && typeof obj.hasMore === "boolean";
}
