/**
 * Protocol types - these MUST match the Rust protocol exactly
 *
 * These types define the JSON-RPC protocol contract between plugins and Codex.
 * Field names use camelCase to match JSON serialization.
 *
 * @see src/services/plugin/protocol.rs in the Codex backend
 */

// =============================================================================
// Metadata Search Types
// =============================================================================

/**
 * Parameters for metadata/series/search method (and future metadata/book/search)
 */
export interface MetadataSearchParams {
  /** Search query string */
  query: string;
  /** Maximum number of results to return */
  limit?: number;
  /** Pagination cursor from previous response */
  cursor?: string;
}

/**
 * Response from metadata/series/search method (and future metadata/book/search)
 */
export interface MetadataSearchResponse {
  /** Search results */
  results: SearchResult[];
  /** Cursor for next page (if more results available) */
  nextCursor?: string;
}

/**
 * Individual search result
 */
export interface SearchResult {
  /** External ID from the provider */
  externalId: string;
  /** Primary title */
  title: string;
  /** Alternative titles */
  alternateTitles: string[];
  /** Year of publication/release */
  year?: number;
  /** Cover image URL */
  coverUrl?: string;
  /** Relevance score (0.0-1.0, where 1.0 is perfect match) */
  relevanceScore?: number;
  /** Preview data for displaying in search results */
  preview?: SearchResultPreview;
}

/**
 * Preview data shown in search result list
 */
export interface SearchResultPreview {
  /** Publication status */
  status?: string;
  /** Genres */
  genres: string[];
  /** Rating (normalized 0-10) */
  rating?: number;
  /** Short description */
  description?: string;
}

// =============================================================================
// Metadata Get Types
// =============================================================================

/**
 * Parameters for metadata/series/get method (and future metadata/book/get)
 */
export interface MetadataGetParams {
  /** External ID from the provider */
  externalId: string;
}

/**
 * Full series metadata from a provider
 */
export interface PluginSeriesMetadata {
  /** External ID from the provider */
  externalId: string;
  /** URL to the series on the provider's website */
  externalUrl: string;

  // Core fields (all optional)
  /** Primary title */
  title?: string;
  /** Alternative titles with language info */
  alternateTitles: AlternateTitle[];
  /** Full description/summary */
  summary?: string;
  /** Publication status */
  status?: SeriesStatus;
  /** Year of first publication */
  year?: number;

  // Extended metadata
  /** Expected total number of books in the series */
  totalBookCount?: number;
  /** BCP47 language code (e.g., "en", "ja", "ko") */
  language?: string;
  /** Age rating (e.g., 0, 13, 16, 18) */
  ageRating?: number;
  /** Reading direction: "ltr", "rtl", or "ttb" */
  readingDirection?: ReadingDirection;

  // Taxonomy
  /** Genres (e.g., "Action", "Romance") */
  genres: string[];
  /** Tags/themes (e.g., "Time Travel", "School Life") */
  tags: string[];

  // Credits
  /** Authors/writers */
  authors: string[];
  /** Artists (if different from authors) */
  artists: string[];
  /** Publisher name */
  publisher?: string;

  // Media
  /** Cover image URL */
  coverUrl?: string;
  /** Banner/background image URL */
  bannerUrl?: string;

  // Rating
  /** External rating information (primary rating) */
  rating?: ExternalRating;
  /** Multiple external ratings from different sources (e.g., AniList, MAL) */
  externalRatings?: ExternalRating[];

  // External links
  /** Links to other sites */
  externalLinks: ExternalLink[];
}

/**
 * Alternate title with language info
 */
export interface AlternateTitle {
  /** The title text */
  title: string;
  /** ISO 639-1 language code (e.g., "en", "ja") */
  language?: string;
  /** Title type (e.g., "romaji", "native", "english") */
  titleType?: string;
}

/**
 * Series publication status
 *
 * These values MUST match the backend's canonical SeriesStatus enum.
 * @see src/db/entities/series_metadata.rs in the Codex backend
 */
export type SeriesStatus = "ongoing" | "ended" | "hiatus" | "abandoned" | "unknown";

/**
 * Reading direction for content
 */
export type ReadingDirection = "ltr" | "rtl" | "ttb";

/**
 * External rating from provider
 */
export interface ExternalRating {
  /** Normalized score (0-100) */
  score: number;
  /** Number of votes */
  voteCount?: number;
  /** Source name (e.g., "mangaupdates") */
  source: string;
}

/**
 * External link to other sites
 */
export interface ExternalLink {
  /** URL */
  url: string;
  /** Display label */
  label: string;
  /** Link type */
  linkType?: ExternalLinkType;
}

/**
 * Type of external link
 */
export type ExternalLinkType = "provider" | "official" | "social" | "purchase" | "read" | "other";

// =============================================================================
// Metadata Match Types
// =============================================================================

/**
 * Parameters for metadata/series/match method (and future metadata/book/match)
 */
export interface MetadataMatchParams {
  /** Title to match against */
  title: string;
  /** Year hint for matching */
  year?: number;
  /** Author hint for matching */
  author?: string;
}

/**
 * Response from metadata/series/match method (and future metadata/book/match)
 */
export interface MetadataMatchResponse {
  /** Best match result, or null if no confident match */
  match: SearchResult | null;
  /** Confidence score (0.0-1.0) */
  confidence: number;
  /** Alternative matches if confidence is low */
  alternatives?: SearchResult[];
}
