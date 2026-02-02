import type {
  ExternalIdContext,
  FullSeries,
  FullSeriesMetadata,
  MetadataContext,
  SeriesContext,
} from "@/types";

// =============================================================================
// Re-export types from @/types for convenience
// =============================================================================

// Re-export the generated types for backwards compatibility with existing imports
export type { ExternalIdContext, MetadataContext, SeriesContext };

// =============================================================================
// Sample Series Context
// =============================================================================

/**
 * Sample series context matching the backend `SeriesContext` structure exactly.
 *
 * This sample uses camelCase for all structured fields (matching backend serde serialization).
 * `customMetadata` contents are preserved as-is (user-defined, no case transformation).
 *
 * Use this sample for:
 * - Template editor previews
 * - Condition editor test data
 * - Frontend validation
 */
/**
 * Extended SeriesContext type that allows any JSON value in customMetadata.
 * The generated type is too restrictive (Record<string, never>), so we override it.
 */
export type SeriesContextWithCustomMetadata = Omit<
  SeriesContext,
  "customMetadata"
> & {
  customMetadata?: Record<string, unknown> | null;
};

export const SAMPLE_SERIES_CONTEXT: SeriesContextWithCustomMetadata = {
  seriesId: "550e8400-e29b-41d4-a716-446655440000",
  bookCount: 5,
  metadata: {
    title: "One Piece",
    titleSort: "One Piece",
    summary:
      "Follow Monkey D. Luffy and his crew on their epic journey to find the legendary One Piece treasure and become the Pirate King.",
    publisher: "Shueisha",
    imprint: "Jump Comics",
    status: "ongoing",
    ageRating: 13,
    language: "ja",
    readingDirection: "rtl",
    year: 1997,
    totalBookCount: 110,
    genres: ["Action", "Adventure", "Comedy", "Fantasy"],
    tags: ["pirates", "treasure", "friendship", "manga"],
    titleLock: false,
    titleSortLock: false,
    summaryLock: false,
    publisherLock: false,
    imprintLock: false,
    statusLock: false,
    ageRatingLock: false,
    languageLock: false,
    readingDirectionLock: false,
    yearLock: false,
    totalBookCountLock: false,
    genresLock: false,
    tagsLock: false,
    customMetadataLock: false,
  },
  externalIds: {
    "plugin:mangabaka": {
      id: "12345",
      url: "https://mangabaka.com/series/12345",
      hash: "abc123def456",
    },
    "plugin:anilist": {
      id: "21",
      url: "https://anilist.co/manga/21",
      hash: null,
    },
  },
  customMetadata: {
    myField: "preserved as-is",
    some_snake_field: 123,
    source: {
      name: "MySource",
      confidence: 0.95,
    },
  },
};

// =============================================================================
// Transform Functions
// =============================================================================

/**
 * Transforms a FullSeries response into a SeriesContext object.
 *
 * This is the primary transform function for template evaluation.
 * It converts the API response structure into the flat SeriesContext
 * structure expected by templates.
 *
 * @param series - The full series response from the API
 * @returns A SeriesContext object for template rendering
 */
