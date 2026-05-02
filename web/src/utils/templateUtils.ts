import type {
  BookContext,
  ExternalIdContext,
  FullBook,
  FullSeries,
  FullSeriesMetadata,
  MetadataContext,
  SeriesContext,
} from "@/types";
import type { components } from "@/types/api.generated";

// =============================================================================
// Re-export types from @/types for convenience
// =============================================================================

// Re-export the generated types for backwards compatibility with existing imports
export type { BookContext, ExternalIdContext, MetadataContext, SeriesContext };

type BookExternalIdDto = components["schemas"]["BookExternalIdDto"];
type BookExternalLinkDto = components["schemas"]["BookExternalLinkDto"];

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

/**
 * Extended BookContext type that allows any JSON value in customMetadata.
 * The generated type is too restrictive (Record<string, never>), so we override it.
 */
export type BookContextWithCustomMetadata = Omit<
  BookContext,
  "customMetadata" | "series"
> & {
  customMetadata?: Record<string, unknown> | null;
  series: SeriesContextWithCustomMetadata;
};

/**
 * Union type for template contexts — either a series or book context.
 * Both share the same `type` discriminator field ("series" or "book").
 */
export type TemplateContext =
  | SeriesContextWithCustomMetadata
  | BookContextWithCustomMetadata;

export const SAMPLE_SERIES_CONTEXT: SeriesContextWithCustomMetadata = {
  type: "series",
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
    totalVolumeCount: 110,
    totalChapterCount: 1086.5,
    genres: ["Action", "Adventure", "Comedy", "Fantasy"],
    tags: ["pirates", "treasure", "friendship", "manga"],
    alternateTitles: [
      { label: "Japanese", title: "ワンピース" },
      { label: "Romaji", title: "Wan Pīsu" },
    ],
    authors: [
      { name: "Oda Eiichiro", role: "author", sortName: "Oda, Eiichiro" },
    ],
    externalRatings: [
      { source: "MyAnimeList", rating: 90.2, votes: 1500000 },
      { source: "AniList", rating: 88.0, votes: 950000 },
    ],
    externalLinks: [
      {
        source: "MyAnimeList",
        url: "https://myanimelist.net/manga/13",
        externalId: "13",
      },
      {
        source: "MangaDex",
        url: "https://mangadex.org/title/a1c7c817",
      },
    ],
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
    totalVolumeCountLock: false,
    totalChapterCountLock: false,
    genresLock: false,
    tagsLock: false,
    customMetadataLock: false,
    coverLock: false,
    authorsJsonLock: false,
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
    type: "series",
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
      totalVolumeCount: metadata.totalVolumeCount ?? null,
      totalChapterCount: metadata.totalChapterCount ?? null,
      genres: series.genres.map((g) => g.name),
      tags: series.tags.map((t) => t.name),
      alternateTitles: series.alternateTitles.map((at) => ({
        label: at.label,
        title: at.title,
      })),
      authors: (series.metadata.authors ?? []).map((a) => ({
        name: a.name,
        ...(a.role && { role: a.role }),
        ...(a.sortName && { sortName: a.sortName }),
      })),
      externalRatings: series.externalRatings.map((r) => ({
        source: r.sourceName,
        rating: r.rating,
        ...(r.voteCount != null && { votes: r.voteCount }),
      })),
      externalLinks: series.externalLinks.map((l) => ({
        source: l.sourceName,
        url: l.url,
        ...(l.externalId && { externalId: l.externalId }),
      })),
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
      totalVolumeCountLock: metadata.locks.totalVolumeCount ?? false,
      totalChapterCountLock: metadata.locks.totalChapterCount ?? false,
      genresLock: metadata.locks.genres ?? false,
      tagsLock: metadata.locks.tags ?? false,
      customMetadataLock: metadata.locks.customMetadata ?? false,
      coverLock: metadata.locks.cover ?? false,
      authorsJsonLock: metadata.locks.authorsJsonLock ?? false,
    },
    externalIds,
    customMetadata: metadata.customMetadata as Record<string, unknown> | null,
  };
}

// =============================================================================
// Sample Book Context
// =============================================================================

/**
 * Sample book context matching the backend `BookContext` structure exactly.
 *
 * Use this sample for:
 * - Template editor previews (book mode)
 * - Condition editor test data
 * - Frontend validation
 */
