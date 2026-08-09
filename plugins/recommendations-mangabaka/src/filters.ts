/**
 * Result quality: keeping franchise spam out of the recommendation set.
 *
 * Tag-vector similarity ranks a series' own spin-offs, side stories, and
 * per-arc volumes as its nearest neighbours, because they genuinely are. A live
 * three-seed probe returned five Re:Zero chapter volumes and the Solo Leveling
 * novel: every result was a franchise member of a seed, and none of them told
 * the user anything they did not already know. Filtering that out is what makes
 * the difference between a usable list and a useless one.
 */

import type { Recommendation } from "@ashdev/codex-plugin-sdk";
import { logger } from "./logger.js";
import { normalizeTitle } from "./titles.js";
import type { MbContentRating, MbRecommendationEntry, MbSeries, MbSeriesType } from "./types.js";

/** An upstream entry paired with its mapped form, so filters can read both. */
export interface Candidate {
  entry: MbRecommendationEntry;
  recommendation: Recommendation;
}

export interface FilterContext {
  /** MangaBaka IDs used to seed the probe. */
  seedIds: Set<number>;
  /** Normalised titles (and alternates) of the seeds. */
  seedTitleKeys: Set<string>;
  /** External IDs the host says to exclude, i.e. series already read. */
  excludeIds: Set<string>;
  /** Whether the user has dismissed this external ID before. */
  isDismissed: (externalId: string) => boolean;
  /**
   * User filters to enforce locally.
   *
   * Redundant for candidates from the content probe, which already applied them
   * upstream, but necessary for collaborative-only ones.
   */
  userFilters?: UserFilterRules;
}

/**
 * Every series ID this one is linked to.
 *
 * Both relationship shapes are read and unioned: they are supposed to carry the
 * same data, but a live response was observed with an ID present in
 * `relationships_v2` and absent from `relationships`.
 */
export function relatedIdsOf(series: MbSeries): Set<number> {
  const ids = new Set<number>();

  for (const group of Object.values(series.relationships ?? {})) {
    if (!Array.isArray(group)) continue;
    for (const id of group) {
      if (typeof id === "number") ids.add(id);
    }
  }

  for (const relation of series.relationships_v2 ?? []) {
    if (typeof relation?.to_series_id === "number") ids.add(relation.to_series_id);
  }

  return ids;
}

/** Whether a candidate is a related work of any seed. */
function isRelatedToSeed(candidate: Candidate, seedIds: Set<number>): boolean {
  // Upstream's own annotation. It is a beta field, so it is trusted but not
  // relied on alone.
  if (candidate.entry.matched_related === true) return true;

  // The independent check: does the candidate itself link back to a seed?
  for (const relatedId of relatedIdsOf(candidate.entry.series)) {
    if (seedIds.has(relatedId)) return true;
  }

  return false;
}

/** Whether a candidate's title matches a seed's under normalisation. */
function collidesWithSeedTitle(candidate: Candidate, seedTitleKeys: Set<string>): boolean {
  if (seedTitleKeys.size === 0) return false;

  const series = candidate.entry.series;
  const keys = [series.title, series.romanized_title, series.native_title];

  for (const secondaries of Object.values(series.secondary_titles ?? {})) {
    for (const secondary of secondaries ?? []) {
      keys.push(secondary?.title);
    }
  }

  return keys.some((key) => {
    const normalized = normalizeTitle(key);
    return normalized.length > 0 && seedTitleKeys.has(normalized);
  });
}

/**
 * Filters a user configured that the plugin may have to enforce itself.
 *
 * `/mix` applies all of these upstream, but `/readers-also-like` accepts only
 * `content_rating` and `tag_not`. Without a local check, a collaborative-only
 * result would sail past a type, genre, or rating filter the user explicitly
 * set, which reads as the setting being broken.
 */
export interface UserFilterRules {
  contentRating?: MbContentRating[];
  includedTypes?: MbSeriesType[];
  excludedTypes?: MbSeriesType[];
  excludedGenres?: string[];
  excludedTagIds?: number[];
  minimumRating?: number;
}

/**
 * Whether a series satisfies the user's filters.
 *
 * Only ever rejects on positive evidence. A series with no type, rating, or
 * genre data recorded is kept rather than assumed to violate the filter:
 * missing data is unknown, not disqualifying, and dropping it would quietly
 * remove newer titles nobody has catalogued yet.
 */