export function transformFullSeriesToSeriesContext(
  series: FullSeries,
): SeriesContextWithCustomMetadata {
  const metadata = series.metadata;

  // Build external IDs map from array
  const externalIds: Record<string, ExternalIdContext> = {};
  for (const eid of series.externalIds) {
    externalIds[eid.source] = {
      id: eid.externalId,
      url: eid.externalUrl ?? null,
      hash: eid.metadataHash ?? null,
    };
  }

  return {
    seriesId: series.id,
    bookCount: series.bookCount,
    metadata: {
      title: metadata.title,
      titleSort: metadata.titleSort ?? null,
      summary: metadata.summary ?? null,
      publisher: metadata.publisher ?? null,
      imprint: metadata.imprint ?? null,
      status: metadata.status ?? null,
      ageRating: metadata.ageRating ?? null,
      language: metadata.language ?? null,
      readingDirection: metadata.readingDirection ?? null,
      year: metadata.year ?? null,
      totalBookCount: metadata.totalBookCount ?? null,
      genres: series.genres.map((g) => g.name),
      tags: series.tags.map((t) => t.name),
      titleLock: metadata.locks.title ?? false,
      titleSortLock: metadata.locks.titleSort ?? false,
      summaryLock: metadata.locks.summary ?? false,
      publisherLock: metadata.locks.publisher ?? false,
      imprintLock: metadata.locks.imprint ?? false,
      statusLock: metadata.locks.status ?? false,
      ageRatingLock: metadata.locks.ageRating ?? false,
      languageLock: metadata.locks.language ?? false,
      readingDirectionLock: metadata.locks.readingDirection ?? false,
      yearLock: metadata.locks.year ?? false,
      totalBookCountLock: metadata.locks.totalBookCount ?? false,
      genresLock: metadata.locks.genres ?? false,
      tagsLock: metadata.locks.tags ?? false,
      customMetadataLock: metadata.locks.customMetadata ?? false,
    },
    externalIds,
    customMetadata: metadata.customMetadata as Record<string, unknown> | null,
  };
}

// =============================================================================
// Legacy MetadataForTemplate (for backwards compatibility)
// =============================================================================

/**
 * Simplified metadata type for use in templates.
 *
 * This type provides a flattened, template-friendly view of series metadata.
 * Complex nested DTOs are simplified to make template authoring easier:
 * - genres/tags are arrays of strings (just names, not full objects)
 * - externalRatings/externalLinks are simplified objects
 * - alternateTitles are simplified objects
 *
 * @deprecated Use `SeriesContext` for new code. This interface is kept for
 * backwards compatibility with `CustomMetadataDisplay` and other existing components.
 */
export interface MetadataForTemplate {
  /** Series title */
  title: string;
  /** Series summary/description */
  summary: string | null;
  /** Publisher name */
  publisher: string | null;
  /** Imprint (sub-publisher) */
  imprint: string | null;
  /** Publication year */
  year: number | null;
  /** Age rating (e.g., 13, 16, 18) */
  ageRating: number | null;
  /** Language code (BCP47 format) */
  language: string | null;
  /** Series status (ongoing, ended, hiatus, abandoned, unknown) */
  status: string | null;
  /** Reading direction (ltr, rtl, ttb, webtoon) */
  readingDirection: string | null;
  /** Expected total book count */
  totalBookCount: number | null;
  /** Custom sort name */
  titleSort: string | null;
  /** Genre names as a simple array of strings */
  genres: string[];
  /** Tag names as a simple array of strings */
  tags: string[];
  /** External ratings from various sources */
  externalRatings: Array<{ source: string; rating: number; votes?: number }>;
  /** External links to other sites */
  externalLinks: Array<{ source: string; url: string; externalId?: string }>;
  /** Alternate titles for this series */
  alternateTitles: Array<{ title: string; label: string }>;
}

/**
 * Sample metadata for template testing in the editor.
 * This provides realistic mock data matching the MetadataForTemplate structure.
 */
export const SAMPLE_METADATA_FOR_TEMPLATE: MetadataForTemplate = {
  title: "Attack on Titan",
  summary:
    "Humanity lives inside cities surrounded by enormous walls due to the Titans, gigantic humanoid creatures who devour humans seemingly without reason.",
  publisher: "Kodansha",
  imprint: "Bessatsu Shōnen Magazine",
  year: 2009,
  ageRating: 16,
  language: "ja",
  status: "ended",
  readingDirection: "rtl",
  totalBookCount: 34,
  titleSort: "Attack on Titan",
  genres: ["Action", "Dark Fantasy", "Post-Apocalyptic"],
  tags: ["manga", "titans", "survival", "military"],
  externalRatings: [
    { source: "MyAnimeList", rating: 8.54, votes: 1250000 },
    { source: "AniList", rating: 84, votes: 890000 },
  ],
  externalLinks: [
    { source: "MyAnimeList", url: "https://myanimelist.net/manga/23390" },
    {
      source: "AniList",
      url: "https://anilist.co/manga/53390",
      externalId: "53390",
    },
  ],
  alternateTitles: [
    { title: "Shingeki no Kyojin", label: "Romaji" },
    { title: "進撃の巨人", label: "Native" },
  ],
};

