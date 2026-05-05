/**
 * RSS parser for MangaUpdates per-series feeds.
 *
 * Per-series feed: `https://api.mangaupdates.com/v1/series/{series_id}/rss`
 *
 * Each `<item>` is one scanlation release. The plugin extracts:
 *   - chapter / volume from the title
 *   - scanlation group from the title
 *   - language tag (parenthesized two-letter code) from the title
 *   - link (the MangaUpdates release page) used as `payloadUrl`
 *   - pubDate as `observedAt`
 *
 * Implementation note: we do NOT pull in a heavy XML parser. The MangaUpdates
 * RSS format is simple, well-formed, and stable. A small targeted regex
 * pipeline avoids a 100kb dependency and CVE surface for marginal benefit.
 */

/** Parsed item, pre-`ReleaseCandidate`. */
export interface ParsedRssItem {
  /** Stable per-source ID. Derived from the release URL or guid. */
  externalReleaseId: string;
  /** Original title string. Useful for debugging / fallback. */
  title: string;
  /** Chapter number (decimals supported, e.g. "47.5"). */
  chapter: number | null;
  /** Volume number. */
  volume: number | null;
  /**
   * Language tag (lowercased ISO 639-1). Defaults to `"en"` when the title
   * doesn't carry an explicit `(xx)` code, since the MangaUpdates v1 RSS
   * endpoint serves the English release stream. The legacy
   * `UNKNOWN_LANGUAGE` sentinel is still exported for callers that want
   * to surface "no tag detected" explicitly, but the parser no longer
   * produces it on its own.
   */
  language: string;
  /** Scanlation group name (best-effort; nullable). */
  group: string | null;
  /** Release page URL on MangaUpdates. Used as `payloadUrl`. */
  link: string;
  /** ISO-8601 string. Falls back to "now" when pubDate is missing/invalid. */
  observedAt: string;
}

/** Sentinel returned when the language tag can't be detected. */
export const UNKNOWN_LANGUAGE = "unknown" as const;

// -----------------------------------------------------------------------------
// XML helpers
// -----------------------------------------------------------------------------

/** Strip CDATA wrapper if present, unescape `&amp;` `&lt;` `&gt;` `&quot;`. */
function decodeXmlText(raw: string): string {
  let s = raw.trim();
  const cdataMatch = s.match(/^<!\[CDATA\[([\s\S]*?)]]>$/);
  if (cdataMatch?.[1] !== undefined) {
    s = cdataMatch[1];
  }
  return s
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&apos;/g, "'");
}

/** Pull the first `<tag>` text content from an XML fragment, or null. */
function extractTagText(xml: string, tag: string): string | null {
  const re = new RegExp(`<${tag}[^>]*>([\\s\\S]*?)</${tag}>`, "i");
  const m = xml.match(re);
  if (!m?.[1]) return null;
  return decodeXmlText(m[1]);
}

/** Pull all `<item>...</item>` blocks from a feed. */
function splitItems(xml: string): string[] {
  const out: string[] = [];
  const re = /<item\b[^>]*>([\s\S]*?)<\/item>/gi;
  for (;;) {
    const match = re.exec(xml);
    if (match === null) break;
    if (match[1] !== undefined) out.push(match[1]);
  }
  return out;
}

// -----------------------------------------------------------------------------
// Title parsing
// -----------------------------------------------------------------------------

/**
 * Extract chapter/volume/group/language from a MangaUpdates RSS title.
 *
 * Observed shapes:
 *   "Vol.2 c.14 by GroupName (en)"
 *   "v.2 c.14.5 by GroupName (es)"
 *   "c.143 by GroupName"                       (language missing)
 *   "Vol.15 by GroupName (en)"                 (volume-only bundle)
 *   "c.143 (en)"                               (no group)
 *
 * Volume tokens: `v.N`, `vol.N`, `Vol.N` (case-insensitive).
 * Chapter tokens: `c.N`, `ch.N`, `Ch.N` (decimals allowed).
 * Group: text between `by ` and the next `(` or end-of-string.
 * Language: trailing `(xx)` two-letter code, lowercased.
 */
