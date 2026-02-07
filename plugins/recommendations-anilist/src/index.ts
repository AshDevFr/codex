/**
 * AniList Recommendations Plugin for Codex
 *
 * Generates personalized manga recommendations by:
 * 1. Matching user's library entries to AniList manga IDs
 * 2. Fetching community recommendations for highly-rated titles
 * 3. Scoring and deduplicating results
 * 4. Returning the top recommendations
 *
 * Communicates via JSON-RPC over stdio using the Codex plugin SDK.
 */

import {
  createLogger,
  createRecommendationPlugin,
  type InitializeParams,
  type Recommendation,
  type RecommendationDismissRequest,
  type RecommendationDismissResponse,
  type RecommendationProvider,
  type RecommendationRequest,
  type RecommendationResponse,
  type UserLibraryEntry,
} from "@ashdev/codex-plugin-sdk";
import {
  AniListRecommendationClient,
  type AniListRecommendationNode,
  getBestTitle,
  stripHtml,
} from "./anilist.js";
import { manifest } from "./manifest.js";

const logger = createLogger({ name: "recommendations-anilist", level: "debug" });

// Plugin state (set during initialization)
let client: AniListRecommendationClient | null = null;
let viewerId: number | null = null;
let maxRecommendations = 20;

// Dismissed recommendations (in-memory per process lifetime)
const dismissedIds = new Set<string>();

// =============================================================================
// Recommendation Generation
// =============================================================================

/**
 * Find AniList IDs for library entries.
 * Tries external_ids first, falls back to title search.
 */
async function resolveAniListIds(
  entries: UserLibraryEntry[],
): Promise<Map<string, { anilistId: number; title: string; rating: number }>> {
  if (!client) throw new Error("Plugin not initialized");

  const resolved = new Map<string, { anilistId: number; title: string; rating: number }>();

  for (const entry of entries) {
    // Check if we already have an AniList external ID
    // Prefer api:anilist (new convention), fall back to legacy source names
    const anilistExt = entry.externalIds?.find(
      (e) => e.source === "api:anilist" || e.source === "anilist" || e.source === "AniList",
    );

    if (anilistExt) {
      const id = Number.parseInt(anilistExt.id, 10);
      if (!Number.isNaN(id)) {
        resolved.set(entry.seriesId, {
          anilistId: id,
          title: entry.title,
          rating: entry.userRating ?? 0,
        });
        continue;
      }
    }

    // Fall back to title search
    const result = await client.searchManga(entry.title);
    if (result) {
      resolved.set(entry.seriesId, {
        anilistId: result.id,
        title: entry.title,
        rating: entry.userRating ?? 0,
      });
    }
  }

  return resolved;
}

/**
 * Pick the best entries from the user's library to seed recommendations.
 * Prioritizes highly-rated, recently-read titles.
 */
function pickSeedEntries(entries: UserLibraryEntry[], maxSeeds: number): UserLibraryEntry[] {
  // Sort by rating (desc), then by recency
  const sorted = [...entries].sort((a, b) => {
    const ratingDiff = (b.userRating ?? 0) - (a.userRating ?? 0);
    if (ratingDiff !== 0) return ratingDiff;
    // Fall back to books read as a proxy for engagement
    return b.booksRead - a.booksRead;
  });

  return sorted.slice(0, maxSeeds);
}

/**
 * Convert AniList recommendation nodes into Recommendation objects.
 */
function convertRecommendations(
  nodes: AniListRecommendationNode[],
  basedOnTitle: string,
  userMangaIds: Set<number>,
  excludeIds: Set<string>,
): Recommendation[] {
  const results: Recommendation[] = [];

  for (const node of nodes) {
    if (!node.mediaRecommendation) continue;

    const media = node.mediaRecommendation;
    const externalId = String(media.id);

    // Skip if excluded or dismissed
    if (excludeIds.has(externalId) || dismissedIds.has(externalId)) continue;

    const inLibrary = userMangaIds.has(media.id);

    // Compute a relevance score based on community rating and AniList average score
    const communityScore = Math.max(0, Math.min(node.rating, 100)) / 100;
    const avgScore = media.averageScore ? media.averageScore / 100 : 0.5;
    const score = Math.round((communityScore * 0.6 + avgScore * 0.4) * 100) / 100;

    results.push({
      externalId,
      externalUrl: media.siteUrl,
      title: getBestTitle(media.title),
      coverUrl: media.coverImage.large ?? undefined,
      summary: stripHtml(media.description),
      genres: media.genres ?? [],
      score: Math.max(0, Math.min(score, 1)),
      reason: `Recommended because you liked ${basedOnTitle}`,
      basedOn: [basedOnTitle],
      inLibrary,
    });
  }

  return results;
}

