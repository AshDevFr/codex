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
  /**
   * All alias candidates extracted from the series-name region. When the title
   * uses `Title A / Title B` (a common 1r0n / LuCaZ convention for "JP name /
   * EN name"), both halves are surfaced here so the matcher can score against
   * either. For titles without a slash separator this is a single-element
   * array equal to `[seriesGuess]`.
   */
  seriesGuessAliases: string[];
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
  /** RSS `<link>` value. On Nyaa this is the `.torrent` download URL. */
  link: string;
  /**
   * Permalink to the release post page (e.g. `https://nyaa.si/view/12345`),
   * derived from the `<guid isPermaLink="true">` tag. Null when the guid is
   * missing or doesn't look like a post URL.
   */
  pageUrl: string | null;
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
 * Strip every `(...)` group from a string. Used to keep year ranges, uploader
 * credits, and format-hint tags out of the chapter/volume tokenizer — those
 * always live inside parentheses, so anything inside them must not be
 * interpreted as release-info.
 */
function stripParens(s: string): string {
  return s.replace(/\([^)]*\)/g, " ");
}

/**
 * Locate the start of the "release-info span" — the offset in `s` (which has
 * already had `(...)` groups blanked) where chapter/volume tokens begin.
 *
 * Anchors, in priority order:
 *   1. A `v##`, `vol.##`, `volume ##` token (with or without a range).
 *   2. A bare numeric range with both sides at 3+ digits (`031-037`,
 *      `001-069`). Two-digit forms are rejected to avoid false positives
 *      inside series names (`30s`, `My 100`, etc.).
 *   3. A `c##` / `ch.##` / `Chapter ##` token.
 *
 * Returns the index of the anchor, or -1 if no release-info is present (the
 * whole string is then treated as a series name).
 */
function findReleaseInfoStart(s: string): number {
  const anchors: RegExp[] = [
    /\b(?:v|vol|volume)\.?\s*[0-9]+/i,
    /\b[0-9]{3,4}\s*[-–]\s*[0-9]{3,4}\b/,
    /\b(?:c|ch|chapter)\.?\s*[0-9]+/i,
  ];
  let best = -1;
  for (const re of anchors) {
    const m = s.match(re);
    if (m && m.index !== undefined && (best === -1 || m.index < best)) {
      best = m.index;
    }
  }
  return best;
}

/**
 * Spread tokens are the comma- / `+`- / whitespace- / `as`-separated atoms
 * that make up the release-info span:
 *
 *   - `volume`     : single volume number      (`v01`, `Vol. 13`)
 *   - `volRange`   : volume range              (`v01-14`)
 *   - `chapter`    : single chapter number     (`c143`, bare `70`)
 *   - `chapRange`  : chapter range             (`c126-142`, bare `031-037`)
 *
 * The tokenizer scans left-to-right and consumes one token per match. Bare
 * numeric tokens are only accepted *after* the release-info anchor — see
 * `findReleaseInfoStart` — so series-name digits don't leak in.
 */
type SpreadToken =
  | { kind: "volume"; value: number }
  | { kind: "volRange"; start: number; end: number }
  | { kind: "chapter"; value: number }
  | { kind: "chapRange"; start: number; end: number };

/**
 * Tokenize the release-info span into volume/chapter atoms.
 *
 * `s` should be the parens-stripped substring starting at the release-info
 * anchor. The tokenizer is intentionally permissive about separators (commas,
 * `+`, whitespace, `as`) — we just consume tokens greedily and aggregate
 * downstream.
 */
