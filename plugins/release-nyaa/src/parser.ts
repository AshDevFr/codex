/**
 * RSS parser for Nyaa.si feeds.
 *
 * Nyaa's RSS namespace exposes one extra element per item that we care about
 * (`<nyaa:infoHash>`), plus the standard `<title>`, `<link>`, `<guid>`,
 * `<pubDate>`, and `<description>` fields. We pull all of them with the same
 * lightweight regex pipeline used for MangaUpdates — no heavy XML dep.
 *
 * Parsing the title is where most of the work is. Real-world examples
 * (sourced from production Nyaa feeds and the user's screenshot of 1r0n's
 * subscription):
 *
 *   "[1r0n] Boruto - Two Blue Vortex - Volume 02 (Digital) (1r0n)"
 *   "[1r0n] One Piece v107 (Digital)"
 *   "[1r0n] Chainsaw Man - Chapter 142 (Digital)"
 *   "[Group] Dandadan c126-142 (2024) (Digital)"
 *   "[Tankobon Blur] Solo Leveling Vol. 13 (2024) (Digital) (Tankobon Blur)"
 *   "Berserk Volume 42 (Digital)"
 *
 * The shape we want out of each item:
 *   - parsed series guess (alias-free string used for matching)
 *   - chapter / volume axes (decimals supported on chapter)
 *   - format hints (Digital / JXL / etc.)
 *   - uploader-tagged group (if encoded as a leading `[Group]` token)
 *
 * Nyaa titles are noisy; we keep parsing best-effort and surface confidence
 * downstream from the alias matcher rather than failing here.
 */

/** Parsed item, pre-`ReleaseCandidate`. */
export interface ParsedRssItem {
  /** Stable per-source ID. Derived from the link or guid. */
  externalReleaseId: string;
  /** Original title. Useful for debugging / fallback. */
  title: string;
  /** Series-name guess after stripping volume/chapter/group/format tokens. */
  seriesGuess: string;
  /** Chapter number (decimals supported). Null if untyped. */
  chapter: number | null;
  /** Trailing chapter of a chapter range (e.g. `c126-142` → 126..142). */
  chapterRangeEnd: number | null;
  /** Volume number. Null if untyped. */
  volume: number | null;
  /** Trailing volume of a volume range (e.g. `v01-14` → 1..14). */
  volumeRangeEnd: number | null;
  /** Leading `[Group]` token, if any. */
  group: string | null;
  /** Format hints as a small dictionary (digital, jxl, ...). */
  formatHints: Record<string, boolean>;
  /** Magnet/torrent link or release page URL. */
  link: string;
  /** `nyaa:infoHash` value, lowercased; null if missing. */
  infoHash: string | null;
  /** ISO-8601 timestamp. Falls back to "now" if pubDate is missing/invalid. */
  observedAt: string;
}

// -----------------------------------------------------------------------------
// XML helpers (mirror release-mangaupdates conventions)
// -----------------------------------------------------------------------------

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
  // Escape `:` for namespaced tags (e.g. `nyaa:infoHash`).
  const safeTag = tag.replace(/:/g, "\\:");
  const re = new RegExp(`<${safeTag}[^>]*>([\\s\\S]*?)</${safeTag}>`, "i");
  const m = xml.match(re);
  if (!m?.[1]) return null;
  return decodeXmlText(m[1]);
}

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
 * Strip a leading `[Group]` token off the title and return both pieces.
 * If the title has no leading bracketed token, returns `{ rest: title,
 * group: null }`.
 */
function extractLeadingGroup(title: string): { rest: string; group: string | null } {
  const m = title.match(/^\s*\[([^\]]+)\]\s*(.*)$/);
  if (!m?.[1]) return { rest: title, group: null };
  const group = m[1].trim();
  const rest = m[2] ?? "";
  return { rest, group: group.length > 0 ? group : null };
}

/**
 * Pull a chapter / chapter-range out of the noise.
 *
 * Accepts:
 *   - `c.143`, `ch.143`, `Chapter 143`, `chapter 143`
 *   - `c143`, `ch143` (no separator)
 *   - `c126-142` (range — we keep both ends)
 *   - decimals (`c.47.5`)
 */