export const SAMPLE_BOOK_CONTEXT: BookContextWithCustomMetadata = {
  type: "book",
  bookId: "660e8400-e29b-41d4-a716-446655440001",
  seriesId: "550e8400-e29b-41d4-a716-446655440000",
  libraryId: "440e8400-e29b-41d4-a716-446655440099",
  fileFormat: "epub",
  pageCount: 369,
  fileSize: 2097152,
  metadata: {
    title: "The Martian",
    titleSort: "Martian, The",
    number: 1,
    subtitle: "A Novel",
    summary:
      "Astronaut Mark Watney is stranded alone on Mars after a dust storm forces his crew to evacuate.",
    publisher: "Crown Publishing",
    imprint: "Broadway Books",
    genre: "Science Fiction",
    languageIso: "en",
    formatDetail: "Trade Paperback",
    blackAndWhite: false,
    manga: false,
    year: 2014,
    month: 2,
    day: 11,
    volume: 1,
    count: 1,
    isbns: "978-0553418026",
    bookType: "novel",
    authors: [{ name: "Andy Weir", role: "author", sortName: "Weir, Andy" }],
    translator: null,
    edition: "First Edition",
    originalTitle: null,
    originalYear: 2011,
    seriesPosition: 1,
    seriesTotal: 1,
    subjects: ["Science Fiction", "Space Exploration", "Survival"],
    awards: [
      {
        name: "Hugo Award",
        year: 2015,
        category: "Best Novel",
        won: false,
      },
      {
        name: "Goodreads Choice Award",
        year: 2014,
        category: "Science Fiction",
        won: true,
      },
    ],
    genres: ["Science Fiction", "Adventure"],
    tags: ["mars", "survival", "space", "nasa"],
    externalLinks: [
      {
        source: "Goodreads",
        url: "https://www.goodreads.com/book/show/18007564",
        externalId: "18007564",
      },
      {
        source: "OpenLibrary",
        url: "https://openlibrary.org/works/OL17091818W",
      },
    ],
    titleLock: false,
    titleSortLock: false,
    numberLock: false,
    summaryLock: false,
    publisherLock: false,
    imprintLock: false,
    genreLock: false,
    languageIsoLock: false,
    formatDetailLock: false,
    blackAndWhiteLock: false,
    mangaLock: false,
    yearLock: false,
    monthLock: false,
    dayLock: false,
    volumeLock: false,
    countLock: false,
    isbnsLock: false,
    bookTypeLock: false,
    subtitleLock: false,
    authorsJsonLock: false,
    translatorLock: false,
    editionLock: false,
    originalTitleLock: false,
    originalYearLock: false,
    seriesPositionLock: false,
    seriesTotalLock: false,
    subjectsLock: false,
    awardsJsonLock: false,
    customMetadataLock: false,
    coverLock: false,
  },
  externalIds: {
    "plugin:goodreads": {
      id: "18007564",
      url: "https://www.goodreads.com/book/show/18007564",
      hash: null,
    },
  },
  customMetadata: {
    readingLevel: "Adult",
    pageEstimate: 369,
  },
  series: SAMPLE_SERIES_CONTEXT,
};

// =============================================================================
// Book Transform Functions
// =============================================================================

/**
 * Transforms a FullBook response into a BookContext object for template evaluation.
 *
 * Since book external IDs and external links are fetched separately from the
 * book detail, they must be passed as separate arguments.
 *
 * @param book - The full book response from the API
 * @param seriesContext - The parent series context (pre-built)
 * @param bookExternalIds - External IDs for this book (from separate API call)
 * @param bookExternalLinks - External links for this book (from separate API call)
 * @returns A BookContext object for template rendering
 */
