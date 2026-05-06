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
 * One uploader subscription entry.
 *
 * Three shapes:
 *   - `user`   — pulls `?page=rss&u=<identifier>` (a Nyaa user feed).
 *   - `query`  — pulls `?page=rss&q=<identifier>` (a plain text search).
 *   - `params` — pulls `?page=rss&<params>` where `<params>` is an
 *     allowlisted set of Nyaa query keys (`q`, `c`, `f`). Used to express
 *     category / filter combinations like the Literature → English-translated
 *     view (`c=3_1`).
 */
export type UploaderSubscription =
  | { kind: "user"; identifier: string }
  | { kind: "query"; identifier: string }
  | { kind: "params"; identifier: string };

/**
 * Keys allowed through from a `q:?…` URL-style token. `page` is always
 * injected by the plugin and can't be overridden; anything not in this set
 * is silently dropped to keep the surface tight.
 */
const PARAMS_ALLOWLIST = new Set(["q", "c", "f", "u"]);

/**
 * Parse a `q:?key=value&…` body into a normalized, allowlisted query string.
 * Returns null when no allowlisted keys remain (caller drops the token).
 *
 * Normalization sorts params alphabetically so two tokens that differ only
 * in key order dedupe to the same identifier.
 */
function parseUrlParams(body: string): { kind: "user" | "params"; identifier: string } | null {
  const params = new URLSearchParams(body);
  const kept: [string, string][] = [];
  for (const [rawKey, rawValue] of params.entries()) {
    const key = rawKey.toLowerCase();
    if (!PARAMS_ALLOWLIST.has(key)) continue;
    const value = rawValue.trim();
    if (value.length === 0) continue;
    kept.push([key, value]);
  }
  if (kept.length === 0) return null;

  // If the *only* allowlisted key is `u`, collapse to a plain user token so
  // `q:?u=1r0n` dedupes against the bare `1r0n` form and reuses the same
  // URL-building branch.
  if (kept.length === 1 && kept[0]?.[0] === "u") {
    return { kind: "user", identifier: kept[0][1] };
  }

  kept.sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  const normalized = new URLSearchParams(kept).toString();
  return { kind: "params", identifier: normalized };
}

/**
 * Parse a single uploader subscription token.
 *
 * Tokens look like:
 *   - `1r0n`                      → user feed
 *   - `q:LuminousScans`           → plain search query
 *   - `query:Manga Group`         → plain search query (long form)
 *   - `q:?c=3_1&q=Berserk`        → URL-style params (allowlisted: q, c, f, u)
 *   - `query:?u=1r0n&c=3_1`       → URL-style params, treated as user feed
 *
 * The leading `?` after `q:` / `query:` is the opt-in switch into URL mode,
 * which keeps `q:c=3_1&q=Berserk` (no `?`) parsing as a literal search term
 * for backwards compatibility.
 *
 * Empty / whitespace-only tokens return null (caller should drop them).
 */
export function parseSubscriptionToken(raw: string): UploaderSubscription | null {
  const trimmed = raw.trim();
  if (trimmed.length === 0) return null;

  // `q:` / `query:` prefix → search query, in either plain or URL-params form.
  const prefixMatch = trimmed.match(/^(q|query):(.*)$/i);
  if (prefixMatch) {
    const body = (prefixMatch[2] ?? "").trim();
    if (body.length === 0) return null;

    if (body.startsWith("?")) {
      return parseUrlParams(body.slice(1));
    }
    return { kind: "query", identifier: body };
  }

  // Plain identifier → username feed.
  return { kind: "user", identifier: trimmed };
}

/**
 * Build a stable per-plugin source key for a subscription. Mirrors the
 * dedup key used in `parseSubscriptionList` so two ways of writing the
 * same subscription collapse to the same source row.
 *
 * Used by `releases/register_sources` (to declare the plugin-owned key for
 * each row) and as a fallback when reconstructing a subscription from a
 * source key whose `config` is missing. Lower-cased identifier preserves
 * the existing case-insensitive dedup behaviour.
 */
export function subscriptionToSourceKey(sub: UploaderSubscription): string {
  return `${sub.kind}:${sub.identifier.toLowerCase()}`;
}

/**
 * Inverse of `subscriptionToSourceKey`: parse a `kind:identifier` source key
 * back into a subscription. Returns null for unrecognized keys (older rows
 * from a previous plugin version, manual edits, etc.) so the caller can log
 * and skip without crashing the whole poll.
 *
 * Note: the identifier coming back is lower-cased (per the source key
 * convention). Nyaa is case-insensitive on usernames and search terms, so
 * the round-trip is lossless for our purposes.
 */
export function sourceKeyToSubscription(key: string): UploaderSubscription | null {
  const idx = key.indexOf(":");
  if (idx <= 0 || idx === key.length - 1) return null;
  const kind = key.slice(0, idx);
  const identifier = key.slice(idx + 1);
  if (kind === "user" || kind === "query" || kind === "params") {
    return { kind, identifier };
  }
  return null;
}

/**
 * Parse the admin `uploaders` config into a clean list of subscriptions.
 *
 * Accepts either a JSON array (preferred — what the manifest now declares) or
 * a legacy comma-separated string. The string path is retained so existing
 * stored configs and CLI/env-driven setups keep working without a migration.
 *
 * Skips empty tokens; preserves order; deduplicates case-insensitively.
 */
export function parseSubscriptionList(raw: unknown): UploaderSubscription[] {
  let tokens: string[];
  if (Array.isArray(raw)) {
    tokens = raw.filter((t): t is string => typeof t === "string");
  } else if (typeof raw === "string") {
    tokens = raw.split(",");
  } else {
    return [];
  }

  const seen = new Set<string>();
  const out: UploaderSubscription[] = [];
  for (const token of tokens) {
    const sub = parseSubscriptionToken(token);
    if (sub === null) continue;
    const key = subscriptionToSourceKey(sub);
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
  if (subscription.kind === "query") {
    return `${base}/?page=rss&q=${encodeURIComponent(subscription.identifier)}`;
  }
  // params: identifier is already a URL-encoded, allowlisted query string.
  return `${base}/?page=rss&${subscription.identifier}`;
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
