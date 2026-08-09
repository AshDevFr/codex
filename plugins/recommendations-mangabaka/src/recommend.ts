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
import {
  type CollaborativeHit,
  type CollaborativeOptions,
  fetchCollaborative,
} from "./collaborative.js";
import type { DismissalStore } from "./dismissals.js";
import { type Candidate, collapseFranchises, filterCandidates } from "./filters.js";
import { logger } from "./logger.js";
import { mapRecommendation } from "./mappers.js";
import {
  type AuthorAdjustmentOptions,
  applyAuthorAdjustment,
  type BlendOptions,
  blendScores,
  buildBlendedReason,
  normalizeByMax,
} from "./scoring.js";
import { type LibraryIndex, type ResolvedSeed, resolveSeeds } from "./seeds.js";
import { normalizeTitle } from "./titles.js";
import type { MbContentRating, MbRecommendationEntry, MbSeriesType } from "./types.js";

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

export type GenerateOptions = AuthorAdjustmentOptions &
  BlendOptions &
  CollaborativeOptions & {
    /**
     * Filters to apply upstream.
     *
     * Sent as query parameters rather than applied to the response. MangaBaka
     * exposes the whole filter surface, so a filtered-out series never occupies
     * a slot in the response and the over-fetch is not wasted on results that
     * were going to be discarded. The AniList provider filters after the fact
     * only because AniList has no equivalent parameters.
     */
    filters?: UpstreamFilters;
  };

/** Filters passed straight through to MangaBaka. */
export interface UpstreamFilters {
  contentRating?: MbContentRating[];
  includedTypes?: MbSeriesType[];
  excludedTypes?: MbSeriesType[];
  excludedGenres?: string[];
  /** Already resolved from names to numeric IDs. */
  excludedTagIds?: number[];
  minimumRating?: number;
}

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

interface MergeInput {
  mixEntries: MbRecommendationEntry[];
  collaborative: Map<number, CollaborativeHit>;
  seedTitles: Map<number, string>;
  library: LibraryIndex;
}

/**
 * A candidate that still carries its raw per-signal inputs.
 *
 * Scoring is deferred until after filtering, so those inputs have to survive
 * the filter pass.
 */
interface SignalCandidate extends Candidate {
  /** Raw content-similarity score, un-normalised. */
  rawContent?: number;
  /** Collaborative endorsement, already normalised within its own response. */
  collaborative?: CollaborativeHit;
}

/**
 * Union the two signals into one candidate list, without scoring them yet.
 *
 * Keyed by MangaBaka series ID so a series found by both is one candidate
 * carrying both signals, rather than two competing entries. Where both returned
 * it, the content entry is preferred as the carrier: only the mix response has
 * the `matched_related`, `matched_author`, and `shared_tags` annotations that
 * filtering and reason text depend on.
 */
function mergeSignals({
  mixEntries,
  collaborative,
  seedTitles,
  library,
}: MergeInput): SignalCandidate[] {
  const contentScores = new Map<number, number>();
  const carriers = new Map<number, MbRecommendationEntry>();

  for (const entry of mixEntries) {
    const id = entry.series.id;
    const score = typeof entry.score === "number" ? entry.score : 0;
    // A repeat within one response is a duplicate, not two candidates.
    if (!carriers.has(id) || score > (contentScores.get(id) ?? 0)) {
      carriers.set(id, entry);
      contentScores.set(id, score);
    }
  }

  for (const [id, hit] of collaborative) {
    if (!carriers.has(id)) carriers.set(id, hit.entry);
  }

  const candidates: SignalCandidate[] = [];

  for (const [id, entry] of carriers) {
    const recommendation = mapRecommendation(entry, { seedTitles, library });
    // Merged and deleted tombstones drop here.
    if (!recommendation) continue;

    candidates.push({
      entry,
      recommendation,
      rawContent: contentScores.get(id),
      collaborative: collaborative.get(id),
    });
  }

  return candidates;
}

/**
 * Blend each surviving candidate's signals into its final score and reason.
 *
 * Deliberately runs *after* filtering. Content scores are normalised against
 * the strongest candidate, and the strongest raw content matches are almost
 * always a seed's own spin-offs, which filtering removes. Normalising before
 * that let those doomed entries set the maximum and pushed every genuine
 * content match down to roughly half its true standing, which in a live run was
 * enough to keep content results out of the top twelve entirely.
 */