/**
 * Transforms a FullSeriesMetadata response into a simplified MetadataForTemplate object.
 *
 * This transformation:
 * - Extracts scalar fields directly
 * - Simplifies genres and tags to arrays of names
 * - Simplifies external ratings, links, and alternate titles to clean objects
 * - Omits internal fields like IDs, timestamps, and locks
 *
 * @deprecated Use `transformFullSeriesToSeriesContext` instead.
 * @param metadata - The full series metadata from the API
 * @returns A simplified metadata object for template rendering
 */
export function transformToMetadataForTemplate(
  metadata: FullSeriesMetadata,
): MetadataForTemplate {
  return {
    // Scalar fields - pass through directly
    title: metadata.title,
    summary: metadata.summary ?? null,
    publisher: metadata.publisher ?? null,
    imprint: metadata.imprint ?? null,
    year: metadata.year ?? null,
    ageRating: metadata.ageRating ?? null,
    language: metadata.language ?? null,
    status: metadata.status ?? null,
    readingDirection: metadata.readingDirection ?? null,
    totalBookCount: metadata.totalBookCount ?? null,
    titleSort: metadata.titleSort ?? null,

    // Simplify genres to just names
    genres: metadata.genres.map((genre) => genre.name),

    // Simplify tags to just names
    tags: metadata.tags.map((tag) => tag.name),

    // Simplify external ratings
    externalRatings: metadata.externalRatings.map((rating) => ({
      source: rating.sourceName,
      rating: rating.rating,
      ...(rating.voteCount !== undefined &&
        rating.voteCount !== null && { votes: rating.voteCount }),
    })),

    // Simplify external links
    externalLinks: metadata.externalLinks.map((link) => ({
      source: link.sourceName,
      url: link.url,
      ...(link.externalId && { externalId: link.externalId }),
    })),

    // Simplify alternate titles
    alternateTitles: metadata.alternateTitles.map((alt) => ({
      title: alt.title,
      label: alt.label,
    })),
  };
}

/**
 * Transforms a FullSeries response (FullSeriesResponse) into a MetadataForTemplate object.
 *
 * This handles the nested structure of FullSeriesResponse where:
 * - Scalar metadata fields are in `series.metadata`
 * - Arrays (genres, tags, etc.) are at the top level of the response
 *
 * @deprecated Use `transformFullSeriesToSeriesContext` instead.
 * @param series - The full series response from the API
 * @returns A simplified metadata object for template rendering
 */
export function transformFullSeriesToMetadataForTemplate(
  series: FullSeries,
): MetadataForTemplate {
  const metadata = series.metadata;

  return {
    // Scalar fields from nested metadata
    title: metadata.title,
    summary: metadata.summary ?? null,
    publisher: metadata.publisher ?? null,
    imprint: metadata.imprint ?? null,
    year: metadata.year ?? null,
    ageRating: metadata.ageRating ?? null,
    language: metadata.language ?? null,
    status: metadata.status ?? null,
    readingDirection: metadata.readingDirection ?? null,
    totalBookCount: metadata.totalBookCount ?? null,
    titleSort: metadata.titleSort ?? null,

    // Arrays from top-level of FullSeriesResponse
    genres: series.genres.map((genre) => genre.name),
    tags: series.tags.map((tag) => tag.name),

    externalRatings: series.externalRatings.map((rating) => ({
      source: rating.sourceName,
      rating: rating.rating,
      ...(rating.voteCount !== undefined &&
        rating.voteCount !== null && { votes: rating.voteCount }),
    })),

    externalLinks: series.externalLinks.map((link) => ({
      source: link.sourceName,
      url: link.url,
      ...(link.externalId && { externalId: link.externalId }),
    })),

    alternateTitles: series.alternateTitles.map((alt) => ({
      title: alt.title,
      label: alt.label,
    })),
  };
}
