/**
 * Capability interfaces - type-safe contracts for plugin capabilities
 *
 * Plugins declare which content types they support in their manifest's
 * capabilities.metadataProvider array. The SDK automatically routes
 * scoped methods (e.g., metadata/series/search) to the provider.
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
  MetadataGetParams,
  MetadataMatchParams,
  MetadataMatchResponse,
  MetadataSearchParams,
  MetadataSearchResponse,
  PluginSeriesMetadata,
} from "./protocol.js";

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
 * Interface for plugins that provide metadata.
 *
 * Plugins implementing this capability can:
 * - Search for content by query
 * - Get full metadata by external ID
 * - Optionally match existing content to provider entries
 *
 * The same interface is used for both series and book metadata.
 * The content type is determined by the method being called:
 * - metadata/series/search -> provider.search()
 * - metadata/book/search -> provider.search() (when book support is added)
 */
export interface MetadataProvider {
  /**
   * Search for content matching a query
   *
   * @param params - Search parameters
   * @returns Search results with relevance scores
   */
  search(params: MetadataSearchParams): Promise<MetadataSearchResponse>;

  /**
   * Get full metadata for a specific external ID
   *
   * @param params - Get parameters including external ID
   * @returns Full metadata
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

// =============================================================================
// Future Capabilities (v2)
// =============================================================================

/**
 * Interface for plugins that sync reading progress (syncProvider: true)
 * @future v2 - Methods will be defined when sync capability is implemented
 */
// biome-ignore lint/suspicious/noEmptyInterface: Placeholder for future v2 capability
export interface SyncProvider {}

/**
 * Interface for plugins that provide recommendations (recommendationProvider: true)
 * @future v2 - Methods will be defined when recommendation capability is implemented
 */
// biome-ignore lint/suspicious/noEmptyInterface: Placeholder for future v2 capability
export interface RecommendationProvider {}

// =============================================================================
// Type Helpers
// =============================================================================

/**
 * Partial metadata provider - allows implementing only some methods
 * Use this for testing or gradual implementation
 */
export type PartialMetadataProvider = Partial<MetadataProvider>;

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
