/**
 * Translation from MangaBaka's series shape into Codex `Recommendation` values.
 *
 * Several upstream fields need real conversion rather than a rename:
 * `total_chapters` arrives as a string, `rating` as a 0-100 float, tag relevance
 * as an ordinal word, and publication status under different names. Each is
 * handled here so the rest of the plugin can work in Codex's vocabulary.
 */

import type { Recommendation, SeriesStatus } from "@ashdev/codex-plugin-sdk";
import { seriesUrl } from "./manifest.js";
import type { LibraryIndex } from "./seeds.js";
import type {
  MbRecommendationEntry,
  MbSeries,
  MbSeriesType,
  MbSharedTag,
  MbStatus,
  MbTagV2,
  MbTagWeight,
} from "./types.js";

/** How many shared tags to name in the reason text before it stops being readable. */
const MAX_REASON_TAGS = 3;
/** How many seed titles to name in the reason text before summarising. */
const MAX_REASON_TITLES = 2;

export interface MappingContext {
  /** MangaBaka seed ID to the Codex title it was resolved from. */
  seedTitles: Map<number, string>;
  /** What the user already has, for in-library flagging. */
  library: LibraryIndex;
}

/**
 * Map MangaBaka's publication status onto Codex's canonical enum.
 *
 * The two vocabularies overlap but do not match: upstream says `releasing`
 * where Codex says `ongoing`, and `completed` where Codex says `ended`.
 * `upcoming` has no Codex equivalent and collapses to `unknown`.
 */
export function mapStatus(status: MbStatus | null | undefined): SeriesStatus | undefined {
  switch (status) {
    case "releasing":
      return "ongoing";
    case "completed":
      return "ended";
    case "hiatus":
      return "hiatus";
    case "cancelled":
      return "abandoned";
    case "upcoming":
    case "unknown":
      return "unknown";
    default:
      return undefined;
  }
}

/**
 * Map the series type onto the `format` field.
 *
 * Uppercased because the recommendation card compares against "MANGA",
 * "NOVEL", and "ONE_SHOT" to decide whether to show a badge and how to label it.
 */
export function mapFormat(type: MbSeriesType | null | undefined): string | undefined {
  return type ? type.toUpperCase() : undefined;
}

/**
 * Infer country of origin from the series type.
 *
 * MangaBaka has no country field, but its type vocabulary encodes the same
 * information for the three regional formats. `novel`, `oel`, and `other` carry
 * no reliable signal and are left unset rather than guessed.
 */
function mapCountry(type: MbSeriesType | null | undefined): string | undefined {
  switch (type) {
    case "manga":
      return "JP";
    case "manhwa":
      return "KR";
    case "manhua":
      return "CN";
    default:
      return undefined;
  }
}

/**
 * Convert MangaBaka's ordinal tag weight into the 0-100 numeric rank Codex
 * expects. An absent weight ranks below `unweighted`: upstream defaults the
 * field to `unweighted` when it has an opinion, so an explicit null means it
 * has none.
 */
function weightToRank(weight: MbTagWeight | null | undefined): number {
  switch (weight) {
    case "core":
      return 100;
    case "defining":
      return 85;
    case "recurrent":
      return 60;
    case "incidental":
      return 35;
    case "unweighted":
      return 10;
    default:
      return 0;
  }
}

/** Take the root segment of a `name_path` as the tag's category. */
function categoryOf(namePath: string | null | undefined): string {
  const root = namePath?.split(">")[0]?.trim();
  return root && root.length > 0 ? root : "Tag";
}

/** Map series tags, dropping spoilers. */
export function mapTags(tags: MbTagV2[] | null | undefined): Recommendation["tags"] {
  if (!Array.isArray(tags) || tags.length === 0) return undefined;

  const mapped = tags
    // A spoiler tag on a recommendation card spoils a series the user has not
    // read yet, which is the whole point of recommending it.
    .filter((tag) => tag?.is_spoiler !== true && typeof tag?.name === "string")
    .map((tag) => ({
      name: tag.name,
      rank: weightToRank(tag.weight),
      category: categoryOf(tag.name_path),
    }));

  return mapped.length > 0 ? mapped : undefined;
}

/** Detect scripts that a Latin-reading audience cannot parse. */
function isLatinScript(value: string): boolean {
  // Covers CJK, Hangul, Cyrillic, Arabic, Hebrew, Thai, and kana. If any such
  // character is present the string is treated as non-Latin.
  return !/[Ѐ-ӿ֐-ࣿ฀-๿　-ヿ㐀-鿿가-힯豈-﫿]/.test(value);
}

/**
 * Choose the most readable title.
 *
 * MangaBaka's primary `title` is sometimes the native-script one. Where a
 * romanization exists it is the better choice for a Codex UI.
 */
export function pickTitle(series: MbSeries): string {
  const { title, romanized_title: romanized, native_title: native } = series;

  if (title && title.trim().length > 0) {
    if (!isLatinScript(title) && romanized && romanized.trim().length > 0) {
      return romanized;
    }
    return title;
  }

  if (romanized && romanized.trim().length > 0) return romanized;
  if (native && native.trim().length > 0) return native;

  return `Series ${series.id}`;
}