function scoreCandidates(
  candidates: SignalCandidate[],
  options: GenerateOptions,
): SignalCandidate[] {
  const contentScores = new Map<number, number>();
  for (const candidate of candidates) {
    if (typeof candidate.rawContent === "number") {
      contentScores.set(candidate.entry.series.id, candidate.rawContent);
    }
  }
  const contentNormalized = normalizeByMax(contentScores);

  return candidates.map((candidate) => {
    const contentSeedTitles = candidate.recommendation.basedOn;
    const collaborativeSeedTitles = candidate.collaborative?.seedTitles ?? [];

    return {
      ...candidate,
      recommendation: {
        ...candidate.recommendation,
        score: blendScores(
          {
            content: contentNormalized.get(candidate.entry.series.id),
            collaborative: candidate.collaborative?.score,
          },
          options,
        ),
        reason: buildBlendedReason({
          sharedTags: candidate.entry.shared_tags,
          contentSeedTitles,
          collaborativeSeedTitles,
        }),
        // Both signals are evidence of relevance, so both belong in basedOn.
        basedOn: [...new Set([...contentSeedTitles, ...collaborativeSeedTitles])],
      },
    };
  });
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
  const filters = options.filters ?? {};

  // Both signals are independent, so they go out together. The content probe is
  // the primary one: if it fails there is nothing to recommend, whereas the
  // collaborative fan-out already degrades seed by seed.
  const [mixResult, collaborative] = await Promise.all([
    client
      .mix({
        series: seeds.map((seed) => seed.id),
        limit: Math.min(limit * MIX_OVERFETCH_FACTOR, MIX_LIMIT_MAX),
        // Tag rules should steer the probe vector, not hard-filter candidates
        // out of it. Upstream defaults this to true.
        strict: false,
        contentRating: filters.contentRating,
        tagNot: filters.excludedTagIds,
        type: filters.includedTypes,
        typeNot: filters.excludedTypes,
        genreNot: filters.excludedGenres,
        ratingLower: filters.minimumRating,
      })
      .catch((error: unknown) => {
        const message = error instanceof Error ? error.message : "Unknown error";
        logger.warn(`Mix request failed: ${message}`);
        return null;
      }),
    fetchCollaborative(client, seeds, {
      ...options,
      // The collaborative endpoint accepts only these two of the filters.
      contentRating: filters.contentRating,
      excludedTagIds: filters.excludedTagIds,
    }),
  ]);

  if (mixResult === null && collaborative.size === 0) {
    logger.warn("Both recommendation signals failed, returning nothing");
    return emptyResponse();
  }

  const mixEntries = mixResult ?? [];
  logger.debug(
    `Content probe returned ${mixEntries.length} candidates; collaborative returned ${collaborative.size}`,
  );

  const candidates = mergeSignals({ mixEntries, collaborative, seedTitles, library });

  const filtered = filterCandidates(candidates, {
    seedIds,
    seedTitleKeys: seedTitleKeys(entries, seeds),
    excludeIds: new Set(excludeIds),
    isDismissed: (externalId) => dismissed.has(externalId),
    // Redundant for content candidates, which were filtered upstream, but the
    // collaborative endpoint only supports two of these parameters.
    userFilters: filters,
  });

  // Scoring after filtering, so the doomed franchise entries do not set the
  // content normalisation maximum.
  const blended = scoreCandidates(filtered, options);

  // Author adjustment runs before the franchise collapse so that a de-ranked
  // same-author entry cannot win its group over a stronger unrelated sibling.
  const scored = applyAuthorAdjustment(blended, options);

  // No separate de-duplication pass: mergeSignals is keyed by series ID, so
  // repeats within or across the two responses have already collapsed.
  const collapsed = collapseFranchises(scored);

  const recommendations = collapsed
    .map((candidate) => candidate.recommendation)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit);

  logger.info(
    `Generated ${recommendations.length} recommendations from ${seeds.length} seeds ` +
      `(${candidates.length} candidates, ${candidates.length - filtered.length} filtered out)`,
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