export function parseTitle(title: string): {
  chapter: number | null;
  volume: number | null;
  group: string | null;
  language: string;
} {
  const trimmed = title.trim();

  // Chapter: c.N or ch.N (allow decimals).
  let chapter: number | null = null;
  const chMatch = trimmed.match(/\bc(?:h)?\.?\s*([0-9]+(?:\.[0-9]+)?)\b/i);
  if (chMatch?.[1]) {
    const n = Number.parseFloat(chMatch[1]);
    if (Number.isFinite(n)) chapter = n;
  }

  // Volume: v.N or vol.N.
  let volume: number | null = null;
  const volMatch = trimmed.match(/\bv(?:ol)?\.?\s*([0-9]+)\b/i);
  if (volMatch?.[1]) {
    const n = Number.parseInt(volMatch[1], 10);
    if (Number.isFinite(n)) volume = n;
  }

  // Group: "by <Group>" up to "(" or end.
  let group: string | null = null;
  const groupMatch = trimmed.match(/\bby\s+(.+?)(?:\s*\([a-z]{2,3}\)\s*)?$/i);
  if (groupMatch?.[1]) {
    const candidate = groupMatch[1].trim();
    if (candidate.length > 0) group = candidate;
  }

  // Language: trailing parenthesized 2-3 letter code (e.g. (en), (es), (id), (por)).
  //
  // The current MangaUpdates v1 RSS endpoint (`/v1/series/{id}/rss`) ships
  // titles without a language tag — it's the English-localized release
  // stream by design. Default to `"en"` so items aren't dropped by the
  // client-side language gate; an explicit `(es)` / `(id)` / etc. still
  // wins when present, and the host's per-series language list remains
  // the authoritative gate downstream. The legacy `UNKNOWN_LANGUAGE`
  // sentinel is kept exported for backwards compatibility but no longer
  // produced by this parser.
  let language = "en";
  const langMatch = trimmed.match(/\(([a-z]{2,3})\)\s*$/i);
  if (langMatch?.[1]) {
    language = langMatch[1].toLowerCase();
  }

  return { chapter, volume, group, language };
}

// -----------------------------------------------------------------------------
// Item parsing
// -----------------------------------------------------------------------------

/**
 * Best-effort `pubDate` -> ISO-8601 conversion. MangaUpdates uses RFC-2822
 * style dates (`Mon, 04 May 2026 02:31:00 GMT`). Falls back to "now" on
 * invalid input — never throws, since one bad pubDate shouldn't drop the
 * whole feed.
 */
function pubDateToIso(raw: string | null): string {
  if (raw) {
    const d = new Date(raw);
    if (!Number.isNaN(d.getTime())) return d.toISOString();
  }
  return new Date().toISOString();
}

/**
 * Derive a stable external_release_id. Prefer `<guid>`, then the link URL,
 * otherwise fall back to a deterministic hash of `(title + pubDate)`.
 *
 * Stability is what matters: re-polling the same item must produce the same
 * ID so the host's `(source_id, external_release_id)` dedup catches it.
 */
function deriveExternalReleaseId(
  guid: string | null,
  link: string | null,
  title: string,
  pubDate: string | null,
): string {
  if (guid && guid.trim().length > 0) return guid.trim();
  if (link && link.trim().length > 0) return link.trim();
  // Deterministic fallback for feeds that omit both. djb2-ish hash keeps the
  // ID short while staying stable across polls.
  const fallback = `${title}|${pubDate ?? ""}`;
  let h = 5381;
  for (let i = 0; i < fallback.length; i++) {
    h = ((h << 5) + h + fallback.charCodeAt(i)) | 0;
  }
  return `t:${(h >>> 0).toString(36)}`;
}

/**
 * Parse a single MangaUpdates `<item>` block into a `ParsedRssItem`. Returns
 * null if the title is missing entirely (truly malformed item).
 */
export function parseItem(itemXml: string): ParsedRssItem | null {
  const title = extractTagText(itemXml, "title");
  if (!title) return null;

  const link = extractTagText(itemXml, "link");
  const guid = extractTagText(itemXml, "guid");
  const pubDate = extractTagText(itemXml, "pubDate");

  const { chapter, volume, group, language } = parseTitle(title);

  return {
    externalReleaseId: deriveExternalReleaseId(guid, link, title, pubDate),
    title,
    chapter,
    volume,
    group,
    language,
    link: link ?? "",
    observedAt: pubDateToIso(pubDate),
  };
}

/**
 * Parse a full MangaUpdates per-series RSS feed body into items. Bad items
 * (missing title) are dropped silently — the feed should be best-effort
 * tolerant.
 */
export function parseFeed(xml: string): ParsedRssItem[] {
  return splitItems(xml)
    .map(parseItem)
    .filter((i): i is ParsedRssItem => i !== null);
}