export function transformFullBookToBookContext(
  book: FullBook,
  seriesContext: SeriesContextWithCustomMetadata,
  bookExternalIds: BookExternalIdDto[] = [],
  bookExternalLinks: BookExternalLinkDto[] = [],
): BookContextWithCustomMetadata {
  const metadata = book.metadata;

  // Build external IDs map from array
  const externalIds: Record<string, ExternalIdContext> = {};
  for (const eid of bookExternalIds) {
    externalIds[eid.source] = {
      id: eid.externalId,
      url: eid.externalUrl ?? null,
      hash: eid.metadataHash ?? null,
    };
  }

  return {
    type: "book",
    bookId: book.id,
    seriesId: book.seriesId,
    libraryId: book.libraryId,
    fileFormat: book.fileFormat,
    pageCount: book.pageCount,
    fileSize: book.fileSize,
    metadata: {
      title: metadata.title ?? null,
      titleSort: metadata.titleSort ?? null,
      number: metadata.number != null ? Number(metadata.number) : null,
      subtitle: metadata.subtitle ?? null,
      summary: metadata.summary ?? null,
      publisher: metadata.publisher ?? null,
      imprint: metadata.imprint ?? null,
      genre: metadata.genre ?? null,
      languageIso: metadata.languageIso ?? null,
      formatDetail: metadata.formatDetail ?? null,
      blackAndWhite: metadata.blackAndWhite ?? null,
      manga: metadata.manga ?? null,
      year: metadata.year ?? null,
      month: metadata.month ?? null,
      day: metadata.day ?? null,
      volume: metadata.volume ?? null,
      count: metadata.count ?? null,
      isbns: metadata.isbns ?? null,
      bookType: metadata.bookType ?? null,
      authors: (metadata.authors ?? []).map((a) => ({
        name: a.name,
        ...(a.role && { role: a.role }),
        ...(a.sortName && { sortName: a.sortName }),
      })),
      translator: metadata.translator ?? null,
      edition: metadata.edition ?? null,
      originalTitle: metadata.originalTitle ?? null,
      originalYear: metadata.originalYear ?? null,
      seriesPosition: metadata.seriesPosition ?? null,
      seriesTotal: metadata.seriesTotal ?? null,
      subjects: metadata.subjects ?? [],
      awards: (metadata.awards ?? []).map((a) => ({
        name: a.name,
        year: a.year ?? null,
        category: a.category ?? null,
        won: a.won,
      })),
      genres: book.genres.map((g) => g.name),
      tags: book.tags.map((t) => t.name),
      externalLinks: bookExternalLinks.map((l) => ({
        source: l.sourceName,
        url: l.url,
        ...(l.externalId && { externalId: l.externalId }),
      })),
      titleLock: metadata.locks.titleLock ?? false,
      titleSortLock: metadata.locks.titleSortLock ?? false,
      numberLock: metadata.locks.numberLock ?? false,
      summaryLock: metadata.locks.summaryLock ?? false,
      publisherLock: metadata.locks.publisherLock ?? false,
      imprintLock: metadata.locks.imprintLock ?? false,
      genreLock: metadata.locks.genreLock ?? false,
      languageIsoLock: metadata.locks.languageIsoLock ?? false,
      formatDetailLock: metadata.locks.formatDetailLock ?? false,
      blackAndWhiteLock: metadata.locks.blackAndWhiteLock ?? false,
      mangaLock: metadata.locks.mangaLock ?? false,
      yearLock: metadata.locks.yearLock ?? false,
      monthLock: metadata.locks.monthLock ?? false,
      dayLock: metadata.locks.dayLock ?? false,
      volumeLock: metadata.locks.volumeLock ?? false,
      countLock: metadata.locks.countLock ?? false,
      isbnsLock: metadata.locks.isbnsLock ?? false,
      bookTypeLock: metadata.locks.bookTypeLock ?? false,
      subtitleLock: metadata.locks.subtitleLock ?? false,
      authorsJsonLock: metadata.locks.authorsJsonLock ?? false,
      translatorLock: metadata.locks.translatorLock ?? false,
      editionLock: metadata.locks.editionLock ?? false,
      originalTitleLock: metadata.locks.originalTitleLock ?? false,
      originalYearLock: metadata.locks.originalYearLock ?? false,
      seriesPositionLock: metadata.locks.seriesPositionLock ?? false,
      seriesTotalLock: metadata.locks.seriesTotalLock ?? false,
      subjectsLock: metadata.locks.subjectsLock ?? false,
      awardsJsonLock: metadata.locks.awardsJsonLock ?? false,
      customMetadataLock: metadata.locks.customMetadataLock ?? false,
      coverLock: metadata.locks.coverLock ?? false,
    },
    externalIds,
    customMetadata: metadata.customMetadata as Record<string, unknown> | null,
    series: seriesContext,
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
  /** Expected total volume count, when known */
  totalVolumeCount: number | null;
  /** Expected total chapter count, when known. May be fractional. */
  totalChapterCount: number | null;
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
  totalVolumeCount: 34,
  totalChapterCount: 139,
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
    totalVolumeCount: metadata.totalVolumeCount ?? null,
    totalChapterCount: metadata.totalChapterCount ?? null,
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
    totalVolumeCount: metadata.totalVolumeCount ?? null,
    totalChapterCount: metadata.totalChapterCount ?? null,
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