/** Title-case a lowercase upstream genre or tag slug ("slice_of_life" -> "Slice of Life"). */
function titleCase(value: string): string {
  const minorWords = new Set(["of", "the", "and", "a", "an", "in", "on"]);

  return value
    .replace(/_/g, " ")
    .split(" ")
    .map((word, index) => {
      if (index > 0 && minorWords.has(word.toLowerCase())) return word.toLowerCase();
      // Hyphenated compounds capitalise on both sides ("sci-fi" -> "Sci-Fi").
      return word
        .split("-")
        .map((part) => (part.length > 0 ? part[0].toUpperCase() + part.slice(1) : part))
        .join("-");
    })
    .join(" ");
}

/** Join a list into readable prose ("A, B and C"). */
function joinWords(values: string[]): string {
  if (values.length <= 1) return values[0] ?? "";
  return `${values.slice(0, -1).join(", ")} and ${values[values.length - 1]}`;
}

/**
 * Render a list of seed titles, summarising once it grows past what a reader
 * will actually take in.
 */
export function summariseTitles(titles: string[]): string {
  if (titles.length === 0) return "";
  if (titles.length <= MAX_REASON_TITLES) return joinWords(titles);

  // Comma-join rather than joinWords here: the trailing "and N more" is already
  // the final conjunction, so "A and B and 1 more" would double it.
  const shown = titles.slice(0, MAX_REASON_TITLES);
  return `${shown.join(", ")} and ${titles.length - MAX_REASON_TITLES} more`;
}

/**
 * Compose the human-readable justification.
 *
 * The protocol offers only a free-text `reason` string, so MangaBaka's
 * structured shared-tag data has to be flattened into prose here.
 */
export function buildReason(
  sharedTags: MbSharedTag[] | null | undefined,
  basedOn: string[],
): string {
  const tagNames = (sharedTags ?? [])
    .filter((tag) => typeof tag?.name === "string" && tag.name.length > 0)
    // Strongest tags first, so truncation drops the least meaningful ones.
    .sort((a, b) => weightToRank(b.weight) - weightToRank(a.weight))
    .slice(0, MAX_REASON_TAGS)
    .map((tag) => tag.name);

  const titles = summariseTitles(basedOn);

  if (tagNames.length > 0 && titles) {
    return `Shares ${joinWords(tagNames)} with ${titles}`;
  }
  if (tagNames.length > 0) {
    return `Matches your taste for ${joinWords(tagNames)}`;
  }
  if (titles) {
    return `Similar to ${titles}`;
  }
  return "Matches your library's taste profile";
}

/** Parse a numeric field that upstream may send as a string. */
function parseNumeric(value: number | string | null | undefined): number | undefined {
  if (typeof value === "number") return Number.isFinite(value) ? value : undefined;
  if (typeof value !== "string") return undefined;

  const parsed = Number.parseFloat(value.trim());
  return Number.isFinite(parsed) ? parsed : undefined;
}

/**
 * Clamp into the 0.0-1.0 range the protocol requires for `score`, and round to
 * two decimals. Upstream returns full float precision (0.69555523762905), which
 * is noise: nothing downstream can act on differences that small, and it makes
 * stored recommendations harder to read.
 */
export function clampScore(score: number | null | undefined): number {
  if (typeof score !== "number" || !Number.isFinite(score)) return 0;
  const bounded = Math.max(0, Math.min(score, 1));
  return Math.round(bounded * 100) / 100;
}

/**
 * Map one upstream recommendation entry into a Codex `Recommendation`.
 *
 * Returns null for entries that should never reach the user: merged and
 * deleted tombstones remain addressable upstream, but recommending one sends
 * the user to a dead page.
 */
export function mapRecommendation(
  entry: MbRecommendationEntry,
  context: MappingContext,
): Recommendation | null {
  const series = entry.series;

  if (series.state === "merged" || series.state === "deleted") {
    return null;
  }

  const basedOn = (entry.matched_seed_ids ?? [])
    // A seed ID with no known title cannot be rendered, and showing a bare
    // number in "Similar to 3397" is worse than omitting it.
    .map((id) => context.seedTitles.get(id))
    .filter((title): title is string => typeof title === "string");

  const volumes = parseNumeric(series.final_volume);
  const chapters = parseNumeric(series.total_chapters);
  const rating = parseNumeric(series.rating);

  return {
    externalId: String(series.id),
    externalUrl: seriesUrl(series.id),
    title: pickTitle(series),
    coverUrl: series.cover?.x350?.x1 ?? series.cover?.raw?.url ?? undefined,
    summary: series.description ?? undefined,
    genres: (series.genres ?? []).filter((g) => typeof g === "string").map(titleCase),
    tags: mapTags(series.tags_v2),

    score: clampScore(entry.score),
    reason: buildReason(entry.shared_tags, basedOn),
    basedOn,

    inLibrary: context.library.has(series),

    status: mapStatus(series.status),
    format: mapFormat(series.type),
    countryOfOrigin: mapCountry(series.type),
    startYear: series.year ?? undefined,
    // Upstream uses 0 to mean "unknown" rather than "zero volumes".
    totalVolumeCount: volumes && volumes > 0 ? Math.round(volumes) : undefined,
    totalChapterCount: chapters && chapters > 0 ? chapters : undefined,
    rating: rating !== undefined ? Math.round(rating) : undefined,
    // `popularity` is deliberately not mapped. MangaBaka reports a rank, where
    // lower is more popular, while this field is rendered as a count where
    // higher is. Passing the rank through would invert the meaning, and it also
    // arrives as a nested object rather than a number, which the host rejects
    // outright when deserialising.
  };
}
