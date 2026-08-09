/**
 * Recommendation generation.
 *
 * Kept out of `index.ts` because that module starts a stdio JSON-RPC server as
 * an import side effect, which makes the generation path awkward to exercise
 * directly. Here it is a plain function over an injected client.
 */

import type {
  RecommendationRequest,
  RecommendationResponse,
  UserLibraryEntry,
} from "@ashdev/codex-plugin-sdk";
import type { MangaBakaRecommendationClient } from "./api.js";
import type { DismissalStore } from "./dismissals.js";
import { type Candidate, collapseFranchises, filterCandidates } from "./filters.js";
import { logger } from "./logger.js";
import { mapRecommendation } from "./mappers.js";
import { type AuthorAdjustmentOptions, applyAuthorAdjustment } from "./scoring.js";
import { type ResolvedSeed, resolveSeeds } from "./seeds.js";
import { normalizeTitle } from "./titles.js";

/** Upstream cap on `/v1/series/mix`. */
const MIX_LIMIT_MAX = 50;

/** Fallback when the host does not specify a limit. */
const DEFAULT_LIMIT = 20;

/**
 * How far past the requested limit to fetch.
 *
 * Filtering removes a large and unpredictable share of every response. On the
 * captured Re:Zero probe it removed six of eight results, so requesting exactly
 * `limit` would have returned a quarter of what was asked for.
 */
export const MIX_OVERFETCH_FACTOR = 3;

export type GenerateOptions = AuthorAdjustmentOptions;

/** An empty, well-formed response. */
function emptyResponse(): RecommendationResponse {
  return { recommendations: [], generatedAt: new Date().toISOString(), cached: false };
}

/**
 * Normalised titles of every seed, including the alternates Codex holds.
 *
 * Used to catch a franchise member that upstream's relationship data misses but
 * whose title gives it away.
 */
function seedTitleKeys(entries: UserLibraryEntry[], seeds: ResolvedSeed[]): Set<string> {
  const keys = new Set<string>();

  const add = (title: string | null | undefined) => {
    const key = normalizeTitle(title);
    if (key.length > 0) keys.add(key);
  };

  for (const seed of seeds) add(seed.title);
  for (const entry of entries) {
    add(entry?.title);
    for (const alternate of entry?.alternateTitles ?? []) add(alternate);
  }

  return keys;
}

/**
 * Generate recommendations for one user.
 *
 * Upstream failures resolve to an empty set rather than propagating. The host
 * runs this inside a task; a transient 503 from a beta endpoint should show up
 * as "no recommendations this time", not a failed task the user has to
 * investigate.
 */
export async function generateRecommendations(
  client: MangaBakaRecommendationClient,
  params: RecommendationRequest,
  dismissed: DismissalStore,
  options: GenerateOptions = {},
): Promise<RecommendationResponse> {
  const { library: entries = [], excludeIds = [] } = params;
  const limit = Math.max(1, params.limit ?? DEFAULT_LIMIT);

  if (entries.length === 0) {
    logger.info("Empty library - returning no recommendations");
    return emptyResponse();
  }

  const { seeds, library } = resolveSeeds(entries);
  if (seeds.length === 0) {
    logger.warn(
      "No library entry could be resolved to a MangaBaka series, so there is nothing to seed " +
        "recommendations from. Run a metadata match with the MangaBaka Metadata plugin first.",
    );
    return emptyResponse();
  }

  const seedTitles = new Map(seeds.map((seed) => [seed.id, seed.title]));
  const seedIds = new Set(seeds.map((seed) => seed.id));

  let entriesFromUpstream: Awaited<ReturnType<MangaBakaRecommendationClient["mix"]>>;
  try {
    entriesFromUpstream = await client.mix({
      series: seeds.map((seed) => seed.id),
      limit: Math.min(limit * MIX_OVERFETCH_FACTOR, MIX_LIMIT_MAX),
      // Tag rules should steer the probe vector, not hard-filter candidates out
      // of it. Upstream defaults this to true.
      strict: false,
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : "Unknown error";
    logger.warn(`Mix request failed, returning no recommendations: ${message}`);
    return emptyResponse();
  }

  logger.debug(`Mix returned ${entriesFromUpstream.length} candidates for ${seeds.length} seeds`);

  // Map first so filters can read both the upstream annotations and the mapped
  // form. Entries the mapper rejects (merged and deleted tombstones) drop here.
  const candidates: Candidate[] = [];
  for (const entry of entriesFromUpstream) {
    const recommendation = mapRecommendation(entry, { seedTitles, library });
    if (recommendation) candidates.push({ entry, recommendation });
  }

  const filtered = filterCandidates(candidates, {
    seedIds,
    seedTitleKeys: seedTitleKeys(entries, seeds),
    excludeIds: new Set(excludeIds),
    isDismissed: (externalId) => dismissed.has(externalId),
  });

  // Author adjustment runs before the franchise collapse so that a de-ranked
  // same-author entry cannot win its group over a stronger unrelated sibling.
  const scored = applyAuthorAdjustment(filtered, options);

  // De-duplicate by external ID before collapsing: a repeat within one response
  // is not a franchise, just a duplicate, and should not consume a group.
  const byExternalId = new Map<string, Candidate>();
  for (const candidate of scored) {
    const existing = byExternalId.get(candidate.recommendation.externalId);
    if (!existing || candidate.recommendation.score > existing.recommendation.score) {
      byExternalId.set(candidate.recommendation.externalId, candidate);
    }
  }

  const collapsed = collapseFranchises([...byExternalId.values()]);

  const recommendations = collapsed
    .map((candidate) => candidate.recommendation)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit);

  logger.info(
    `Generated ${recommendations.length} recommendations from ${seeds.length} seeds ` +
      `(${entriesFromUpstream.length} candidates, ${candidates.length - filtered.length} filtered out)`,
  );

  if (recommendations.length < limit) {
    // Worth surfacing: it usually means the seeds were dominated by one
    // franchise, so almost everything similar to them was a related work.
    logger.debug(
      `Returned ${recommendations.length} of the ${limit} requested; filtering removed the rest`,
    );
  }

  return { recommendations, generatedAt: new Date().toISOString(), cached: false };
}
