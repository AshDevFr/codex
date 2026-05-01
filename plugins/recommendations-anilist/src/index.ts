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
  type PluginStorage,
  type Recommendation,
  type RecommendationClearResponse,
  type RecommendationDismissRequest,
  type RecommendationDismissResponse,
  type RecommendationProvider,
  type RecommendationRequest,
  type RecommendationResponse,
  type SeriesStatus,
  type UserLibraryEntry,
} from "@ashdev/codex-plugin-sdk";
import {
  type AniListMediaStatus,
  AniListRecommendationClient,
  type AniListRecommendationNode,
  getBestTitle,
  stripHtml,
} from "./anilist.js";
import { EXTERNAL_ID_SOURCE_ANILIST, manifest } from "./manifest.js";

const logger = createLogger({ name: "recommendations-anilist", level: "debug" });

// =============================================================================
// Filter Configuration
// =============================================================================

/** Plugin-side filters applied during recommendation generation */
export interface RecommendationFilters {
  /** Allowed country codes (empty = no filter) */
  allowedCountries: Set<string>;
  /** Genres to exclude */
  excludedGenres: Set<string>;
  /** Formats to exclude (e.g. "NOVEL", "ONE_SHOT") */
  excludedFormats: Set<string>;
  /** Minimum AniList average score (0-100, 0 = disabled) */
  minAniListScore: number;
}

const DEFAULT_FILTERS: RecommendationFilters = {
  allowedCountries: new Set(),
  excludedGenres: new Set(),
  excludedFormats: new Set(),
  minAniListScore: 0,
};

/** Parse a comma-separated string into a Set of trimmed, uppercased values */
function parseCommaSet(value: unknown): Set<string> {
  if (typeof value !== "string" || value.trim() === "") return new Set();
  return new Set(
    value
      .split(",")
      .map((s) => s.trim().toUpperCase())
      .filter((s) => s.length > 0),
  );
}

/** Parse a comma-separated string into a Set of trimmed values (case-preserved) */
function parseCommaSetPreserveCase(value: unknown): Set<string> {
  if (typeof value !== "string" || value.trim() === "") return new Set();
  return new Set(
    value
      .split(",")
      .map((s) => s.trim())
      .filter((s) => s.length > 0),
  );
}

// Plugin state (set during initialization)
let client: AniListRecommendationClient | null = null;
let viewerId: number | null = null;
let searchFallback = true;
let filters: RecommendationFilters = { ...DEFAULT_FILTERS };
let storage: PluginStorage | null = null;

/** Set the AniList client (exported for testing) */
export function setClient(c: AniListRecommendationClient | null): void {
  client = c;
}

/** Set the searchFallback flag (exported for testing) */
export function setSearchFallback(enabled: boolean): void {
  searchFallback = enabled;
}

/** Set the recommendation filters (exported for testing) */
export function setFilters(f: RecommendationFilters): void {
  filters = f;
}

/** Reset filters to defaults (exported for testing) */
export function resetFilters(): void {
  filters = {
    allowedCountries: new Set(),
    excludedGenres: new Set(),
    excludedFormats: new Set(),
    minAniListScore: 0,
  };
}

/** Storage key for persisted dismissed recommendation IDs */
const DISMISSED_STORAGE_KEY = "dismissed_ids";

// In-memory cache of dismissed IDs (synced with storage).
// Loaded from storage on initialize, updated on dismiss/clear.
export const dismissedIds = new Set<string>();

/**
 * Load dismissed IDs from persistent storage into the in-memory cache.
 */
