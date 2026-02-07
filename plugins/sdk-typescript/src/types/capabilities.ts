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

// Re-export SyncProvider from the sync module (replaces the former placeholder)
export type { SyncProvider } from "../sync.js";

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
// Future Capabilities (v2)
// =============================================================================

// SyncProvider is now defined in ../sync.ts and re-exported above.

// Re-export RecommendationProvider from the recommendations module
export type { RecommendationProvider } from "../recommendations.js";

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
