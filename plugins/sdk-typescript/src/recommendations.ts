/**
 * Recommendation provider types
 *
 * Matches the Rust-side types in src/services/plugin/recommendations.rs
 */

// =============================================================================
// UserLibraryEntry (matches Rust UserLibraryEntry in protocol.rs)
// =============================================================================

/** An entry in the user's library, sent to the plugin for context */
export interface UserLibraryEntry {
  /** Codex series ID */
  seriesId: string;
  /** Primary title */
  title: string;
  /** Alternate titles */
  alternateTitles: string[];
  /** Year of publication */
  year?: number;
  /** Series status (e.g., "ongoing", "completed") */
  status?: string;
  /** Genres */
  genres: string[];
  /** Tags */
  tags: string[];
  /** Total number of books in the series */
  totalBookCount?: number;
  /** External IDs from metadata providers */
  externalIds: Array<{ source: string; id: string }>;
  /** User's reading status */
  readingStatus?: string;
  /** Number of books the user has read */
  booksRead: number;
  /** Number of books the user owns */
  booksOwned: number;
  /** User's rating (0-100 scale) */
  userRating?: number;
  /** User's notes */
  userNotes?: string;
  /** When the user started reading (ISO 8601) */
  startedAt?: string;
  /** When the user last read (ISO 8601) */
  lastReadAt?: string;
  /** When the user completed reading (ISO 8601) */
  completedAt?: string;
}

// =============================================================================
// Recommendation Request/Response
// =============================================================================

/** Parameters for `recommendations/get` method */
export interface RecommendationRequest {
  /** User's library entries */
  library: UserLibraryEntry[];
  /** Max recommendations to return */
  limit?: number;
  /** External IDs to exclude */
  excludeIds: string[];
}

/** A single recommendation */
export interface Recommendation {
  /** External ID on the source service */
  externalId: string;
  /** URL to the entry on the external service */
  externalUrl?: string;
  /** Title of the recommended series/book */
  title: string;
  /** Cover image URL */
  coverUrl?: string;
  /** Summary/description */
  summary?: string;
  /** Genres */
  genres: string[];
  /** Confidence/relevance score (0.0 to 1.0) */
  score: number;
  /** Human-readable reason for this recommendation */
  reason: string;
  /** Titles that influenced this recommendation */
  basedOn: string[];
  /** Codex series ID if matched */
  codexSeriesId?: string;
  /** Whether this series is already in the user's library */
  inLibrary: boolean;
}

/** Response from `recommendations/get` method */
export interface RecommendationResponse {
  /** Personalized recommendations */
  recommendations: Recommendation[];
  /** When generated (ISO 8601) */
  generatedAt?: string;
  /** Whether cached results */
  cached: boolean;
}

// =============================================================================
// Profile Update
// =============================================================================

/** Parameters for `recommendations/updateProfile` method */
export interface ProfileUpdateRequest {
  /** Updated library entries */
  entries: UserLibraryEntry[];
}

/** Response from `recommendations/updateProfile` method */
export interface ProfileUpdateResponse {
  /** Whether the profile was updated */
  updated: boolean;
  /** Number of entries processed */
  entriesProcessed: number;
}

// =============================================================================
// Clear
// =============================================================================

/** Response from `recommendations/clear` method */
export interface RecommendationClearResponse {
  /** Whether the clear succeeded */
  cleared: boolean;
}

// =============================================================================
// Dismiss
// =============================================================================

/** Dismiss reason */
export type DismissReason = "not_interested" | "already_read" | "already_owned";

/** Parameters for `recommendations/dismiss` method */
export interface RecommendationDismissRequest {
  /** External ID of the recommendation to dismiss */
  externalId: string;
  /** Reason for dismissal */
  reason?: DismissReason;
}

/** Response from `recommendations/dismiss` method */
export interface RecommendationDismissResponse {
  /** Whether the dismissal was recorded */
  dismissed: boolean;
}

// =============================================================================
// Provider Interface
// =============================================================================

/** Interface for recommendation provider plugins */
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