export function matchesUserFilters(series: MbSeries, rules: UserFilterRules): boolean {
  if (rules.includedTypes?.length && series.type && !rules.includedTypes.includes(series.type)) {
    return false;
  }
  if (rules.excludedTypes?.length && series.type && rules.excludedTypes.includes(series.type)) {
    return false;
  }

  if (rules.contentRating?.length && series.content_rating) {
    if (!rules.contentRating.includes(series.content_rating)) return false;
  }

  if (rules.excludedGenres?.length && Array.isArray(series.genres)) {
    const excluded = new Set(rules.excludedGenres.map((genre) => genre.toLowerCase()));
    if (series.genres.some((genre) => excluded.has(String(genre).toLowerCase()))) return false;
  }

  if (rules.excludedTagIds?.length && Array.isArray(series.tags_v2)) {
    const excluded = new Set(rules.excludedTagIds);
    if (series.tags_v2.some((tag) => excluded.has(tag?.id))) return false;
  }

  if (rules.minimumRating !== undefined && typeof series.rating === "number") {
    if (series.rating < rules.minimumRating) return false;
  }

  return true;
}

/**
 * Drop candidates that should never reach the user.
 *
 * Four independent reasons, checked in cost order: the candidate is a seed, the
 * host excluded it, the user dismissed it, or it belongs to a seed's franchise.
 */
export function filterCandidates<T extends Candidate>(
  candidates: T[],
  context: FilterContext,
): T[] {
  const kept: T[] = [];
  let relatedDropped = 0;
  let filteredOut = 0;

  for (const candidate of candidates) {
    const { series } = candidate.entry;
    const externalId = candidate.recommendation.externalId;

    // A seed is trivially similar to itself.
    if (context.seedIds.has(series.id)) continue;
    if (context.excludeIds.has(externalId)) continue;
    if (context.isDismissed(externalId)) continue;

    if (
      isRelatedToSeed(candidate, context.seedIds) ||
      collidesWithSeedTitle(candidate, context.seedTitleKeys)
    ) {
      relatedDropped++;
      continue;
    }

    if (context.userFilters && !matchesUserFilters(series, context.userFilters)) {
      filteredOut++;
      continue;
    }

    kept.push(candidate);
  }

  if (relatedDropped > 0) {
    logger.debug(`Dropped ${relatedDropped} candidates related to a seed`);
  }
  if (filteredOut > 0) {
    logger.debug(`Dropped ${filteredOut} candidates that did not match the configured filters`);
  }

  return kept;
}

/**
 * Reduce each franchise among the survivors to its single strongest entry.
 *
 * Distinct from seed-relatedness: this catches a franchise the user has no seed
 * for, where three volumes of the same unfamiliar series would otherwise fill
 * the list. Candidates are grouped by mutual relationship links and by
 * normalised title, then the highest scoring member of each group is kept.
 *
 * Grouping deliberately only links two candidates when one names the *other*
 * directly. Two series that both reference some absent third work are not
 * thereby the same franchise.
 */
export function collapseFranchises<T extends Candidate>(candidates: T[]): T[] {
  if (candidates.length === 0) return [];

  // Union-find over candidate positions, so a chain (1-2, 2-3) collapses whole.
  const parent = candidates.map((_, index) => index);

  const find = (index: number): number => {
    let root = index;
    while (parent[root] !== root) root = parent[root];
    // Path compression keeps repeated lookups cheap on long chains.
    let cursor = index;
    while (parent[cursor] !== root) {
      const next = parent[cursor];
      parent[cursor] = root;
      cursor = next;
    }
    return root;
  };

  const union = (a: number, b: number): void => {
    const rootA = find(a);
    const rootB = find(b);
    if (rootA !== rootB) parent[rootB] = rootA;
  };

  const indexById = new Map<number, number>();
  const indexByTitle = new Map<string, number>();

  candidates.forEach((candidate, index) => {
    indexById.set(candidate.entry.series.id, index);
  });

  candidates.forEach((candidate, index) => {
    // Link to any other candidate this one explicitly names as related.
    for (const relatedId of relatedIdsOf(candidate.entry.series)) {
      const relatedIndex = indexById.get(relatedId);
      if (relatedIndex !== undefined) union(index, relatedIndex);
    }

    // Link volumes of one work that relationship data does not connect.
    const titleKey = normalizeTitle(candidate.entry.series.title);
    if (titleKey.length === 0) return;

    const seen = indexByTitle.get(titleKey);
    if (seen === undefined) {
      indexByTitle.set(titleKey, index);
    } else {
      union(index, seen);
    }
  });

  // Keep the strongest member of each group, preserving input order.
  const bestByGroup = new Map<number, number>();
  candidates.forEach((candidate, index) => {
    const group = find(index);
    const incumbent = bestByGroup.get(group);
    if (
      incumbent === undefined ||
      candidate.recommendation.score > candidates[incumbent].recommendation.score
    ) {
      bestByGroup.set(group, index);
    }
  });

  const winners = new Set(bestByGroup.values());
  const kept = candidates.filter((_, index) => winners.has(index));

  const collapsed = candidates.length - kept.length;
  if (collapsed > 0) {
    logger.debug(`Collapsed ${collapsed} candidates into their franchise's best entry`);
  }

  return kept;
}
