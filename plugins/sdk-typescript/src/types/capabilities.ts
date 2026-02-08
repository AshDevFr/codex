/**
 * Capability interfaces - type-safe contracts for plugin capabilities
 *
 * All provider interfaces live here. Plugins declare which capabilities
 * they support in their manifest, and implement the corresponding interface.
 *
 * @example
 * ```typescript
 * // If manifest has capabilities.metadataProvider: ["series"],
 * // the plugin must implement MetadataProvider
 * const provider: MetadataProvider = {
 *   search: async (params) => { ... },
 *   get: async (params) => { ... },
 *   match: async (params) => { ... }, // optional
 * };
 * ```
 */

import type {
  BookMatchParams,
  BookSearchParams,
  MetadataGetParams,
  MetadataMatchParams,
  MetadataMatchResponse,
  MetadataSearchParams,
  MetadataSearchResponse,
  PluginBookMetadata,
  PluginSeriesMetadata,
} from "./protocol.js";
import type {
  ProfileUpdateRequest,
  ProfileUpdateResponse,
  RecommendationClearResponse,
  RecommendationDismissRequest,
  RecommendationDismissResponse,
  RecommendationRequest,
  RecommendationResponse,
} from "./recommendations.js";
import type {
  ExternalUserInfo,
  SyncPullRequest,
  SyncPullResponse,
  SyncPushRequest,
  SyncPushResponse,
  SyncStatusResponse,
} from "./sync.js";

// =============================================================================
// Content Types
// =============================================================================

/**
 * Content types that a metadata provider can support.
 * Plugins declare which types they support in capabilities.metadataProvider.
 */
export type MetadataContentType = "series" | "book";

// =============================================================================
// Metadata Provider Capability
// =============================================================================

/**
 * Interface for plugins that provide series metadata.
 *
 * Plugins implementing this capability can:
 * - Search for series by query
 * - Get full metadata by external ID
 * - Optionally match existing series to provider entries
 */
export interface MetadataProvider {
  /**
   * Search for series matching a query
   *
   * @param params - Search parameters
   * @returns Search results with relevance scores
   */
  search(params: MetadataSearchParams): Promise<MetadataSearchResponse>;

  /**
   * Get full metadata for a specific external ID
   *
   * @param params - Get parameters including external ID
   * @returns Full series metadata
   */
  get(params: MetadataGetParams): Promise<PluginSeriesMetadata>;

  /**
   * Find the best match for existing content (optional)
   *
   * This is used for auto-matching during library scans.
   * If not implemented, Codex will use search() and pick the top result.
   *
   * @param params - Match parameters including title and hints
   * @returns Best match with confidence score
   */
  match?(params: MetadataMatchParams): Promise<MetadataMatchResponse>;
}

/**
 * Interface for plugins that provide book metadata.
 *
 * Plugins implementing this capability can:
 * - Search for books by ISBN or title/author
 * - Get full metadata by external ID
 * - Optionally match existing books to provider entries
 */
export interface BookMetadataProvider {
  /**
   * Search for books matching a query or ISBN
   *
   * @param params - Search parameters (isbn, query, author, year)
   * @returns Search results with relevance scores
   */
  search(params: BookSearchParams): Promise<MetadataSearchResponse>;

  /**
   * Get full book metadata for a specific external ID
   *
   * @param params - Get parameters including external ID
   * @returns Full book metadata
   */
  get(params: MetadataGetParams): Promise<PluginBookMetadata>;

  /**
   * Find the best match for an existing book (optional)
   *
   * This is used for auto-matching during library scans.
   * ISBN matching is tried first if provided, then title/author fallback.
   * If not implemented, Codex will use search() and pick the top result.
   *
   * @param params - Match parameters including title, authors, ISBN
   * @returns Best match with confidence score
   */
  match?(params: BookMatchParams): Promise<MetadataMatchResponse>;
}

// =============================================================================
// Sync Provider Capability
// =============================================================================

/**
 * Interface for plugins that sync reading progress.
 *
 * Plugins implementing this capability can push and pull reading progress
 * between Codex and external services (e.g., AniList, MyAnimeList).
 *
 * Declare this capability in the plugin manifest with `userReadSync: true`.
 *
 * @example
 * ```typescript
 * const provider: SyncProvider = {
 *   async getUserInfo() {
 *     return {
 *       externalId: "12345",
 *       username: "manga_reader",
 *       avatarUrl: "https://anilist.co/img/avatar.jpg",
 *       profileUrl: "https://anilist.co/user/manga_reader",
 *     };
 *   },
 *   async pushProgress(params) {
 *     // Push entries to external service
 *     return { success: [], failed: [] };
 *   },
 *   async pullProgress(params) {
 *     // Pull entries from external service
 *     return { entries: [], hasMore: false };
 *   },
 * };
 * ```
 */
export interface SyncProvider {
  /**
   * Get user info from the external service.
   *
   * Returns the user's identity on the external service.
   * Used to display the connected account in the UI.
   *
   * @returns External user information
   */
  getUserInfo(): Promise<ExternalUserInfo>;

  /**
   * Push reading progress to the external service.
   *
   * Sends one or more reading progress entries from Codex to the
   * external service. Returns results indicating which entries
   * were created, updated, unchanged, or failed.
   *
   * @param params - Push request with entries to sync
   * @returns Push results with success and failure details
   */
  pushProgress(params: SyncPushRequest): Promise<SyncPushResponse>;

  /**
   * Pull reading progress from the external service.
   *
   * Retrieves reading progress entries from the external service.
   * Supports pagination via cursor and incremental sync via `since`.
   *
   * @param params - Pull request with optional filters and pagination
   * @returns Pull results with entries and pagination info
   */
  pullProgress(params: SyncPullRequest): Promise<SyncPullResponse>;

  /**
   * Get sync status overview (optional).
   *
   * Provides a summary of the sync state between Codex and the
   * external service, including pending operations and conflicts.
   *
   * @returns Sync status information
   */
  status?(): Promise<SyncStatusResponse>;
}

// =============================================================================
// Recommendation Provider Capability
// =============================================================================

/**
 * Interface for plugins that provide recommendations.
 *
 * Plugins implementing this capability generate personalized suggestions
 * based on a user's library and reading history.
 *
 * Declare this capability in the plugin manifest with `userRecommendationProvider: true`.
 */
export interface RecommendationProvider {
  /** Get personalized recommendations */
  get(params: RecommendationRequest): Promise<RecommendationResponse>;
  /** Update the user's taste profile from new activity */
  updateProfile?(params: ProfileUpdateRequest): Promise<ProfileUpdateResponse>;
  /** Clear cached recommendations */
  clear?(): Promise<RecommendationClearResponse>;
  /** Dismiss a recommendation */
  dismiss?(params: RecommendationDismissRequest): Promise<RecommendationDismissResponse>;
}

// =============================================================================
// Type Helpers
// =============================================================================

/**
 * Partial series metadata provider - allows implementing only some methods
 * Use this for testing or gradual implementation
 */
export type PartialMetadataProvider = Partial<MetadataProvider>;

/**
 * Partial book metadata provider - allows implementing only some methods
 * Use this for testing or gradual implementation
 */
export type PartialBookMetadataProvider = Partial<BookMetadataProvider>;

// =============================================================================
// Backwards Compatibility (deprecated)
// =============================================================================

/**
 * @deprecated Use MetadataProvider instead
 */
export type SeriesMetadataProvider = MetadataProvider;

/**
 * @deprecated Use PartialMetadataProvider instead
 */
export type PartialSeriesMetadataProvider = PartialMetadataProvider;
