/**
 * RSS parser for MangaUpdates per-series feeds.
 *
 * Per-series feed: `https://api.mangaupdates.com/v1/series/{series_id}/rss`
 *
 * The v1 RSS feed is intentionally sparse:
 *   - `<title>` carries `{Series Name} {v.N}? {c.N}` — chapter and/or volume
 *     suffixed with optional letter (`c.113a`, `c.113b` for split chapters)
 *   - `<description>` carries the scanlation group name
 *   - per-item `<link>`, `<guid>`, `<pubDate>` are NOT present; only the
 *     channel-level `<link>` (the series page on mangaupdates.com) exists
 *
 * Items that carry neither chapter nor volume info are dropped — they're
 * usually announcements ("oneshot release", series-name-only entries) and
 * have no place in an inbox.
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
  /** ISO-8601 upstream publish date (from `<pubDate>`), or null when the
   *  feed carried no usable date. This is the release date, not detection. */
  releasedAt: string | null;
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

  // Chapter: c.N or ch.N. Decimals (`47.5`) and letter suffixes (`113a`,
  // `113b` for split chapters) are both supported; the letter suffix is
  // stripped so `c.113a` and `c.113b` map to chapter 113. Letter-suffix
  // variants get distinct externalReleaseIds via the group, so they remain
  // separate ledger rows even though they share an integer. The lookahead
  // (`(?![0-9])`) replaces the older `\b` so the trailing letter doesn't
  // block the match the way `\b` does between two word characters.
  let chapter: number | null = null;
  const chMatch = trimmed.match(/\bc(?:h)?\.?\s*([0-9]+(?:\.[0-9]+)?)[a-z]?(?![0-9])/i);
  if (chMatch?.[1]) {
    const n = Number.parseFloat(chMatch[1]);
    if (Number.isFinite(n)) chapter = n;
  }

  // Volume: v.N or vol.N. Letter suffixes accepted and discarded for the
  // same reason as chapters.
  let volume: number | null = null;
  const volMatch = trimmed.match(/\bv(?:ol)?\.?\s*([0-9]+)[a-z]?(?![0-9])/i);
  if (volMatch?.[1]) {
    const n = Number.parseInt(volMatch[1], 10);
    if (Number.isFinite(n)) volume = n;
  }

  // Group: legacy "by <Group>" pattern. The current MangaUpdates v1 RSS
  // feed places the scanlation group in `<description>`, not the title;
  // this branch is kept as a fallback so older / legacy feed shapes still
  // surface a group. Captured up to `(` or end-of-string so a trailing
  // `(en)` language tag doesn't bleed into the group name.
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
 * style dates (`Mon, 04 May 2026 02:31:00 GMT`). Returns `null` on missing or
 * invalid input — the release date is unknown then, so the host stores NULL
 * rather than a misleading fallback. Never throws: one bad pubDate shouldn't
 * drop the whole feed.
 */
function pubDateToIso(raw: string | null): string | null {
  if (raw) {
    const d = new Date(raw);
    if (!Number.isNaN(d.getTime())) return d.toISOString();
  }
  return null;
}

/**
 * Derive a stable external_release_id.
 *
 * Priority:
 *   1. `<guid>` if present (richest legacy format).
 *   2. `<link>` if present (legacy format with per-item links).
 *   3. Deterministic hash of `(title + group + pubDate)` for the current
 *      v1 RSS shape, which carries none of the above per-item fields.
 *      Including the group in the hash is what lets multiple groups
 *      releasing the same chapter ("c.200" by Asura, by FLAME-SCANS,
 *      by LeviatanScans) hash to distinct IDs and become distinct
 *      ledger rows. Same-group same-chapter re-polls collide on the
 *      hash and dedupe, which is what the host expects.
 */
function deriveExternalReleaseId(
  guid: string | null,
  link: string | null,
  title: string,
  group: string | null,
  pubDate: string | null,
): string {
  if (guid && guid.trim().length > 0) return guid.trim();
  if (link && link.trim().length > 0) return link.trim();
  const fallback = `${title}|${group ?? ""}|${pubDate ?? ""}`;
  let h = 5381;
  for (let i = 0; i < fallback.length; i++) {
    h = ((h << 5) + h + fallback.charCodeAt(i)) | 0;
  }
  return `t:${(h >>> 0).toString(36)}`;
}

/**
 * Parse a single MangaUpdates `<item>` block into a `ParsedRssItem`. Returns
 * null when the item is unusable:
 *   - missing `<title>` (truly malformed), or
 *   - title carries neither chapter nor volume (announcements, oneshot
 *     stubs, series-name-only entries — pure inbox noise).
 */
export function parseItem(itemXml: string): ParsedRssItem | null {
  const title = extractTagText(itemXml, "title");
  if (!title) return null;

  const link = extractTagText(itemXml, "link");
  const guid = extractTagText(itemXml, "guid");
  const pubDate = extractTagText(itemXml, "pubDate");
  const description = extractTagText(itemXml, "description");

  const { chapter, volume, group: groupFromTitle, language } = parseTitle(title);
  if (chapter === null && volume === null) return null;

  // The v1 RSS feed places the scanlation group in `<description>`. Prefer
  // it; fall back to the legacy "by <Group>" title pattern.
  const descTrimmed = description?.trim();
  const group = descTrimmed && descTrimmed.length > 0 ? descTrimmed : groupFromTitle;

  return {
    externalReleaseId: deriveExternalReleaseId(guid, link, title, group, pubDate),
    title,
    chapter,
    volume,
    group,
    language,
    link: link ?? "",
    releasedAt: pubDateToIso(pubDate),
  };
}

/** Parsed feed: items plus the channel-level link (if any). */
export interface ParsedFeed {
  /** Channel-level `<link>` — the series page on mangaupdates.com. Used as
   *  the `payloadUrl` for releases when no per-item link exists (the v1
   *  RSS shape). `null` when the channel block is missing or malformed. */
  channelLink: string | null;
  items: ParsedRssItem[];
}

/**
 * Parse a full MangaUpdates per-series RSS feed body. Items that fail
 * `parseItem` (missing title, or no chapter/volume) are dropped silently —
 * the feed parser is best-effort tolerant.
 */
export function parseFeed(xml: string): ParsedFeed {
  return {
    channelLink: extractChannelLink(xml),
    items: splitItems(xml)
      .map(parseItem)
      .filter((i): i is ParsedRssItem => i !== null),
  };
}

/**
 * Extract the channel-level `<link>` from a feed. The v1 RSS feed uses
 * `<channel><link>https://...</link></channel>` and that URL is the series
 * page on mangaupdates.com. We prefer the first `<link>` *outside* any
 * `<item>` block so per-item legacy links (which we don't expect at the
 * channel level anyway) can never bleed in.
 */
function extractChannelLink(xml: string): string | null {
  // Strip every <item>...</item> block before searching — cheap way to
  // scope to the channel header.
  const stripped = xml.replace(/<item\b[^>]*>[\s\S]*?<\/item>/gi, "");
  const link = extractTagText(stripped, "link");
  if (!link) return null;
  const trimmed = link.trim();
  return trimmed.length > 0 ? trimmed : null;
}