function tokenizeReleaseInfo(s: string): SpreadToken[] {
  const tokens: SpreadToken[] = [];

  // Match either a prefixed volume/chapter token, or a bare numeric range /
  // single. The order in the alternation matters: ranges must be tried before
  // single tokens, and prefixed forms must be tried before bare numerics so
  // we don't mis-classify `v05` as bare-chapter `5`.
  //
  //   1. `v##-##` / `vol.##-##` / `volume ##-##`              → volRange
  //   2. `v##` / `vol.##` / `volume ##`                       → volume
  //   3. `c##.##-##.##` / `ch.##-##` / `Chapter ##-##`        → chapRange
  //   4. `c##.##` / `ch.##` / `Chapter ##`                    → chapter
  //   5. bare `###-###` (3+ digits each side)                 → chapRange
  //   6. bare `##` (1+ digits) — only matches *after* the first anchor token
  //      has been emitted, see `acceptShortBare` below. Lets us pick up
  //      "extra" chapters expressed as short numerics (`+ 70`) without
  //      promoting incidental name-region digits.
  const tokenRe = new RegExp(
    [
      "\\b(?<vrs>v|vol|volume)\\.?\\s*([0-9]+)\\s*[-–]\\s*([0-9]+)\\b",
      "\\b(?<vss>v|vol|volume)\\.?\\s*([0-9]+)\\b",
      "\\b(?<crs>c|ch|chapter)\\.?\\s*([0-9]+(?:\\.[0-9]+)?)\\s*[-–]\\s*([0-9]+(?:\\.[0-9]+)?)\\b",
      "\\b(?<css>c|ch|chapter)\\.?\\s*([0-9]+(?:\\.[0-9]+)?)\\b",
      "\\b(?<brs>)([0-9]{3,4})\\s*[-–]\\s*([0-9]{3,4})\\b",
      "\\b(?<bss>)([0-9]{1,4})\\b",
    ].join("|"),
    "gi",
  );

  for (;;) {
    const m = tokenRe.exec(s);
    if (m === null) break;
    const groups = m.groups ?? {};
    if (groups.vrs !== undefined) {
      const start = Number.parseInt(m[2] ?? "", 10);
      const end = Number.parseInt(m[3] ?? "", 10);
      if (Number.isFinite(start) && Number.isFinite(end)) {
        tokens.push({ kind: "volRange", start, end });
      }
      continue;
    }
    if (groups.vss !== undefined) {
      const value = Number.parseInt(m[5] ?? "", 10);
      if (Number.isFinite(value)) tokens.push({ kind: "volume", value });
      continue;
    }
    if (groups.crs !== undefined) {
      const start = Number.parseFloat(m[7] ?? "");
      const end = Number.parseFloat(m[8] ?? "");
      if (Number.isFinite(start) && Number.isFinite(end)) {
        tokens.push({ kind: "chapRange", start, end });
      }
      continue;
    }
    if (groups.css !== undefined) {
      const value = Number.parseFloat(m[10] ?? "");
      if (Number.isFinite(value)) tokens.push({ kind: "chapter", value });
      continue;
    }
    if (groups.brs !== undefined) {
      const start = Number.parseInt(m[12] ?? "", 10);
      const end = Number.parseInt(m[13] ?? "", 10);
      if (Number.isFinite(start) && Number.isFinite(end)) {
        tokens.push({ kind: "chapRange", start, end });
      }
      continue;
    }
    if (groups.bss !== undefined) {
      const raw = m[15] ?? "";
      const value = Number.parseInt(raw, 10);
      if (!Number.isFinite(value)) continue;
      // Only accept short (≤2 digit) bare numerics once we've already
      // committed to a richer token; on its own a `42` is more likely a
      // year fragment or noise than a chapter. 3+ digits is unambiguous in
      // this corpus so we always accept it.
      if (raw.length < 3 && tokens.length === 0) continue;
      tokens.push({ kind: "chapter", value });
    }
  }

  return tokens;
}

/**
 * Aggregate spread tokens into volume + chapter axes by taking min/max across
 * each kind. Downstream matching just needs to know the span a release covers
 * ("does this release include chapter X?") — a min..max window answers that
 * question conservatively without picking a single canonical token.
 */
function aggregateTokens(tokens: SpreadToken[]): {
  volume: number | null;
  volumeRangeEnd: number | null;
  chapter: number | null;
  chapterRangeEnd: number | null;
} {
  let vMin: number | null = null;
  let vMax: number | null = null;
  let cMin: number | null = null;
  let cMax: number | null = null;
  for (const t of tokens) {
    if (t.kind === "volume") {
      vMin = vMin === null || t.value < vMin ? t.value : vMin;
      vMax = vMax === null || t.value > vMax ? t.value : vMax;
    } else if (t.kind === "volRange") {
      vMin = vMin === null || t.start < vMin ? t.start : vMin;
      vMax = vMax === null || t.end > vMax ? t.end : vMax;
    } else if (t.kind === "chapter") {
      cMin = cMin === null || t.value < cMin ? t.value : cMin;
      cMax = cMax === null || t.value > cMax ? t.value : cMax;
    } else {
      cMin = cMin === null || t.start < cMin ? t.start : cMin;
      cMax = cMax === null || t.end > cMax ? t.end : cMax;
    }
  }
  return {
    volume: vMin,
    // Only emit a range-end when it actually differs from the start: a single
    // volume is `volume=N, volumeRangeEnd=null`, matching the prior contract.
    volumeRangeEnd: vMin !== null && vMax !== null && vMax !== vMin ? vMax : null,
    chapter: cMin,
    chapterRangeEnd: cMin !== null && cMax !== null && cMax !== cMin ? cMax : null,
  };
}