function extractChapter(s: string): { chapter: number | null; chapterRangeEnd: number | null } {
  // Range: `c126-142` (also `ch.126-142`, `Chapter 126-142`)
  const rangeRe = /\b(?:c|ch|chapter)\.?\s*([0-9]+(?:\.[0-9]+)?)\s*[-–]\s*([0-9]+(?:\.[0-9]+)?)\b/i;
  const range = s.match(rangeRe);
  if (range?.[1] && range[2]) {
    const start = Number.parseFloat(range[1]);
    const end = Number.parseFloat(range[2]);
    if (Number.isFinite(start) && Number.isFinite(end)) {
      return { chapter: start, chapterRangeEnd: end };
    }
  }
  // Single: `c.143`, `c143`, `Chapter 143`. Exclude things like `c8000` that
  // look like a resolution/codec by capping at 5 digits — Nyaa chapters
  // seldom go above 9999.
  const singleRe = /\b(?:c|ch|chapter)\.?\s*([0-9]{1,4}(?:\.[0-9]+)?)\b/i;
  const single = s.match(singleRe);
  if (single?.[1]) {
    const n = Number.parseFloat(single[1]);
    if (Number.isFinite(n)) return { chapter: n, chapterRangeEnd: null };
  }
  return { chapter: null, chapterRangeEnd: null };
}

/**
 * Pull a volume / volume-range out of the noise.
 *
 * Accepts:
 *   - `v01`, `v1`, `vol.1`, `vol 1`, `Volume 1`, `Vol. 1`
 *   - ranges: `v01-14`, `Vol. 1-14`
 */
function extractVolume(s: string): { volume: number | null; volumeRangeEnd: number | null } {
  // Range first.
  const rangeRe = /\b(?:v|vol|volume)\.?\s*([0-9]+)\s*[-–]\s*([0-9]+)\b/i;
  const range = s.match(rangeRe);
  if (range?.[1] && range[2]) {
    const start = Number.parseInt(range[1], 10);
    const end = Number.parseInt(range[2], 10);
    if (Number.isFinite(start) && Number.isFinite(end)) {
      return { volume: start, volumeRangeEnd: end };
    }
  }
  const singleRe = /\b(?:v|vol|volume)\.?\s*([0-9]{1,4})\b/i;
  const single = s.match(singleRe);
  if (single?.[1]) {
    const n = Number.parseInt(single[1], 10);
    if (Number.isFinite(n)) return { volume: n, volumeRangeEnd: null };
  }
  return { volume: null, volumeRangeEnd: null };
}

/**
 * Walk the parenthesized tags in the title and extract format hints.
 *
 * Common Nyaa hints we care about:
 *   - `(Digital)` → `digital`
 *   - `(JXL)` → `jxl`
 *   - `(Mag-Z)` / `(Magazine)` → `magazine`
 *   - `(2024)` is a year, ignored (we'd need it for naming dedup but not for filtering)
 */
function extractFormatHints(s: string): Record<string, boolean> {
  const hints: Record<string, boolean> = {};
  const tagRe = /\(([^)]+)\)/g;
  for (;;) {
    const match = tagRe.exec(s);
    if (match === null) break;
    const tag = (match[1] ?? "").trim().toLowerCase();
    if (tag.length === 0) continue;
    if (tag === "digital") hints.digital = true;
    else if (tag === "jxl") hints.jxl = true;
    else if (tag === "magazine" || tag === "mag-z") hints.magazine = true;
    else if (tag === "webtoon") hints.webtoon = true;
    else if (tag === "bw" || tag === "b&w") hints.bw = true;
    else if (tag === "color") hints.color = true;
  }
  return hints;
}

/**
 * Heuristic: strip everything that looks like a chapter/volume/format token,
 * a parenthesized tag, or a leading `[Group]` to expose a clean series-name
 * guess. The remaining string is alias-normalized downstream by the matcher.
 */