// =============================================================================
// Provider Implementation
// =============================================================================

const provider: RecommendationProvider = {
  async get(params: RecommendationRequest): Promise<RecommendationResponse> {
    if (!client) {
      throw new Error("Plugin not initialized - no AniList client");
    }

    if (viewerId === null) {
      viewerId = await client.getViewerId();
      logger.info(`Authenticated as viewer ${viewerId}`);
    }

    const { library, limit, excludeIds: rawExcludeIds = [] } = params;
    const effectiveLimit = Math.min(limit ?? maxRecommendations, 50);
    const excludeIds = new Set(rawExcludeIds);

    // Get user's existing manga IDs for dedup
    const userMangaIds = await client.getUserMangaIds(viewerId);
    logger.debug(`User has ${userMangaIds.size} manga in AniList list`);

    // Pick seed entries (top-rated from user's library)
    const maxSeeds = 10;
    const seeds = pickSeedEntries(library, maxSeeds);
    logger.debug(`Using ${seeds.length} seed entries from library of ${library.length}`);

    // Resolve AniList IDs for seed entries
    const resolved = await resolveAniListIds(seeds);
    logger.debug(`Resolved ${resolved.size} AniList IDs from ${seeds.length} seeds`);

    // Fetch recommendations for each seed
    const allRecs = new Map<string, Recommendation>();

    for (const [, { anilistId, title }] of resolved) {
      try {
        const nodes = await client.getRecommendationsForMedia(anilistId, 10);
        const recs = convertRecommendations(nodes, title, userMangaIds, excludeIds);

        for (const rec of recs) {
          // If we've seen this recommendation before, merge basedOn and keep higher score
          const existing = allRecs.get(rec.externalId);
          if (existing) {
            // Merge basedOn titles
            const mergedBasedOn = [...new Set([...existing.basedOn, ...rec.basedOn])];
            // Boost score slightly for multiply-recommended titles
            const boostedScore = Math.min(existing.score + 0.05, 1.0);
            allRecs.set(rec.externalId, {
              ...existing,
              score: Math.round(boostedScore * 100) / 100,
              basedOn: mergedBasedOn,
              reason:
                mergedBasedOn.length > 1
                  ? `Recommended based on ${mergedBasedOn.join(", ")}`
                  : existing.reason,
            });
          } else {
            allRecs.set(rec.externalId, rec);
          }
        }
      } catch (error) {
        const msg = error instanceof Error ? error.message : "Unknown error";
        logger.warn(`Failed to get recommendations for AniList ID ${anilistId}: ${msg}`);
      }
    }

    // Sort by score descending and take top N
    const sorted = [...allRecs.values()].sort((a, b) => b.score - a.score).slice(0, effectiveLimit);

    logger.info(`Generated ${sorted.length} recommendations from ${resolved.size} seed titles`);

    return {
      recommendations: sorted,
      generatedAt: new Date().toISOString(),
      cached: false,
    };
  },

  async dismiss(params: RecommendationDismissRequest): Promise<RecommendationDismissResponse> {
    dismissedIds.add(params.externalId);
    logger.debug(
      `Dismissed recommendation: ${params.externalId} (reason: ${params.reason ?? "none"})`,
    );
    return { dismissed: true };
  },
};

// =============================================================================
// Plugin Initialization
// =============================================================================

createRecommendationPlugin({
  manifest,
  provider,
  logLevel: "debug",
  onInitialize(params: InitializeParams) {
    const accessToken = params.credentials?.access_token;
    if (accessToken) {
      client = new AniListRecommendationClient(accessToken);
      logger.info("AniList client initialized with access token");
    } else {
      logger.warn("No access token provided - recommendation operations will fail");
    }

    if (params.config?.maxRecommendations) {
      maxRecommendations = params.config.maxRecommendations as number;
      logger.info(`Max recommendations set to: ${maxRecommendations}`);
    }
  },
});

logger.info("AniList recommendations plugin started");
