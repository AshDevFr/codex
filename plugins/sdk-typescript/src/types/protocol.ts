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
  /** Publication status (for series search results) */
  status?: string;
  /** Genres */
  genres: string[];
  /** Rating (normalized 0-10) */
  rating?: number;
  /** Short description */
  description?: string;
  /** Number of books in the series (if known by the provider) */
  bookCount?: number;
  /** Author names (for book search results) */
  authors?: string[];
  /**
   * Content format discriminator (e.g. `manga`, `novel`, `light_novel`,
   * `manhwa`, `manhua`, `comic`, `webtoon`, `one_shot`, `doujin`,
   * `artbook`).
   *
   * Free-form string so plugins are not locked into an enum that requires
   * Codex core changes when new formats appear. Plugin authors are
   * encouraged to emit lowercase snake_case values from the recommended
   * vocabulary above so the UI can render consistent badges; unknown
   * values still render as a neutral badge.
   */
  format?: string;
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
  /**
   * Expected total number of volumes in the series, when known.
   * Use this for volume-organized libraries.
   */
  totalVolumeCount?: number;
  /**
   * Expected total number of chapters in the series, when known.
   * May be fractional (e.g. 47.5).
   * Use this for chapter-organized libraries.
   */
  totalChapterCount?: number;
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
  /** Authors/writers (structured with roles, or plain strings for backward compat) */
  authors: BookAuthor[];
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

  // External IDs (cross-references to other services)
  /**
   * Cross-reference IDs from other services.
   * Uses the `api:` prefix convention (e.g., "api:anilist", "api:myanimelist").
   *
   * These allow other plugins (sync, recommendations) to match series
   * to external services without needing title-based search.
   */
  externalIds?: ExternalId[];
}

/**
 * Cross-reference ID for a series on an external service.
 *
 * Source naming convention:
 * - `api:<service>` - External API service ID (e.g., "api:anilist", "api:myanimelist")
 * - `plugin:<name>` - Plugin match provenance (managed by Codex, not set by plugins)
 * - No prefix - File/user sources (e.g., "comicinfo", "epub", "manual")
 */
export interface ExternalId {
  /** Source identifier (e.g., "api:anilist", "api:myanimelist", "api:mangadex") */
  source: string;
  /** ID on the external service */
  externalId: string;
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

// =============================================================================
// Book Metadata Types
// =============================================================================

/**
 * Parameters for metadata/book/search method
 *
 * Supports both ISBN lookup and title/author search:
 * - If `isbn` is provided, direct ISBN lookup is attempted first (faster, more accurate)
 * - If only `query` is provided, title/author search is used
 * - If both are provided, ISBN is tried first with query as fallback
 */
export interface BookSearchParams {
  /** ISBN-10 or ISBN-13 (if provided, takes priority over query) */
  isbn?: string;
  /** Search query (title, author, or combined) - used if no ISBN */
  query?: string;
  /** Optional: filter by author name */
  author?: string;
  /** Optional: filter by publication year */
  year?: number;
  /** Maximum number of results to return */
  limit?: number;
  /** Pagination cursor from previous response */
  cursor?: string;
}

/**
 * Parameters for metadata/book/match method (auto-matching)
 */
export interface BookMatchParams {
  /** Book title */
  title: string;
  /** Authors (if known) */
  authors: string[];
  /** ISBN (if available - will be tried first) */
  isbn?: string;
  /** Publication year (if known) */
  year?: number;
  /** Publisher (if known) */
  publisher?: string;
}

/**
 * Full book metadata from a provider
 */
export interface PluginBookMetadata {
  /** External ID from the provider */
  externalId: string;
  /** URL to the book on the provider's website */
  externalUrl: string;

  // Core fields
  /** Primary title */
  title?: string;
  /** Subtitle (e.g., "A Novel") */
  subtitle?: string;
  /** Alternative titles with language info */
  alternateTitles: AlternateTitle[];
  /** Full description/summary */
  summary?: string;
  /** Book type (comic, manga, novel, etc.) */
  bookType?: string;

  // Book-specific fields
  /** Volume number in series */
  volume?: number;
  /** Chapter number (for single-chapter releases) */
  chapter?: number;
  /** Page count */
  pageCount?: number;
  /** Release date (ISO 8601 format) */
  releaseDate?: string;
  /** Publication year */
  year?: number;

  // ISBN and identifiers
  /** Primary ISBN (ISBN-13 preferred) */
  isbn?: string;
  /** All ISBNs (ISBN-10 and ISBN-13) */
  isbns: string[];

  // Translation/Edition info
  /** Edition information (e.g., "First Edition", "Revised") */
  edition?: string;
  /** Original title (for translations) */
  originalTitle?: string;
  /** Original publication year */
  originalYear?: number;
  /** Translator name */
  translator?: string;
  /** BCP47 language code (e.g., "en", "ja", "ko") */
  language?: string;

  // Series position
  /** Position in series (e.g., 1.0, 1.5 for specials) */
  seriesPosition?: number;
  /** Total number of books in series (if known) */
  seriesTotal?: number;

  // Taxonomy
  /** Genres (e.g., "Science Fiction", "Romance") */
  genres: string[];
  /** Tags/themes (e.g., "Time Travel", "Space Exploration") */
  tags: string[];
  /** Subjects/topics (library classification) */
  subjects: string[];

  // Credits
  /** Structured authors with roles */
  authors: BookAuthor[];
  /** Artists (for comics/manga) */
  artists: string[];
  /** Publisher name */
  publisher?: string;

  // Media
  /** Primary cover URL (for backwards compatibility) */
  coverUrl?: string;
  /** Multiple covers with different sizes/sources */
  covers: BookCover[];

  // Rating
  /** Primary external rating */
  rating?: ExternalRating;
  /** Multiple external ratings from different sources */
  externalRatings: ExternalRating[];

  // Awards
  /** Awards received */
  awards: BookAward[];

  // External links
  /** Links to other sites */
  externalLinks: ExternalLink[];

  // External IDs (cross-references to other services)
  /**
   * Cross-reference IDs from other services.
   * Uses the `api:` prefix convention (e.g., "api:openlibrary").
   *
   * These allow other plugins (sync, recommendations) to match books
   * to external services without needing title-based search.
   */
  externalIds?: ExternalId[];
}

/**
 * Structured author with role information
 */
export interface BookAuthor {
  /** Author's display name */
  name: string;
  /** Author's role */
  role: BookAuthorRole;
  /** Author's name in sort order (e.g., "Doe, Jane") */
  sortName?: string;
}

/**
 * Author role in a book
 */
export type BookAuthorRole =
  | "author"
  | "co_author"
  | "editor"
  | "translator"
  | "illustrator"
  | "contributor"
  | "writer"
  | "penciller"
  | "inker"
  | "colorist"
  | "letterer"
  | "cover_artist";

/**
 * Book cover with size and source information
 */
export interface BookCover {
  /** URL to download the cover image */
  url: string;
  /** Image width in pixels (if known) */
  width?: number;
  /** Image height in pixels (if known) */
  height?: number;
  /** Size hint for cover */
  size?: BookCoverSize;
}

/**
 * Cover size hint
 */
export type BookCoverSize = "small" | "medium" | "large";

/**
 * Book award information
 */
export interface BookAward {
  /** Award name (e.g., "Hugo Award") */
  name: string;
  /** Year the award was given */
  year?: number;
  /** Award category (e.g., "Best Novel") */
  category?: string;
  /** Whether the book won (true) or was nominated (false) */
  won: boolean;
}
