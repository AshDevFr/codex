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
import type { MbRecommendationEntry, MbSeries } from "./types.js";

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
 * Drop candidates that should never reach the user.
 *
 * Four independent reasons, checked in cost order: the candidate is a seed, the
 * host excluded it, the user dismissed it, or it belongs to a seed's franchise.
 */
export function filterCandidates(candidates: Candidate[], context: FilterContext): Candidate[] {
  const kept: Candidate[] = [];
  let relatedDropped = 0;

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

    kept.push(candidate);
  }

  if (relatedDropped > 0) {
    logger.debug(`Dropped ${relatedDropped} candidates related to a seed`);
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
export function collapseFranchises(candidates: Candidate[]): Candidate[] {
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
