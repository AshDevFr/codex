/**
 * The collaborative signal: what readers of your favourites also read.
 *
 * Tag-vector similarity can only ever return neighbours of what the library
 * already contains, so on its own it produces "more of the same" with no
 * serendipity. `/v1/series/{id}/readers-also-like` is derived from shared
 * library activity instead of content, which is the one thing the content
 * probe structurally cannot supply.
 *
 * Unlike `/mix`, this endpoint takes a single series, so it needs one request
 * per seed. The fan-out is bounded on both axes: only the top-rated seeds are
 * queried, and only a few requests run at a time. The 1-day edge cache makes
 * repeat calls cheap.
 */

import type { MangaBakaRecommendationClient } from "./api.js";
import { logger } from "./logger.js";
import type { ResolvedSeed } from "./seeds.js";
import type { MbContentRating, MbRecommendationEntry } from "./types.js";

/** How many of the user's top-rated seeds to query by default. */
export const DEFAULT_COLLABORATIVE_SEEDS = 5;

/** How many collaborative requests may be in flight at once. */
export const COLLABORATIVE_CONCURRENCY = 3;

/** How many results to request per seed. */
const PER_SEED_LIMIT = 24;

export interface CollaborativeOptions {
  /** How many top-rated seeds to query. Zero disables the signal entirely. */
  collaborativeSeeds?: number;
  /**
   * Allowed content ratings, passed upstream.
   *
   * This endpoint accepts only `content_rating` and `tag_not` of the filter
   * surface `/mix` supports; the rest have no equivalent here and are applied
   * to the content probe only.
   */
  contentRating?: MbContentRating[];
  /** Tag IDs to exclude, passed upstream. */
  excludedTagIds?: number[];
}

/** A series endorsed by the reading habits of one or more seeds' readers. */
export interface CollaborativeHit {
  entry: MbRecommendationEntry;
  /** Strongest normalised endorsement across the seeds that returned it, 0-1. */
  score: number;
  /** Codex titles of the seeds whose readers also read this. */
  seedTitles: string[];
}

/** Run tasks with a bounded number in flight, preserving no particular order. */
async function withConcurrency<T>(
  tasks: Array<() => Promise<T>>,
  limit: number,
): Promise<PromiseSettledResult<T>[]> {
  const results: PromiseSettledResult<T>[] = new Array(tasks.length);
  let next = 0;

  const worker = async (): Promise<void> => {
    while (true) {
      const index = next++;
      if (index >= tasks.length) return;
      try {
        results[index] = { status: "fulfilled", value: await tasks[index]() };
      } catch (reason) {
        results[index] = { status: "rejected", reason };
      }
    }
  };

  await Promise.all(Array.from({ length: Math.min(limit, tasks.length) }, worker));
  return results;
}

/**
 * Fetch and normalise collaborative recommendations for the top-rated seeds.
 *
 * Scores are normalised per response. The raw values are unbounded
 * co-occurrence weights whose scale depends on how widely read the source
 * series is: a live sample returned top scores of 10, 46, and 306 for three
 * different seeds. Comparing those directly would let one popular seed
 * dominate purely because more people have read it.
 */
export async function fetchCollaborative(
  client: MangaBakaRecommendationClient,
  seeds: ResolvedSeed[],
  options: CollaborativeOptions = {},
): Promise<Map<number, CollaborativeHit>> {
  const hits = new Map<number, CollaborativeHit>();

  const wanted = options.collaborativeSeeds ?? DEFAULT_COLLABORATIVE_SEEDS;
  if (wanted <= 0 || seeds.length === 0) return hits;

  // Seeds arrive in the host's order, which is rating descending, so the first
  // K are the user's favourites without needing a re-sort.
  const chosen = seeds.slice(0, wanted);
  const seedIds = new Set(seeds.map((seed) => seed.id));

  const settled = await withConcurrency(
    chosen.map((seed) => async () => ({
      seed,
      entries: await client.readersAlsoLike(seed.id, {
        limit: PER_SEED_LIMIT,
        contentRating: options.contentRating,
        tagNot: options.excludedTagIds,
      }),
    })),
    COLLABORATIVE_CONCURRENCY,
  );

  let failures = 0;

  for (const result of settled) {
    if (result.status === "rejected") {
      failures++;
      const message =
        result.reason instanceof Error ? result.reason.message : String(result.reason);
      // One unavailable seed costs some breadth, not the whole run.
      logger.warn(`Collaborative lookup failed for one seed: ${message}`);
      continue;
    }

    const { seed, entries } = result.value;

    const maxScore = entries.reduce(
      (max, entry) => (typeof entry.score === "number" ? Math.max(max, entry.score) : max),
      0,
    );
    if (maxScore <= 0) continue;

    for (const entry of entries) {
      const raw = entry.score;
      if (typeof raw !== "number" || raw <= 0) continue;

      // Seeds routinely appear in each other's collaborative lists.
      if (seedIds.has(entry.series.id)) continue;

      const score = Math.round((raw / maxScore) * 100) / 100;
      const existing = hits.get(entry.series.id);

      if (!existing) {
        hits.set(entry.series.id, { entry, score, seedTitles: [seed.title] });
        continue;
      }

      existing.seedTitles.push(seed.title);
      if (score > existing.score) {
        existing.score = score;
        existing.entry = entry;
      }
    }
  }

  logger.debug(
    `Collaborative signal: ${hits.size} series from ${chosen.length - failures}/${chosen.length} seeds`,
  );

  return hits;
}