async function loadDismissedIds(): Promise<void> {
  if (!storage) return;
  try {
    const result = await storage.get(DISMISSED_STORAGE_KEY);
    if (Array.isArray(result.data)) {
      dismissedIds.clear();
      for (const id of result.data) {
        if (typeof id === "string") {
          dismissedIds.add(id);
        }
      }
      logger.debug(`Loaded ${dismissedIds.size} dismissed IDs from storage`);
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Unknown error";
    logger.warn(`Failed to load dismissed IDs from storage: ${msg}`);
  }
}

/**
 * Persist the current dismissed IDs set to storage.
 */
async function saveDismissedIds(): Promise<void> {
  if (!storage) return;
  try {
    await storage.set(DISMISSED_STORAGE_KEY, [...dismissedIds]);
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Unknown error";
    logger.warn(`Failed to save dismissed IDs to storage: ${msg}`);
  }
}

// =============================================================================
// Recommendation Generation
// =============================================================================

/**
 * Find AniList IDs for library entries.
 * Tries external_ids first, falls back to title search.
 */
export async function resolveAniListIds(
  entries: UserLibraryEntry[],
): Promise<Map<string, { anilistId: number; title: string; rating: number }>> {
  if (!client) throw new Error("Plugin not initialized");

  const resolved = new Map<string, { anilistId: number; title: string; rating: number }>();

  for (const entry of entries) {
    // Check if we already have an AniList external ID
    // Prefer api:anilist (new convention), fall back to legacy source names
    const anilistExt = entry.externalIds?.find(
      (e) =>
        e.source === EXTERNAL_ID_SOURCE_ANILIST || e.source === "anilist" || e.source === "AniList",
    );

    if (anilistExt) {
      const id = Number.parseInt(anilistExt.externalId, 10);
      if (!Number.isNaN(id)) {
        resolved.set(entry.seriesId, {
          anilistId: id,
          title: entry.title,
          rating: entry.userRating ?? 0,
        });
        continue;
      }
    }

    // Fall back to title search (when enabled)
    if (searchFallback) {
      const result = await client.searchManga(entry.title);
      if (result) {
        resolved.set(entry.seriesId, {
          anilistId: result.id,
          title: entry.title,
          rating: entry.userRating ?? 0,
        });
      }
    }
  }

  return resolved;
}

/**
 * Map AniList media status to Codex SeriesStatus.
 * AniList values: FINISHED, RELEASING, NOT_YET_RELEASED, CANCELLED, HIATUS
 */
export function mapAniListStatus(status: AniListMediaStatus | null): SeriesStatus | undefined {
  if (!status) return undefined;
  switch (status) {
    case "RELEASING":
      return "ongoing";
    case "FINISHED":
      return "ended";
    case "HIATUS":
      return "hiatus";
    case "CANCELLED":
      return "abandoned";
    case "NOT_YET_RELEASED":
      return "unknown";
    default:
      return undefined;
  }
}

/**
 * Convert AniList recommendation nodes into Recommendation objects.
 * Applies plugin-side filters (from user config) to exclude unwanted results.
 */
export function convertRecommendations(
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

    // Apply plugin-side filters from user config
    if (
      filters.allowedCountries.size > 0 &&
      (!media.countryOfOrigin || !filters.allowedCountries.has(media.countryOfOrigin.toUpperCase()))
    ) {
      continue;
    }

    if (
      filters.excludedFormats.size > 0 &&
      media.format &&
      filters.excludedFormats.has(media.format.toUpperCase())
    ) {
      continue;
    }

    if (filters.excludedGenres.size > 0 && media.genres) {
      const hasExcludedGenre = media.genres.some((g) => filters.excludedGenres.has(g));
      if (hasExcludedGenre) continue;
    }

    if (filters.minAniListScore > 0 && (media.averageScore ?? 0) < filters.minAniListScore) {
      continue;
    }

    const inLibrary = userMangaIds.has(media.id);

    // Compute a relevance score based on community rating and AniList average score
    const communityScore = Math.max(0, Math.min(node.rating, 100)) / 100;
    const avgScore = media.averageScore ? media.averageScore / 100 : 0.5;
    const score = Math.round((communityScore * 0.6 + avgScore * 0.4) * 100) / 100;

    const status = mapAniListStatus(media.status);
    const totalVolumeCount = media.volumes != null && media.volumes > 0 ? media.volumes : undefined;
    const totalChapterCount =
      media.chapters != null && media.chapters > 0 ? media.chapters : undefined;

    results.push({
      externalId,
      externalUrl: media.siteUrl,
      title: getBestTitle(media.title),
      coverUrl: media.coverImage.large ?? undefined,
      summary: stripHtml(media.description),
      genres: media.genres ?? [],
      tags: media.tags?.map((t) => ({ name: t.name, rank: t.rank, category: t.category })),
      score: Math.max(0, Math.min(score, 1)),
      reason: `Recommended because you liked ${basedOnTitle}`,
      basedOn: [basedOnTitle],
      inLibrary,
      status,
      format: media.format ?? undefined,
      countryOfOrigin: media.countryOfOrigin ?? undefined,
      startYear: media.startDate?.year ?? undefined,
      // Legacy field mirrors the volume count so older Codex versions still
      // see a value; new field is the authoritative one going forward.
      totalBookCount: totalVolumeCount,
      totalVolumeCount,
      totalChapterCount,
      rating: media.averageScore ?? undefined,
      popularity: media.popularity ?? undefined,
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
    const effectiveLimit = Math.min(limit ?? 20, 100);
    const excludeIds = new Set(rawExcludeIds);

    // Library entries are pre-curated seeds from Codex server (rated + recent reads).
    // Return early if no seeds provided.
    if (!library || library.length === 0) {
      logger.info("Empty library — returning no recommendations");
      return { recommendations: [], generatedAt: new Date().toISOString(), cached: false };
    }

    // Get user's existing manga IDs for dedup
    const userMangaIds = await client.getUserMangaIds(viewerId);
    logger.debug(`User has ${userMangaIds.size} manga in AniList list`);

    // Resolve AniList IDs for seed entries (library is already curated by Codex)
    logger.debug(`Using ${library.length} seed entries`);
    const resolved = await resolveAniListIds(library);
    logger.debug(`Resolved ${resolved.size} AniList IDs from ${library.length} seeds`);

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
    await saveDismissedIds();
    return { dismissed: true };
  },

  async clear(): Promise<RecommendationClearResponse> {
    const count = dismissedIds.size;
    dismissedIds.clear();
    logger.info(`Cleared ${count} dismissed recommendations`);
    await saveDismissedIds();
    return { cleared: true };
  },
};

// =============================================================================
// Plugin Initialization
// =============================================================================

createRecommendationPlugin({
  manifest,
  provider,
  logLevel: "debug",
  async onInitialize(params: InitializeParams) {
    const accessToken = params.credentials?.access_token;
    if (accessToken) {
      client = new AniListRecommendationClient(accessToken);
      logger.info("AniList client initialized with access token");
    } else {
      logger.warn("No access token provided - recommendation operations will fail");
    }

    // Read searchFallback from userConfig (default: true — preserve existing behavior)
    const uc = params.userConfig;
    if (uc && typeof uc.searchFallback === "boolean") {
      searchFallback = uc.searchFallback;
      logger.info(`Search fallback set to: ${searchFallback}`);
    }

    // Read recommendation filters from userConfig
    if (uc) {
      filters = {
        allowedCountries: parseCommaSet(uc.allowedCountries),
        excludedGenres: parseCommaSetPreserveCase(uc.excludedGenres),
        excludedFormats: parseCommaSet(uc.excludedFormats),
        minAniListScore:
          typeof uc.minAniListScore === "number"
            ? Math.max(0, Math.min(uc.minAniListScore, 100))
            : 0,
      };
      const activeFilters: string[] = [];
      if (filters.allowedCountries.size > 0)
        activeFilters.push(`countries=[${[...filters.allowedCountries].join(",")}]`);
      if (filters.excludedGenres.size > 0)
        activeFilters.push(`excludedGenres=[${[...filters.excludedGenres].join(",")}]`);
      if (filters.excludedFormats.size > 0)
        activeFilters.push(`excludedFormats=[${[...filters.excludedFormats].join(",")}]`);
      if (filters.minAniListScore > 0) activeFilters.push(`minScore=${filters.minAniListScore}`);
      if (activeFilters.length > 0) {
        logger.info(`Recommendation filters: ${activeFilters.join(", ")}`);
      }
    }

    // Capture the storage client and restore persisted dismissed IDs
    storage = params.storage;
    await loadDismissedIds();
  },
});

logger.info("AniList recommendations plugin started");
