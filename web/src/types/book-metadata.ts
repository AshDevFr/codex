/**
 * Extended book metadata types for Phase 5 frontend improvements.
 *
 * These types represent the expanded book metadata fields added in Phase 1.
 * Once the backend DTOs are updated (Phase 6), these can be replaced with
 * the generated types from the OpenAPI spec.
 */

/**
 * Book type classification for content categorization.
 * Matches the BookType enum in src/db/entities/book_metadata.rs
 */
export type BookType =
  | "comic"
  | "manga"
  | "novel"
  | "novella"
  | "anthology"
  | "artbook"
  | "oneshot"
  | "omnibus"
  | "graphic_novel"
  | "magazine";

/**
 * Display names for book types
 */
export const BOOK_TYPE_DISPLAY: Record<BookType, string> = {
  comic: "Comic",
  manga: "Manga",
  novel: "Novel",
  novella: "Novella",
  anthology: "Anthology",
  artbook: "Art Book",
  oneshot: "Oneshot",
  omnibus: "Omnibus",
  graphic_novel: "Graphic Novel",
  magazine: "Magazine",
};

/**
 * Colors for book type badges
 */
export const BOOK_TYPE_COLORS: Record<BookType, string> = {
  comic: "blue",
  manga: "pink",
  novel: "green",
  novella: "teal",
  anthology: "grape",
  artbook: "violet",
  oneshot: "orange",
  omnibus: "cyan",
  graphic_novel: "indigo",
  magazine: "yellow",
};

/**
 * Author role in a book
 */
export type BookAuthorRole =
  | "author"
  | "co-author"
  | "editor"
  | "translator"
  | "illustrator"
  | "contributor";

/**
 * Structured author information
 */
export interface BookAuthor {
  name: string;
  role: BookAuthorRole;
  sortName?: string;
}

/**
 * Award information
 */
export interface BookAward {
  name: string;
  year?: number;
  category?: string;
  /** true = won, false = nominated */
  won: boolean;
}

/**
 * Book external ID for tracking metadata sources
 */
export interface BookExternalId {
  id: string;
  bookId: string;
  source: string;
  externalId: string;
  externalUrl?: string | null;
  metadataHash?: string | null;
  lastSyncedAt?: string | null;
  createdAt: string;
  updatedAt: string;
}

/**
 * Book cover information
 */
export interface BookCover {
  id: string;
  bookId: string;
  source: string;
  path: string;
  isSelected: boolean;
  width?: number | null;
  height?: number | null;
  createdAt: string;
  updatedAt: string;
}

/**
 * Extended book metadata with new Phase 1 fields.
 * This extends the existing BookMetadataDto with additional fields.
 */
export interface ExtendedBookMetadata {
  // Fields not included in BookMetadataDto but available in BookFullMetadata
  isbns?: string | null;

  // New Phase 1 fields
  bookType?: BookType | null;
  subtitle?: string | null;
  authorsJson?: string | null;
  translator?: string | null;
  edition?: string | null;
  originalTitle?: string | null;
  originalYear?: number | null;
  seriesPosition?: number | null;
  seriesTotal?: number | null;
  subjects?: string | null;
  awardsJson?: string | null;
  customMetadata?: string | null;

  // New lock fields
  bookTypeLock?: boolean;
  subtitleLock?: boolean;
  authorsJsonLock?: boolean;
  translatorLock?: boolean;
  editionLock?: boolean;
  originalTitleLock?: boolean;
  originalYearLock?: boolean;
  seriesPositionLock?: boolean;
  seriesTotalLock?: boolean;
  subjectsLock?: boolean;
  awardsJsonLock?: boolean;
  customMetadataLock?: boolean;
  coverLock?: boolean;
}

/**
 * Parse authors JSON string to array of BookAuthor
 */
export function parseAuthorsJson(
  json: string | null | undefined,
): BookAuthor[] {
  if (!json) return [];
  try {
    const parsed = JSON.parse(json);
    if (!Array.isArray(parsed)) return [];
    return parsed.map((item: unknown) => ({
      name: (item as BookAuthor)?.name ?? "Unknown",
      role: (item as BookAuthor)?.role ?? "author",
      sortName: (item as BookAuthor)?.sortName,
    }));
  } catch {
    return [];
  }
}

/**
 * Parse awards JSON string to array of BookAward
 */
export function parseAwardsJson(json: string | null | undefined): BookAward[] {
  if (!json) return [];
  try {
    const parsed = JSON.parse(json);
    if (!Array.isArray(parsed)) return [];
    return parsed.map((item: unknown) => ({
      name: (item as BookAward)?.name ?? "Unknown Award",
      year: (item as BookAward)?.year,
      category: (item as BookAward)?.category,
      won: (item as BookAward)?.won ?? false,
    }));
  } catch {
    return [];
  }
}

/**
 * Parse subjects to array of strings
 * Handles both JSON array and comma-separated string formats
 */
export function parseSubjects(value: string | null | undefined): string[] {
  if (!value) return [];

  // Try JSON array first
  try {
    const parsed = JSON.parse(value);
    if (Array.isArray(parsed)) {
      return parsed.filter((s): s is string => typeof s === "string");
    }
  } catch {
    // Not JSON, try comma-separated
  }

  // Fall back to comma-separated
  return value
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

/**
 * Display name for author role
 */
export const AUTHOR_ROLE_DISPLAY: Record<BookAuthorRole, string> = {
  author: "Author",
  "co-author": "Co-Author",
  editor: "Editor",
  translator: "Translator",
  illustrator: "Illustrator",
  contributor: "Contributor",
};

/**
 * Color for author role badges
 */
export const AUTHOR_ROLE_COLORS: Record<BookAuthorRole, string> = {
  author: "blue",
  "co-author": "cyan",
  editor: "grape",
  translator: "orange",
  illustrator: "pink",
  contributor: "gray",
};