function extractSeriesGuess(input: string, group: string | null): string {
  let s = input;

  // Drop everything in (...) — format hints, year, group repeated.
  s = s.replace(/\([^)]*\)/g, " ");

  // Drop chapter/volume tokens (single or range).
  s = s.replace(
    /\b(?:c|ch|chapter)\.?\s*[0-9]+(?:\.[0-9]+)?(?:\s*[-–]\s*[0-9]+(?:\.[0-9]+)?)?\b/gi,
    " ",
  );
  s = s.replace(/\b(?:v|vol|volume)\.?\s*[0-9]+(?:\s*[-–]\s*[0-9]+)?\b/gi, " ");

  // Drop trailing/leading separator dashes used as titling glue (e.g.
  // `Boruto - Two Blue Vortex - Volume 02` → `Boruto Two Blue Vortex`).
  s = s.replace(/\s+[-–—]\s+/g, " ");

  // If the leading group token survived, drop it.
  if (group) {
    const groupRe = new RegExp(`\\[\\s*${escapeRegex(group)}\\s*\\]`, "gi");
    s = s.replace(groupRe, " ");
  }

  // Drop misc dotted-extension tokens (filenames sometimes leak through).
  s = s.replace(/\b\w+\.(?:cbz|cbr|epub|pdf|mobi|7z|zip)\b/gi, " ");

  // Collapse whitespace.
  return s.replace(/\s+/g, " ").trim();
}

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Public entry point — extract the structured fields from a single Nyaa
 * release title.
 *
 * Returns null only if the title is empty after trimming. Otherwise returns a
 * best-effort parse where the series guess may still be empty (e.g. for
 * meta-bundles without a leading series name); the matcher then drops those.
 */
export function parseTitle(title: string): {
  seriesGuess: string;
  chapter: number | null;
  chapterRangeEnd: number | null;
  volume: number | null;
  volumeRangeEnd: number | null;
  group: string | null;
  formatHints: Record<string, boolean>;
} | null {
  const trimmed = title.trim();
  if (trimmed.length === 0) return null;

  const { rest, group } = extractLeadingGroup(trimmed);
  const { chapter, chapterRangeEnd } = extractChapter(rest);
  const { volume, volumeRangeEnd } = extractVolume(rest);
  const formatHints = extractFormatHints(rest);
  const seriesGuess = extractSeriesGuess(rest, group);

  return {
    seriesGuess,
    chapter,
    chapterRangeEnd,
    volume,
    volumeRangeEnd,
    group,
    formatHints,
  };
}

// -----------------------------------------------------------------------------
// Item parsing
// -----------------------------------------------------------------------------

function pubDateToIso(raw: string | null): string {
  if (raw) {
    const d = new Date(raw);
    if (!Number.isNaN(d.getTime())) return d.toISOString();
  }
  return new Date().toISOString();
}

function deriveExternalReleaseId(
  guid: string | null,
  link: string | null,
  infoHash: string | null,
  title: string,
  pubDate: string | null,
): string {
  if (guid && guid.trim().length > 0) return guid.trim();
  if (link && link.trim().length > 0) return link.trim();
  if (infoHash && infoHash.length > 0) return `urn:btih:${infoHash}`;
  // Deterministic fallback: djb2-ish hash. Same algorithm MangaUpdates uses.
  const fallback = `${title}|${pubDate ?? ""}`;
  let h = 5381;
  for (let i = 0; i < fallback.length; i++) {
    h = ((h << 5) + h + fallback.charCodeAt(i)) | 0;
  }
  return `t:${(h >>> 0).toString(36)}`;
}

/**
 * Parse a single Nyaa `<item>` block. Returns null when the title is missing
 * (truly malformed entry).
 */
export function parseItem(itemXml: string): ParsedRssItem | null {
  const title = extractTagText(itemXml, "title");
  if (!title) return null;

  const link = extractTagText(itemXml, "link");
  const guid = extractTagText(itemXml, "guid");
  const pubDate = extractTagText(itemXml, "pubDate");
  const infoHashRaw = extractTagText(itemXml, "nyaa:infoHash");
  const infoHash = infoHashRaw ? infoHashRaw.toLowerCase().trim() : null;

  const parsedTitle = parseTitle(title);
  if (parsedTitle === null) return null;

  return {
    externalReleaseId: deriveExternalReleaseId(guid, link, infoHash, title, pubDate),
    title,
    seriesGuess: parsedTitle.seriesGuess,
    chapter: parsedTitle.chapter,
    chapterRangeEnd: parsedTitle.chapterRangeEnd,
    volume: parsedTitle.volume,
    volumeRangeEnd: parsedTitle.volumeRangeEnd,
    group: parsedTitle.group,
    formatHints: parsedTitle.formatHints,
    link: link ?? "",
    infoHash,
    observedAt: pubDateToIso(pubDate),
  };
}

/**
 * Parse a full Nyaa RSS feed body into structured items. Bad items (missing
 * title) are dropped silently — Nyaa feeds occasionally include broken entries
 * and we'd rather keep going than poison the whole poll.
 */
export function parseFeed(xml: string): ParsedRssItem[] {
  return splitItems(xml)
    .map(parseItem)
    .filter((i): i is ParsedRssItem => i !== null);
}
