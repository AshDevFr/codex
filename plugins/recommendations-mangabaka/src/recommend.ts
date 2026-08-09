/**
 * Recommendation generation.
 *
 * Kept out of `index.ts` because that module starts a stdio JSON-RPC server as
 * an import side effect, which makes the generation path awkward to exercise
 * directly. Here it is a plain function over an injected client.
 */

import type {
  Recommendation,
  RecommendationRequest,
  RecommendationResponse,
} from "@ashdev/codex-plugin-sdk";
import type { MangaBakaRecommendationClient } from "./api.js";
import type { DismissalStore } from "./dismissals.js";
import { logger } from "./logger.js";
import { mapRecommendation } from "./mappers.js";
import { resolveSeeds } from "./seeds.js";

/** Upstream cap on `/v1/series/mix`. */
const MIX_LIMIT_MAX = 50;

/** Fallback when the host does not specify a limit. */
const DEFAULT_LIMIT = 20;

/**
 * How far past the requested limit to fetch.
 *
 * Exclusions, dismissals, seed echoes, and (from the next stage) related-work
 * filtering all remove candidates after the response arrives. Requesting
 * exactly `limit` would leave the user short by however many were dropped.
 */
export const MIX_OVERFETCH_FACTOR = 3;

/** An empty, well-formed response. */
function emptyResponse(): RecommendationResponse {
  return { recommendations: [], generatedAt: new Date().toISOString(), cached: false };
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
): Promise<RecommendationResponse> {
  const { library = [], excludeIds = [] } = params;
  const limit = Math.max(1, params.limit ?? DEFAULT_LIMIT);

  if (library.length === 0) {
    logger.info("Empty library - returning no recommendations");
    return emptyResponse();
  }

  const { seeds, libraryIds } = resolveSeeds(library);
  if (seeds.length === 0) {
    logger.warn(
      "No library entry could be resolved to a MangaBaka series, so there is nothing to seed " +
        "recommendations from. Run a metadata match with the MangaBaka Metadata plugin first.",
    );
    return emptyResponse();
  }

  const seedTitles = new Map(seeds.map((seed) => [seed.id, seed.title]));
  const seedIds = new Set(seeds.map((seed) => seed.id));

  let entries: Awaited<ReturnType<MangaBakaRecommendationClient["mix"]>>;
  try {
    entries = await client.mix({
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

  logger.debug(`Mix returned ${entries.length} candidates for ${seeds.length} seeds`);

  const excluded = new Set(excludeIds);
  // Keyed by external ID so a repeat within one response collapses to its
  // strongest occurrence rather than appearing twice.
  const byExternalId = new Map<string, Recommendation>();

  for (const entry of entries) {
    // A seed is, trivially, similar to itself. Returning one tells the user
    // nothing they did not already know.
    if (seedIds.has(entry.series.id)) continue;

    const recommendation = mapRecommendation(entry, { seedTitles, libraryIds });
    if (!recommendation) continue;

    if (excluded.has(recommendation.externalId)) continue;
    if (dismissed.has(recommendation.externalId)) continue;

    const existing = byExternalId.get(recommendation.externalId);
    if (!existing || recommendation.score > existing.score) {
      byExternalId.set(recommendation.externalId, recommendation);
    }
  }

  const recommendations = [...byExternalId.values()]
    .sort((a, b) => b.score - a.score)
    .slice(0, limit);

  logger.info(`Generated ${recommendations.length} recommendations from ${seeds.length} seeds`);

  return { recommendations, generatedAt: new Date().toISOString(), cached: false };
}
