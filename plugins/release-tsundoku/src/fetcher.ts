/**
 * Tsundoku series-feed fetcher.
 *
 * Wraps `fetch` against `POST {baseUrl}/api/v1/series/feed` with a hard
 * timeout and JSON parsing, returning a discriminated result so the caller
 * can act on a parsed page (`ok`) or surface the upstream status back to the
 * host's per-host backoff layer (`error`).
 *
 * We use the filtered `POST` variant — the body carries the consumer's
 * `provider:externalId` set so the feed returns only the tracked series, not
 * the whole catalog. The response is keyset-paginated: walk while `hasMore` is
 * true, passing `nextCursor` back as `cursor`. That cursor paginates *within a
 * single poll* and is not persisted — each poll re-walks the tracked set's
 * current coverage and relies on host-side dedup. Network and parsing are the
 * only side effects, which keeps it trivially testable with a mocked `fetch`.
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

/** Body for one `POST /series/feed` page. */
export interface FeedRequest {
  /**
   * `provider:externalId` filter — the feed is narrowed to series carrying one
   * of these. Must be non-empty (an empty list means "no filter" upstream,
   * i.e. the whole catalog — callers guard against that).
   */
  externalIds: string[];
  /** Pagination cursor within this poll. `null` starts at the beginning. */
  cursor: string | null;
  /** Page size (the caller clamps to 1..=500). */
  limit: number;
}

/** Build the feed endpoint URL (trailing slashes on `baseUrl` tolerated). */
export function feedUrl(baseUrl: string): string {
  return `${baseUrl.replace(/\/+$/, "")}${FEED_PATH}`;
}

/**
 * Fetch one page of the filtered Tsundoku series feed via `POST`.
 *
 * We post the tracked `externalIds` set so the feed returns only the
 * consumer's series (not the whole catalog). The `cursor` is for pagination
 * *within a single poll* — it is not persisted across polls; each poll walks
 * the current coverage of the tracked set and relies on host-side dedup to
 * suppress unchanged releases.
 *
 * @param baseUrl - Tsundoku instance base URL (trailing slash tolerated).
 * @param req - Filter set + pagination cursor + page size.
 * @param opts - Fetcher options (custom fetch, timeout).
 */
export async function fetchFeedPage(
  baseUrl: string,
  req: FeedRequest,
  opts: FeedFetcherOptions = {},
): Promise<FeedFetchResult> {
  const fetchImpl = opts.fetchImpl ?? globalThis.fetch;
  const timeoutMs = opts.timeoutMs ?? DEFAULT_TIMEOUT_MS;

  const url = feedUrl(baseUrl);
  const headers: Record<string, string> = {
    Accept: "application/json",
    "Content-Type": "application/json",
    "User-Agent": "Codex-ReleaseTracker/1.0 (+https://github.com/AshDevFr/codex)",
  };
  const body = JSON.stringify({
    externalIds: req.externalIds,
    cursor: req.cursor,
    limit: req.limit,
  });

  // AbortSignal.timeout is the cleanest path; we already require Node 22+.
  const signal = AbortSignal.timeout(timeoutMs);

  let resp: Response;
  try {
    resp = await fetchImpl(url, { method: "POST", headers, body, signal });
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
