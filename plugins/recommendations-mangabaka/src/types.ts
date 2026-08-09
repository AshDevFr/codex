/**
 * MangaBaka API response types, narrowed to what the recommender reads.
 *
 * This is a deliberate subset rather than a shared package. `metadata-mangabaka`
 * models the same upstream service, but it is built on the *stable* endpoints
 * (`/v1/series/{id}`, `/v1/series/search`). Everything here is built on
 * endpoints upstream marks `x-api-stability: beta`, so the two are expected to
 * drift on different schedules. Coupling them would drag a stable plugin's
 * release cadence behind beta churn.
 *
 * API docs: https://mangabaka.org/api
 *
 * Every field except `id` is optional. Beta endpoints may add, drop, or rename
 * fields without notice, so parsing has to survive a partially unrecognised
 * payload rather than throw.
 */

/** Standard API response envelope. */
export interface MbApiResponse<T> {
  status: number;
  data: T;
}

/** Series type. Doubles as the country-of-origin signal (manga=JP, manhwa=KR, manhua=CN). */
export type MbSeriesType = "manga" | "novel" | "manhwa" | "manhua" | "oel" | "other";

/** Publication status. */
export type MbStatus = "cancelled" | "completed" | "hiatus" | "releasing" | "unknown" | "upcoming";

/** Sexual content rating, ordered from least to most explicit. */
export type MbContentRating = "safe" | "suggestive" | "erotica" | "pornographic";

/**
 * Per-series tag relevance, ordered from strongest to weakest.
 *
 * Upstream defaults this to `unweighted` but still returns explicit `null` in
 * some rows, so consumers must handle the absent case separately from
 * `unweighted`.
 */
export type MbTagWeight = "core" | "defining" | "recurrent" | "incidental" | "unweighted";

/** Lifecycle state. Anything other than `active` should be skipped. */
export type MbSeriesState = "active" | "merged" | "deleted";

/** One scaled CDN image variant. */
export interface MbScaledImage {
  x1?: string | null;
  x2?: string | null;
  x3?: string | null;
}

/** Cover images. `raw` is the original; the sized variants are CDN-resized. */
export interface MbCover {
  raw?: { url?: string | null } | null;
  x150?: MbScaledImage | null;
  x250?: MbScaledImage | null;
  x350?: MbScaledImage | null;
}

/** A tag as it appears on a series (`tags_v2`). */
export interface MbTagV2 {
  id: number;
  name: string;
  /**
   * Hierarchy path, e.g. "Audience Demographics > Male Oriented > Shounen".
   * The root segment is used as the Codex tag category.
   */
  name_path?: string | null;
  weight?: MbTagWeight | null;
  is_spoiler?: boolean | null;
}

/** A tag as it appears in the shared-tag annotation on a recommendation. */
export interface MbSharedTag {
  id: number;
  name: string;
  weight?: MbTagWeight | null;
}

/** An alternate title. */
export interface MbSecondaryTitle {
  title: string;
  type?: string | null;
}

/** A typed relationship to another series (`relationships_v2`). */
export interface MbRelationshipV2 {
  to_series_id: number;
  relation_type?: string | null;
}

/** Cross-service identity and rating for one external source. */
export interface MbSourceEntry {
  id?: number | string | null;
  rating?: number | null;
  rating_normalized?: number | null;
}

/**
 * Cross-service IDs. Used to widen in-library detection to series matched by a
 * provider other than `metadata-mangabaka`.
 */
export interface MbSource {
  anilist?: MbSourceEntry | null;
  anime_planet?: MbSourceEntry | null;
  anime_news_network?: MbSourceEntry | null;
  kitsu?: MbSourceEntry | null;
  manga_updates?: MbSourceEntry | null;
  my_anime_list?: MbSourceEntry | null;
  shikimori?: MbSourceEntry | null;
}

/** A series as embedded in a recommendation result. */
export interface MbSeries {
  id: number;
  state?: MbSeriesState | null;
  merged_with?: number | null;

  title?: string | null;
  native_title?: string | null;
  romanized_title?: string | null;
  /** Alternate titles keyed by language code (or "unknown"). */
  secondary_titles?: Record<string, MbSecondaryTitle[]> | null;

  cover?: MbCover | null;
  description?: string | null;
  authors?: string[] | null;
  artists?: string[] | null;

  year?: number | null;
  status?: MbStatus | null;
  type?: MbSeriesType | null;
  content_rating?: MbContentRating | null;

  genres?: string[] | null;
  tags_v2?: MbTagV2[] | null;

