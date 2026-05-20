import type { BookCondition, SeriesCondition } from "@/types/filters";

/**
 * URL state for the `/search` page.
 *
 * Encoded into URLSearchParams so the page is bookmarkable and shareable.
 * The `condition` is JSON-then-base64url-encoded so arbitrary nested
 * conditions survive a copy/paste without URL-decoding accidents.
 *
 * Lossy fallback: if the encoded `condition` exceeds `MAX_URL_CONDITION_LEN`,
 * encoders should skip writing it and the caller is expected to nudge the
 * user toward saving a preset instead. Reading silently ignores a missing
 * `c` param (treats as no condition).
 */
export interface SearchUrlState {
  query: string;
  sort: string;
  tab: "series" | "books";
  page: number;
  /** Optional structured condition for the active tab. */
  condition?: SeriesCondition | BookCondition;
}

export interface SearchUrlEncodeOptions {
  /**
   * Maximum length of the encoded `c` parameter before it gets dropped from
   * the URL. Most browsers and servers handle ~2k characters fine; we cap
   * a bit lower to leave room for the other params. Tunable for testing.
   */
  maxConditionLength?: number;
}

export const DEFAULT_MAX_URL_CONDITION_LEN = 1800;
export const DEFAULT_SEARCH_PAGE_SIZE = 50;

const URL_PARAMS = {
  query: "q",
  sort: "sort",
  tab: "tab",
  page: "page",
  condition: "c",
} as const;

/**
 * base64url encode: standard base64 with `+` → `-`, `/` → `_`, and trailing
 * `=` padding stripped so the result is URL-safe.
 */
function toBase64Url(input: string): string {
  const bytes = new TextEncoder().encode(input);
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

function fromBase64Url(input: string): string {
  const padded = input.replace(/-/g, "+").replace(/_/g, "/");
  const padLen = padded.length % 4;
  const padding = padLen === 0 ? "" : "=".repeat(4 - padLen);
  const binary = atob(padded + padding);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return new TextDecoder().decode(bytes);
}

export function encodeCondition(
  condition: SeriesCondition | BookCondition,
): string {
  return toBase64Url(JSON.stringify(condition));
}

export function decodeCondition<T = SeriesCondition | BookCondition>(
  encoded: string,
): T | null {
  try {
    const json = fromBase64Url(encoded);
    return JSON.parse(json) as T;
  } catch {
    return null;
  }
}

/**
 * Serialize the search state to URLSearchParams. Skips empty/default fields
 * so the URL stays as short as possible. Returns a tuple of the params and a
 * flag indicating whether the condition was dropped due to length.
 */
export function serializeSearchUrl(
  state: SearchUrlState,
  options: SearchUrlEncodeOptions = {},
): { params: URLSearchParams; conditionDropped: boolean } {
  const params = new URLSearchParams();
  let conditionDropped = false;

  if (state.query.trim().length > 0) {
    params.set(URL_PARAMS.query, state.query);
  }
  if (state.sort) {
    params.set(URL_PARAMS.sort, state.sort);
  }
  if (state.tab !== "series") {
    params.set(URL_PARAMS.tab, state.tab);
  }
  if (state.page > 1) {
    params.set(URL_PARAMS.page, String(state.page));
  }
  if (state.condition !== undefined) {
    const encoded = encodeCondition(state.condition);
    const limit = options.maxConditionLength ?? DEFAULT_MAX_URL_CONDITION_LEN;
    if (encoded.length <= limit) {
      params.set(URL_PARAMS.condition, encoded);
    } else {
      conditionDropped = true;
    }
  }

  return { params, conditionDropped };
}

/**
 * Parse URLSearchParams into a typed SearchUrlState. Missing or malformed
 * values fall back to sensible defaults so the page can always render.
 */
export function parseSearchUrl(params: URLSearchParams): SearchUrlState {
  const tabParam = params.get(URL_PARAMS.tab);
  const tab: "series" | "books" = tabParam === "books" ? "books" : "series";

  const pageRaw = params.get(URL_PARAMS.page);
  const pageParsed = pageRaw ? Number.parseInt(pageRaw, 10) : 1;
  const page = Number.isFinite(pageParsed) && pageParsed >= 1 ? pageParsed : 1;

  const encoded = params.get(URL_PARAMS.condition);
  const condition = encoded
    ? (decodeCondition(encoded) ?? undefined)
    : undefined;

  return {
    query: params.get(URL_PARAMS.query) ?? "",
    sort: params.get(URL_PARAMS.sort) ?? "",
    tab,
    page,
    condition: condition ?? undefined,
  };
}

/**
 * Effective sort for an outgoing list request. Empty value lets the backend
 * pick: when a query is present that resolves to "relevance"; otherwise the
 * natural default per target ("name,asc" for series, "title,asc" for books).
 */
export function effectiveSort(state: SearchUrlState): string {
  return state.sort;
}