/**
 * Walk the parenthesized tags in the title and extract format hints.
 *
 * Common Nyaa hints we care about:
 *   - `(Digital)` → `digital`
 *   - `(JXL)` → `jxl`
 *   - `(Mag-Z)` / `(Magazine)` → `magazine`
 *   - `(Omnibus Edition)` / `(Omnibus)` → `omnibus`
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
    else if (tag === "omnibus" || tag === "omnibus edition") hints.omnibus = true;
  }
  return hints;
}

/**
 * Strip a trailing `[...]` token (e.g. `[Oak]` at the end of some
 * danke-Empire releases). Mirrors `extractLeadingGroup` but at the tail and
 * without surfacing the value — trailing brackets are credit, not a parsing
 * signal we currently use.
 */
function stripTrailingBracket(s: string): string {
  return s.replace(/\s*\[[^\]]+\]\s*$/g, "").trim();
}

/**
 * Take the "name region" of a release title (everything before the first
 * release-info anchor, with parens already stripped) and reduce it to a clean
 * primary guess plus alias candidates.
 *
 * The name region may still contain:
 *   - subtitle dashes: `Boruto - Two Blue Vortex` → joined with spaces
 *   - alias separator: `Ao no Hako / Blue Box` → both halves returned
 *
 * Apostrophes and hyphenated words (`Amagami-san`, `Chillin'`) are preserved
 * — the host's `normalize_alias` strips them at match time, but we want to
 * keep them readable in logs and admin surfaces.
 */
function extractSeriesAliases(nameRegion: string): {
  primary: string;
  aliases: string[];
} {
  // Subtitle dashes: ` - `, ` – `, ` — ` are titling glue, not separators.
  // Joining the halves with a single space mirrors the prior behavior the
  // existing tests assert (`Boruto Two Blue Vortex`).
  const dashJoined = nameRegion.replace(/\s+[-–—]\s+/g, " ");

  // Alias separator. Only ` / ` (with whitespace on both sides) splits — bare
  // `/` survives so e.g. `AC/DC Tales` stays one alias.
  const parts = dashJoined
    .split(/\s+\/\s+/)
    .map((p) => p.replace(/\s+/g, " ").trim())
    .filter((p) => p.length > 0);

  if (parts.length === 0) return { primary: "", aliases: [] };
  return { primary: parts[0] ?? "", aliases: parts };
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
  seriesGuessAliases: string[];
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
  const formatHints = extractFormatHints(rest);

  // Blank out `(...)` groups so years and uploader credits can't be picked up
  // by the release-info tokenizer, then split into name region / release-info
  // region at the first chapter/volume anchor.
  const flattened = stripTrailingBracket(stripParens(rest));
  const anchor = findReleaseInfoStart(flattened);
  const nameRegion = anchor === -1 ? flattened : flattened.slice(0, anchor);
  const infoRegion = anchor === -1 ? "" : flattened.slice(anchor);

  const tokens = tokenizeReleaseInfo(infoRegion);
  const { volume, volumeRangeEnd, chapter, chapterRangeEnd } = aggregateTokens(tokens);
  const { primary, aliases } = extractSeriesAliases(nameRegion);

  return {
    seriesGuess: primary,
    seriesGuessAliases: aliases.length > 0 ? aliases : [primary],
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

/**
 * Pull the post-page URL out of the guid when it looks like a Nyaa
 * `/view/<id>` permalink. The `<link>` tag in Nyaa feeds is the `.torrent`
 * download URL, which is not what we want to surface to users.
 */
function derivePageUrl(guid: string | null): string | null {
  if (!guid) return null;
  const trimmed = guid.trim();
  if (trimmed.length === 0) return null;
  // Match http(s)://<host>/view/<id> with optional trailing slash / query.
  if (/^https?:\/\/[^/]+\/view\/[^/?#]+/i.test(trimmed)) return trimmed;
  return null;
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
    seriesGuessAliases: parsedTitle.seriesGuessAliases,
    chapter: parsedTitle.chapter,
    chapterRangeEnd: parsedTitle.chapterRangeEnd,
    volume: parsedTitle.volume,
    volumeRangeEnd: parsedTitle.volumeRangeEnd,
    group: parsedTitle.group,
    formatHints: parsedTitle.formatHints,
    link: link ?? "",
    pageUrl: derivePageUrl(guid),
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