  final_volume?: number | string | null;
  /** Upstream sends this as a string (e.g. "50"), not a number. */
  total_chapters?: number | string | null;
  /** 0-100 scale, fractional (e.g. 67.16). */
  rating?: number | null;
  popularity?: number | null;

  /** Map of relation type to related series IDs, e.g. `{ "other": [57367] }`. */
  relationships?: Record<string, number[]> | null;
  relationships_v2?: MbRelationshipV2[] | null;
  source?: MbSource | null;
}

/**
 * One recommendation result.
 *
 * Shared by `/v1/series/mix` and `/v1/series/{id}/readers-also-like`, but the
 * two populate different fields, and the differences matter:
 *
 * - **Mix** returns `cosine`, `shared_tags`, `matched_seed_ids`,
 *   `matched_author`, and `matched_related`. Its `score` is a bounded
 *   similarity, observed in the 0.3-0.8 range.
 * - **Readers-also-like** returns `shared_users` and `rank`, and carries *none*
 *   of the mix annotations. Its `score` is an unbounded co-occurrence weight
 *   whose scale varies enormously between seeds (10, 46, and 306 as the top
 *   score for three different series), so it is only meaningful relative to
 *   other entries in the same response.
 *
 * The embedded `series` object is the full shape from both endpoints.
 */
export interface MbRecommendationEntry {
  /**
   * Ranking score.
   *
   * Not comparable across endpoints, nor across readers-also-like responses for
   * different seeds. Normalise per response before combining.
   */
  score?: number | null;
  /** Raw cosine similarity, before MMR diversification. Mix only. */
  cosine?: number | null;
  /** Mix only. */
  shared_tags?: MbSharedTag[] | null;
  /** Mix only. */
  shared_tags_total?: number | null;
  /** True when the result shares an author or artist with a seed. Mix only. */
  matched_author?: boolean | null;
  /** True when the result is a known related work of a seed. Mix only. */
  matched_related?: boolean | null;
  /** Which seed IDs contributed to this result. Mix only. */
  matched_seed_ids?: number[] | null;
  /** How many readers hold both this series and the source. Collaborative only. */
  shared_users?: number | null;
  /** Position within this response, starting at 1. Collaborative only. */
  rank?: number | null;
  series: MbSeries;
}

/** Response from `GET /v1/series/mix`. */
export interface MbMixResponse extends MbApiResponse<MbRecommendationEntry[]> {
  /** Aggregate tag profile of the combined seed vector. Diagnostic only. */
  dna?: unknown;
  seed_count?: number | null;
}

/** Response from `GET /v1/series/{id}/readers-also-like`. */
export type MbReadersAlsoLikeResponse = MbApiResponse<MbRecommendationEntry[]>;

/** A tag in the global catalogue (`GET /v1/tags`). */
export interface MbCatalogueTag {
  id: number;
  name: string;
  name_path?: string | null;
  parent_id?: number | null;
  merged_with?: number | null;
  content_rating?: MbContentRating | null;
}

/** Response from `GET /v1/tags`. */
export type MbTagsResponse = MbApiResponse<MbCatalogueTag[]>;

// =============================================================================
// Request parameters
// =============================================================================

/** Filters accepted by both `/mix` and `/readers-also-like`. */
export interface MbCommonFilters {
  /** Allowed content ratings. Empty or omitted means no rating filter. */
  contentRating?: MbContentRating[];
  /**
   * Tag IDs to exclude. Upstream requires *numeric IDs*; passing tag names
   * returns HTTP 400. Resolve names via `GET /v1/tags` first.
   */
  tagNot?: number[];
}

/** Parameters for `GET /v1/series/mix`. */
export interface MbMixParams extends MbCommonFilters {
  /** Seed series IDs. At least one seed or one include-tag is required. */
  series: number[];
  /** Max results, 1-50. */
  limit?: number;
  /**
   * When true, include-tags act as a hard SQL filter; when false they only
   * boost the probe vector. Upstream defaults to true.
   */
  strict?: boolean;
  /** Limit results to these series types. */
  type?: MbSeriesType[];
  /** Exclude these series types. */
  typeNot?: MbSeriesType[];
  /** Limit results to these publication statuses. */
  status?: MbStatus[];
  /** Exclude these publication statuses. */
  statusNot?: MbStatus[];
  /** Exclude these genres. */
  genreNot?: string[];
  /** Minimum rating, 0-100. */
  ratingLower?: number;
  /** Maximum rating, 0-100. */
  ratingUpper?: number;
}

/** Parameters for `GET /v1/series/{id}/readers-also-like`. */
export interface MbReadersAlsoLikeParams extends MbCommonFilters {
  /** Max results, 1-24. */
  limit?: number;
}
