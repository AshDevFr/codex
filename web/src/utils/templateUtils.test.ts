import { describe, expect, it } from "vitest";
import type { FullBook, FullSeries, FullSeriesMetadata } from "@/types";
import type { components } from "@/types/api.generated";
import {
  SAMPLE_BOOK_CONTEXT,
  SAMPLE_SERIES_CONTEXT,
  type SeriesContext,
  transformFullBookToBookContext,
  transformFullSeriesToMetadataForTemplate,
  transformFullSeriesToSeriesContext,
  transformToMetadataForTemplate,
} from "./templateUtils";

/**
 * Creates a minimal valid FullSeriesMetadata object for testing
 */
function createMockMetadata(
  overrides: Partial<FullSeriesMetadata> = {},
): FullSeriesMetadata {
  return {
    seriesId: "test-uuid",
    title: "Test Series",
    summary: null,
    publisher: null,
    imprint: null,
    year: null,
    ageRating: null,
    language: null,
    status: null,
    readingDirection: null,
    totalBookCount: null,
    titleSort: null,
    genres: [],
    tags: [],
    externalRatings: [],
    externalLinks: [],
    alternateTitles: [],
    customMetadata: null,
    locks: {
      title: false,
      titleSort: false,
      summary: false,
      publisher: false,
      imprint: false,
      language: false,
      ageRating: false,
      year: false,
      status: false,
      totalBookCount: false,
      readingDirection: false,
      customMetadata: false,
      genres: false,
      tags: false,
      cover: false,
      authorsJsonLock: false,
      alternateTitles: false,
    },
    createdAt: "2024-01-01T00:00:00Z",
    updatedAt: "2024-01-01T00:00:00Z",
    ...overrides,
  };
}

describe("templateUtils", () => {
  describe("transformToMetadataForTemplate", () => {
    it("should transform basic scalar fields", () => {
      const metadata = createMockMetadata({
        title: "Attack on Titan",
        summary: "Humanity lives inside walls...",
        publisher: "Kodansha",
        imprint: "Bessatsu Shōnen Magazine",
        year: 2009,
        ageRating: 16,
        language: "ja",
        status: "ended",
        readingDirection: "rtl",
        totalBookCount: 34,
        titleSort: "Attack on Titan",
      });

      const result = transformToMetadataForTemplate(metadata);

      expect(result.title).toBe("Attack on Titan");
      expect(result.summary).toBe("Humanity lives inside walls...");
      expect(result.publisher).toBe("Kodansha");
      expect(result.imprint).toBe("Bessatsu Shōnen Magazine");
      expect(result.year).toBe(2009);
      expect(result.ageRating).toBe(16);
      expect(result.language).toBe("ja");
      expect(result.status).toBe("ended");
      expect(result.readingDirection).toBe("rtl");
      expect(result.totalBookCount).toBe(34);
      expect(result.titleSort).toBe("Attack on Titan");
    });

    it("should handle null/undefined optional fields", () => {
      const metadata = createMockMetadata({
        title: "Minimal Series",
        summary: undefined,
        publisher: undefined,
      });

      const result = transformToMetadataForTemplate(metadata);

      expect(result.title).toBe("Minimal Series");
      expect(result.summary).toBeNull();
      expect(result.publisher).toBeNull();
      expect(result.imprint).toBeNull();
      expect(result.year).toBeNull();
      expect(result.ageRating).toBeNull();
      expect(result.language).toBeNull();
      expect(result.status).toBeNull();
      expect(result.readingDirection).toBeNull();
      expect(result.totalBookCount).toBeNull();
      expect(result.titleSort).toBeNull();
    });

    it("should transform genres to array of names", () => {
      const metadata = createMockMetadata({
        genres: [
          { id: "genre-1", name: "Action", createdAt: "2024-01-01T00:00:00Z" },
          {
            id: "genre-2",
            name: "Dark Fantasy",
            createdAt: "2024-01-01T00:00:00Z",
          },
          {
            id: "genre-3",
            name: "Post-Apocalyptic",
            createdAt: "2024-01-01T00:00:00Z",
          },
        ],
      });

      const result = transformToMetadataForTemplate(metadata);

      expect(result.genres).toEqual([
        "Action",
        "Dark Fantasy",
        "Post-Apocalyptic",
      ]);
    });

    it("should transform tags to array of names", () => {
      const metadata = createMockMetadata({
        tags: [
          { id: "tag-1", name: "manga", createdAt: "2024-01-01T00:00:00Z" },
          { id: "tag-2", name: "titans", createdAt: "2024-01-01T00:00:00Z" },
          { id: "tag-3", name: "survival", createdAt: "2024-01-01T00:00:00Z" },
        ],
      });

      const result = transformToMetadataForTemplate(metadata);

      expect(result.tags).toEqual(["manga", "titans", "survival"]);
    });

    it("should handle empty genres and tags arrays", () => {
      const metadata = createMockMetadata({
        genres: [],
        tags: [],
      });

      const result = transformToMetadataForTemplate(metadata);

      expect(result.genres).toEqual([]);
      expect(result.tags).toEqual([]);
    });

    it("should transform external ratings with vote counts", () => {
      const metadata = createMockMetadata({
        externalRatings: [
          {
            id: "rating-1",
            seriesId: "test-uuid",
            sourceName: "MyAnimeList",
            rating: 8.54,
            voteCount: 1250000,
            createdAt: "2024-01-01T00:00:00Z",
            updatedAt: "2024-01-01T00:00:00Z",
            fetchedAt: "2024-01-15T10:30:00Z",
          },
          {
            id: "rating-2",
            seriesId: "test-uuid",
            sourceName: "AniList",
            rating: 84,
            voteCount: 890000,
            createdAt: "2024-01-01T00:00:00Z",
            updatedAt: "2024-01-01T00:00:00Z",
            fetchedAt: "2024-01-15T10:30:00Z",
          },
        ],
      });

      const result = transformToMetadataForTemplate(metadata);

      expect(result.externalRatings).toHaveLength(2);
      expect(result.externalRatings[0]).toEqual({
        source: "MyAnimeList",
        rating: 8.54,
        votes: 1250000,
      });
      expect(result.externalRatings[1]).toEqual({
        source: "AniList",
        rating: 84,
        votes: 890000,
      });
    });

    it("should transform external ratings without vote counts", () => {
      const metadata = createMockMetadata({
        externalRatings: [
          {
            id: "rating-1",
            seriesId: "test-uuid",
            sourceName: "UserRating",
            rating: 9.0,
            voteCount: null,
            createdAt: "2024-01-01T00:00:00Z",
            updatedAt: "2024-01-01T00:00:00Z",
            fetchedAt: "2024-01-15T10:30:00Z",
          },
        ],
      });

      const result = transformToMetadataForTemplate(metadata);

      expect(result.externalRatings[0]).toEqual({
        source: "UserRating",
        rating: 9.0,
      });
      // votes property should not be present when null
      expect("votes" in result.externalRatings[0]).toBe(false);
    });

    it("should transform external links with external IDs", () => {
      const metadata = createMockMetadata({
        externalLinks: [
          {
            id: "link-1",
            seriesId: "test-uuid",
            sourceName: "MyAnimeList",
            url: "https://myanimelist.net/manga/23390",
            externalId: "23390",
            createdAt: "2024-01-01T00:00:00Z",
            updatedAt: "2024-01-01T00:00:00Z",
          },
          {
            id: "link-2",
            seriesId: "test-uuid",
            sourceName: "AniList",
            url: "https://anilist.co/manga/53390",
            externalId: "53390",
            createdAt: "2024-01-01T00:00:00Z",
            updatedAt: "2024-01-01T00:00:00Z",
          },
        ],
      });

      const result = transformToMetadataForTemplate(metadata);

      expect(result.externalLinks).toHaveLength(2);
      expect(result.externalLinks[0]).toEqual({
        source: "MyAnimeList",
        url: "https://myanimelist.net/manga/23390",
        externalId: "23390",
      });
      expect(result.externalLinks[1]).toEqual({
        source: "AniList",
        url: "https://anilist.co/manga/53390",
        externalId: "53390",
      });
    });

    it("should transform external links without external IDs", () => {
      const metadata = createMockMetadata({
        externalLinks: [
          {
            id: "link-1",
            seriesId: "test-uuid",
            sourceName: "Custom",
            url: "https://example.com/series",
            externalId: null,
            createdAt: "2024-01-01T00:00:00Z",
            updatedAt: "2024-01-01T00:00:00Z",
          },
        ],
      });

      const result = transformToMetadataForTemplate(metadata);

      expect(result.externalLinks[0]).toEqual({
        source: "Custom",
        url: "https://example.com/series",
      });
      // externalId property should not be present when null/empty
      expect("externalId" in result.externalLinks[0]).toBe(false);
    });

    it("should transform alternate titles", () => {
      const metadata = createMockMetadata({
        alternateTitles: [
          {
            id: "alt-1",
            seriesId: "test-uuid",
            title: "Shingeki no Kyojin",
            label: "Japanese",
            createdAt: "2024-01-01T00:00:00Z",
            updatedAt: "2024-01-01T00:00:00Z",
          },
          {
            id: "alt-2",
            seriesId: "test-uuid",
            title: "進撃の巨人",
            label: "Native",
            createdAt: "2024-01-01T00:00:00Z",
            updatedAt: "2024-01-01T00:00:00Z",
          },
        ],
      });

      const result = transformToMetadataForTemplate(metadata);

      expect(result.alternateTitles).toHaveLength(2);
      expect(result.alternateTitles[0]).toEqual({
        title: "Shingeki no Kyojin",
        label: "Japanese",
      });
      expect(result.alternateTitles[1]).toEqual({
        title: "進撃の巨人",
        label: "Native",
      });
    });

    it("should handle empty arrays for all list fields", () => {
      const metadata = createMockMetadata({
        genres: [],
        tags: [],
        externalRatings: [],
        externalLinks: [],
        alternateTitles: [],
      });

      const result = transformToMetadataForTemplate(metadata);

      expect(result.genres).toEqual([]);
      expect(result.tags).toEqual([]);
      expect(result.externalRatings).toEqual([]);
      expect(result.externalLinks).toEqual([]);
      expect(result.alternateTitles).toEqual([]);
    });

    it("should not include internal fields like seriesId, locks, timestamps", () => {
      const metadata = createMockMetadata({
        title: "Test",
      });

      const result = transformToMetadataForTemplate(metadata);

      // Verify internal fields are not present
      expect("seriesId" in result).toBe(false);
      expect("locks" in result).toBe(false);
      expect("createdAt" in result).toBe(false);
      expect("updatedAt" in result).toBe(false);
      expect("customMetadata" in result).toBe(false);
    });

    it("should produce template-compatible output structure", () => {
      const metadata = createMockMetadata({
        title: "Batman: Year One",
        summary: "The definitive origin story of Batman.",
        publisher: "DC Comics",
        year: 1987,
        genres: [
          { id: "g1", name: "Action", createdAt: "2024-01-01T00:00:00Z" },
          { id: "g2", name: "Crime", createdAt: "2024-01-01T00:00:00Z" },
        ],
        tags: [
          { id: "t1", name: "batman", createdAt: "2024-01-01T00:00:00Z" },
          { id: "t2", name: "origin", createdAt: "2024-01-01T00:00:00Z" },
        ],
      });

      const result = transformToMetadataForTemplate(metadata);

      // Verify structure matches what templates expect
      expect(typeof result.title).toBe("string");
      expect(
        result.summary === null || typeof result.summary === "string",
      ).toBe(true);
      expect(Array.isArray(result.genres)).toBe(true);
      expect(Array.isArray(result.tags)).toBe(true);
      expect(result.genres.every((g) => typeof g === "string")).toBe(true);
      expect(result.tags.every((t) => typeof t === "string")).toBe(true);
    });
  });

  describe("transformFullSeriesToMetadataForTemplate", () => {
    /**
     * Creates a minimal valid FullSeries object for testing.
     * FullSeries has a nested structure where scalar metadata is in `metadata`
     * and arrays (genres, tags, etc.) are at the top level.
     */
    function createMockFullSeries(
      overrides: Partial<FullSeries> = {},
      metadataOverrides: Partial<FullSeries["metadata"]> = {},
    ): FullSeries {
      return {
        id: "series-uuid",
        libraryId: "library-uuid",
        libraryName: "Test Library",
        bookCount: 10,
        unreadCount: 5,
        hasCustomCover: false,
        selectedCoverSource: "first_book",
        path: "/media/series/test",
        genres: [],
        tags: [],
        externalIds: [],
        externalRatings: [],
        externalLinks: [],
        alternateTitles: [],
        metadata: {
          title: "Test Series",
          summary: null,
          publisher: null,
          imprint: null,
          year: null,
          ageRating: null,
          language: null,
          status: null,
          readingDirection: null,
          totalBookCount: null,
          titleSort: null,
          customMetadata: null,
          locks: {
            title: false,
            titleSort: false,
            summary: false,
            publisher: false,
            imprint: false,
            language: false,
            ageRating: false,
            year: false,
            status: false,
            totalBookCount: false,
            readingDirection: false,
            customMetadata: false,
            genres: false,
            tags: false,
            cover: false,
            authorsJsonLock: false,
            alternateTitles: false,
          },
          createdAt: "2024-01-01T00:00:00Z",
          updatedAt: "2024-01-01T00:00:00Z",
          ...metadataOverrides,
        },
        createdAt: "2024-01-01T00:00:00Z",
        updatedAt: "2024-01-01T00:00:00Z",
        ...overrides,
      };
    }

    it("should transform scalar fields from nested metadata", () => {
      const series = createMockFullSeries(
        {},
        {
          title: "Attack on Titan",
          summary: "Humanity lives inside walls...",
          publisher: "Kodansha",
          imprint: "Bessatsu Shōnen Magazine",
          year: 2009,
          ageRating: 16,
          language: "ja",
          status: "ended",
          readingDirection: "rtl",
          totalBookCount: 34,
          titleSort: "Attack on Titan",
        },
      );

      const result = transformFullSeriesToMetadataForTemplate(series);

      expect(result.title).toBe("Attack on Titan");
      expect(result.summary).toBe("Humanity lives inside walls...");
      expect(result.publisher).toBe("Kodansha");
      expect(result.imprint).toBe("Bessatsu Shōnen Magazine");
      expect(result.year).toBe(2009);
      expect(result.ageRating).toBe(16);
      expect(result.language).toBe("ja");
      expect(result.status).toBe("ended");
      expect(result.readingDirection).toBe("rtl");
      expect(result.totalBookCount).toBe(34);
      expect(result.titleSort).toBe("Attack on Titan");
    });

    it("should handle null/undefined optional metadata fields", () => {
      const series = createMockFullSeries(
        {},
        {
          title: "Minimal Series",
          summary: undefined,
          publisher: undefined,
        },
      );

      const result = transformFullSeriesToMetadataForTemplate(series);

      expect(result.title).toBe("Minimal Series");
      expect(result.summary).toBeNull();
      expect(result.publisher).toBeNull();
      expect(result.imprint).toBeNull();
      expect(result.year).toBeNull();
    });

    it("should transform genres from top-level to array of names", () => {
      const series = createMockFullSeries({
        genres: [
          { id: "genre-1", name: "Action", createdAt: "2024-01-01T00:00:00Z" },
          {
            id: "genre-2",
            name: "Dark Fantasy",
            createdAt: "2024-01-01T00:00:00Z",
          },
        ],
      });

      const result = transformFullSeriesToMetadataForTemplate(series);

      expect(result.genres).toEqual(["Action", "Dark Fantasy"]);
    });

    it("should transform tags from top-level to array of names", () => {
      const series = createMockFullSeries({
        tags: [
          { id: "tag-1", name: "manga", createdAt: "2024-01-01T00:00:00Z" },
          { id: "tag-2", name: "titans", createdAt: "2024-01-01T00:00:00Z" },
        ],
      });

      const result = transformFullSeriesToMetadataForTemplate(series);

      expect(result.tags).toEqual(["manga", "titans"]);
    });

    it("should transform external ratings from top-level", () => {
      const series = createMockFullSeries({
        externalRatings: [
          {
            id: "rating-1",
            seriesId: "series-uuid",
            sourceName: "MyAnimeList",
            rating: 8.54,
            voteCount: 1250000,
            createdAt: "2024-01-01T00:00:00Z",
            updatedAt: "2024-01-01T00:00:00Z",
            fetchedAt: "2024-01-15T10:30:00Z",
          },
        ],
      });

      const result = transformFullSeriesToMetadataForTemplate(series);

      expect(result.externalRatings).toHaveLength(1);
      expect(result.externalRatings[0]).toEqual({
        source: "MyAnimeList",
        rating: 8.54,
        votes: 1250000,
      });
    });

    it("should transform external links from top-level", () => {
      const series = createMockFullSeries({
        externalLinks: [
          {
            id: "link-1",
            seriesId: "series-uuid",
            sourceName: "MyAnimeList",
            url: "https://myanimelist.net/manga/23390",
            externalId: "23390",
            createdAt: "2024-01-01T00:00:00Z",
            updatedAt: "2024-01-01T00:00:00Z",
          },
        ],
      });

      const result = transformFullSeriesToMetadataForTemplate(series);

      expect(result.externalLinks).toHaveLength(1);
      expect(result.externalLinks[0]).toEqual({
        source: "MyAnimeList",
        url: "https://myanimelist.net/manga/23390",
        externalId: "23390",
      });
    });

    it("should transform alternate titles from top-level", () => {
      const series = createMockFullSeries({
        alternateTitles: [
          {
            id: "alt-1",
            seriesId: "series-uuid",
            title: "Shingeki no Kyojin",
            label: "Japanese",
            createdAt: "2024-01-01T00:00:00Z",
            updatedAt: "2024-01-01T00:00:00Z",
          },
        ],
      });

      const result = transformFullSeriesToMetadataForTemplate(series);

      expect(result.alternateTitles).toHaveLength(1);
      expect(result.alternateTitles[0]).toEqual({
        title: "Shingeki no Kyojin",
        label: "Japanese",
      });
    });

    it("should not include series-specific fields like id, libraryId, bookCount", () => {
      const series = createMockFullSeries({
        id: "series-123",
        libraryId: "library-456",
        bookCount: 25,
      });

      const result = transformFullSeriesToMetadataForTemplate(series);

      expect("id" in result).toBe(false);
      expect("libraryId" in result).toBe(false);
      expect("bookCount" in result).toBe(false);
      expect("unreadCount" in result).toBe(false);
      expect("path" in result).toBe(false);
    });

    it("should handle empty arrays correctly", () => {
      const series = createMockFullSeries({
        genres: [],
        tags: [],
        externalRatings: [],
        externalLinks: [],
        alternateTitles: [],
      });

      const result = transformFullSeriesToMetadataForTemplate(series);

      expect(result.genres).toEqual([]);
      expect(result.tags).toEqual([]);
      expect(result.externalRatings).toEqual([]);
      expect(result.externalLinks).toEqual([]);
      expect(result.alternateTitles).toEqual([]);
    });

    it("should produce output compatible with CustomMetadataDisplay", () => {
      const series = createMockFullSeries(
        {
          genres: [
            { id: "g1", name: "Action", createdAt: "2024-01-01T00:00:00Z" },
          ],
          tags: [
            { id: "t1", name: "batman", createdAt: "2024-01-01T00:00:00Z" },
          ],
        },
        {
          title: "Batman: Year One",
          summary: "The definitive origin story of Batman.",
          publisher: "DC Comics",
          year: 1987,
        },
      );

      const result = transformFullSeriesToMetadataForTemplate(series);

      // Verify structure matches MetadataForTemplate interface
      expect(typeof result.title).toBe("string");
      expect(
        result.summary === null || typeof result.summary === "string",
      ).toBe(true);
      expect(Array.isArray(result.genres)).toBe(true);
      expect(Array.isArray(result.tags)).toBe(true);
      expect(result.genres.every((g) => typeof g === "string")).toBe(true);
      expect(result.tags.every((t) => typeof t === "string")).toBe(true);
    });
  });

  describe("SAMPLE_SERIES_CONTEXT", () => {
    it("should have correct top-level structure matching backend SeriesContext", () => {
      // Verify all required top-level fields exist
      expect(SAMPLE_SERIES_CONTEXT).toHaveProperty("type");
      expect(SAMPLE_SERIES_CONTEXT).toHaveProperty("seriesId");
      expect(SAMPLE_SERIES_CONTEXT).toHaveProperty("bookCount");
      expect(SAMPLE_SERIES_CONTEXT).toHaveProperty("metadata");
      expect(SAMPLE_SERIES_CONTEXT).toHaveProperty("externalIds");
      expect(SAMPLE_SERIES_CONTEXT).toHaveProperty("customMetadata");
    });

    it("should have type discriminator set to 'series'", () => {
      expect(SAMPLE_SERIES_CONTEXT.type).toBe("series");
    });

    it("should use camelCase for all structured field names", () => {
      // Top-level fields should be camelCase
      expect("seriesId" in SAMPLE_SERIES_CONTEXT).toBe(true);
      expect("bookCount" in SAMPLE_SERIES_CONTEXT).toBe(true);
      expect("externalIds" in SAMPLE_SERIES_CONTEXT).toBe(true);
      expect("customMetadata" in SAMPLE_SERIES_CONTEXT).toBe(true);

      // Should NOT have snake_case versions
      expect("series_id" in SAMPLE_SERIES_CONTEXT).toBe(false);
      expect("book_count" in SAMPLE_SERIES_CONTEXT).toBe(false);
      expect("external_ids" in SAMPLE_SERIES_CONTEXT).toBe(false);
      expect("custom_metadata" in SAMPLE_SERIES_CONTEXT).toBe(false);
    });

    it("should have metadata with camelCase field names", () => {
      const { metadata } = SAMPLE_SERIES_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      // Core metadata fields should be camelCase
      expect("titleSort" in metadata).toBe(true);
      expect("ageRating" in metadata).toBe(true);
      expect("readingDirection" in metadata).toBe(true);
      expect("totalBookCount" in metadata).toBe(true);

      // Should NOT have snake_case versions
      expect("title_sort" in metadata).toBe(false);
      expect("age_rating" in metadata).toBe(false);
      expect("reading_direction" in metadata).toBe(false);
      expect("total_book_count" in metadata).toBe(false);
    });

    it("should have metadata lock fields with camelCase names", () => {
      const { metadata } = SAMPLE_SERIES_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      // Lock fields should be camelCase
      expect("titleLock" in metadata).toBe(true);
      expect("titleSortLock" in metadata).toBe(true);
      expect("summaryLock" in metadata).toBe(true);
      expect("ageRatingLock" in metadata).toBe(true);
      expect("readingDirectionLock" in metadata).toBe(true);
      expect("totalBookCountLock" in metadata).toBe(true);
      expect("genresLock" in metadata).toBe(true);
      expect("tagsLock" in metadata).toBe(true);
      expect("customMetadataLock" in metadata).toBe(true);

      // Should NOT have snake_case versions
      expect("title_lock" in metadata).toBe(false);
      expect("title_sort_lock" in metadata).toBe(false);
    });

    it("should have genres and tags as string arrays in metadata", () => {
      const { metadata } = SAMPLE_SERIES_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      expect(Array.isArray(metadata.genres)).toBe(true);
      expect(Array.isArray(metadata.tags)).toBe(true);
      expect(metadata.genres?.length).toBeGreaterThan(0);
      expect(metadata.tags?.length).toBeGreaterThan(0);
      expect(metadata.genres?.every((g) => typeof g === "string")).toBe(true);
      expect(metadata.tags?.every((t) => typeof t === "string")).toBe(true);
    });

    it("should have externalIds with proper structure", () => {
      const { externalIds } = SAMPLE_SERIES_CONTEXT;
      expect(externalIds).toBeDefined();
      if (!externalIds) return;

      // Should have at least one external ID
      expect(Object.keys(externalIds).length).toBeGreaterThan(0);

      // Each external ID should have id, url, and hash fields
      for (const [source, context] of Object.entries(externalIds)) {
        expect(typeof source).toBe("string");
        expect(context).toHaveProperty("id");
        expect(typeof context.id).toBe("string");
      }
    });

    it("should have customMetadata preserved as-is (no case transformation)", () => {
      const { customMetadata } = SAMPLE_SERIES_CONTEXT;

      expect(customMetadata).not.toBeNull();
      expect(customMetadata).not.toBeUndefined();

      // customMetadata should preserve user-defined field names exactly
      // This includes both camelCase and snake_case as defined by the user
      expect(customMetadata).toHaveProperty("myField");
      expect(customMetadata).toHaveProperty("some_snake_field");

      // Nested objects should also be preserved
      expect(customMetadata).toHaveProperty("source");
      expect((customMetadata as Record<string, unknown>).source).toHaveProperty(
        "name",
      );
    });

    it("should be serializable to JSON matching backend format", () => {
      // Serialize to JSON and parse back
      const json = JSON.stringify(SAMPLE_SERIES_CONTEXT);
      const parsed = JSON.parse(json) as SeriesContext;

      // Verify structure survives serialization
      expect(parsed.seriesId).toBe(SAMPLE_SERIES_CONTEXT.seriesId);
      expect(parsed.bookCount).toBe(SAMPLE_SERIES_CONTEXT.bookCount);
      expect(parsed.metadata).toBeDefined();
      expect(SAMPLE_SERIES_CONTEXT.metadata).toBeDefined();
      if (!parsed.metadata || !SAMPLE_SERIES_CONTEXT.metadata) return;
      expect(parsed.metadata.title).toBe(SAMPLE_SERIES_CONTEXT.metadata.title);
      expect(parsed.metadata.genres).toEqual(
        SAMPLE_SERIES_CONTEXT.metadata.genres,
      );
      expect(parsed.metadata.tags).toEqual(SAMPLE_SERIES_CONTEXT.metadata.tags);
    });

    it("should have all boolean lock fields set to false by default", () => {
      const { metadata } = SAMPLE_SERIES_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      // All lock fields should be false in the sample
      expect(metadata.titleLock).toBe(false);
      expect(metadata.titleSortLock).toBe(false);
      expect(metadata.summaryLock).toBe(false);
      expect(metadata.publisherLock).toBe(false);
      expect(metadata.imprintLock).toBe(false);
      expect(metadata.statusLock).toBe(false);
      expect(metadata.ageRatingLock).toBe(false);
      expect(metadata.languageLock).toBe(false);
      expect(metadata.readingDirectionLock).toBe(false);
      expect(metadata.yearLock).toBe(false);
      expect(metadata.totalBookCountLock).toBe(false);
      expect(metadata.genresLock).toBe(false);
      expect(metadata.tagsLock).toBe(false);
      expect(metadata.customMetadataLock).toBe(false);
      expect(metadata.coverLock).toBe(false);
      expect(metadata.authorsJsonLock).toBe(false);
    });

    it("should have alternate titles in metadata", () => {
      const { metadata } = SAMPLE_SERIES_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      expect(metadata.alternateTitles).toBeDefined();
      expect(Array.isArray(metadata.alternateTitles)).toBe(true);
      expect(metadata.alternateTitles?.length).toBeGreaterThan(0);
      expect(metadata.alternateTitles?.[0]).toHaveProperty("label");
      expect(metadata.alternateTitles?.[0]).toHaveProperty("title");
    });

    it("should have authors in metadata", () => {
      const { metadata } = SAMPLE_SERIES_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      expect(metadata.authors).toBeDefined();
      expect(Array.isArray(metadata.authors)).toBe(true);
      expect(metadata.authors?.length).toBeGreaterThan(0);
      expect(metadata.authors?.[0]).toHaveProperty("name");
      expect(metadata.authors?.[0]).toHaveProperty("role");
    });

    it("should have external ratings in metadata", () => {
      const { metadata } = SAMPLE_SERIES_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      expect(metadata.externalRatings).toBeDefined();
      expect(Array.isArray(metadata.externalRatings)).toBe(true);
      expect(metadata.externalRatings?.length).toBeGreaterThan(0);
      expect(metadata.externalRatings?.[0]).toHaveProperty("source");
      expect(metadata.externalRatings?.[0]).toHaveProperty("rating");
    });

    it("should have external links in metadata", () => {
      const { metadata } = SAMPLE_SERIES_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      expect(metadata.externalLinks).toBeDefined();
      expect(Array.isArray(metadata.externalLinks)).toBe(true);
      expect(metadata.externalLinks?.length).toBeGreaterThan(0);
      expect(metadata.externalLinks?.[0]).toHaveProperty("source");
      expect(metadata.externalLinks?.[0]).toHaveProperty("url");
    });
  });

  // ===========================================================================
  // Book Context Tests
  // ===========================================================================

  describe("SAMPLE_BOOK_CONTEXT", () => {
    it("should have type discriminator set to 'book'", () => {
      expect(SAMPLE_BOOK_CONTEXT.type).toBe("book");
    });

    it("should have all required top-level fields", () => {
      expect(SAMPLE_BOOK_CONTEXT).toHaveProperty("bookId");
      expect(SAMPLE_BOOK_CONTEXT).toHaveProperty("seriesId");
      expect(SAMPLE_BOOK_CONTEXT).toHaveProperty("libraryId");
      expect(SAMPLE_BOOK_CONTEXT).toHaveProperty("fileFormat");
      expect(SAMPLE_BOOK_CONTEXT).toHaveProperty("pageCount");
      expect(SAMPLE_BOOK_CONTEXT).toHaveProperty("fileSize");
      expect(SAMPLE_BOOK_CONTEXT).toHaveProperty("metadata");
      expect(SAMPLE_BOOK_CONTEXT).toHaveProperty("externalIds");
      expect(SAMPLE_BOOK_CONTEXT).toHaveProperty("customMetadata");
      expect(SAMPLE_BOOK_CONTEXT).toHaveProperty("series");
    });

    it("should use camelCase for all top-level fields", () => {
      expect("bookId" in SAMPLE_BOOK_CONTEXT).toBe(true);
      expect("seriesId" in SAMPLE_BOOK_CONTEXT).toBe(true);
      expect("libraryId" in SAMPLE_BOOK_CONTEXT).toBe(true);
      expect("fileFormat" in SAMPLE_BOOK_CONTEXT).toBe(true);
      expect("pageCount" in SAMPLE_BOOK_CONTEXT).toBe(true);
      expect("fileSize" in SAMPLE_BOOK_CONTEXT).toBe(true);
      expect("externalIds" in SAMPLE_BOOK_CONTEXT).toBe(true);
      expect("customMetadata" in SAMPLE_BOOK_CONTEXT).toBe(true);

      // Should NOT have snake_case
      expect("book_id" in SAMPLE_BOOK_CONTEXT).toBe(false);
      expect("series_id" in SAMPLE_BOOK_CONTEXT).toBe(false);
      expect("file_format" in SAMPLE_BOOK_CONTEXT).toBe(false);
    });

    it("should have book-specific metadata fields", () => {
      const { metadata } = SAMPLE_BOOK_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      expect(metadata.title).toBe("The Martian");
      expect(metadata.subtitle).toBe("A Novel");
      expect(metadata.number).toBe(1);
      expect(metadata.publisher).toBe("Crown Publishing");
      expect(metadata.bookType).toBe("novel");
      expect(metadata.languageIso).toBe("en");
      expect(metadata.year).toBe(2014);
      expect(metadata.isbns).toBe("978-0553418026");
    });

    it("should have authors array in metadata", () => {
      const { metadata } = SAMPLE_BOOK_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      expect(Array.isArray(metadata.authors)).toBe(true);
      expect(metadata.authors?.length).toBeGreaterThan(0);
      expect(metadata.authors?.[0]).toHaveProperty("name");
      expect(metadata.authors?.[0]?.name).toBe("Andy Weir");
    });

    it("should have awards array in metadata", () => {
      const { metadata } = SAMPLE_BOOK_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      expect(Array.isArray(metadata.awards)).toBe(true);
      expect(metadata.awards?.length).toBe(2);
      expect(metadata.awards?.[0]).toHaveProperty("name");
      expect(metadata.awards?.[0]).toHaveProperty("year");
      expect(metadata.awards?.[0]).toHaveProperty("category");
      expect(metadata.awards?.[0]).toHaveProperty("won");
    });

    it("should have genres and tags arrays", () => {
      const { metadata } = SAMPLE_BOOK_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      expect(Array.isArray(metadata.genres)).toBe(true);
      expect(metadata.genres?.length).toBeGreaterThan(0);
      expect(Array.isArray(metadata.tags)).toBe(true);
      expect(metadata.tags?.length).toBeGreaterThan(0);
    });

    it("should have subjects array", () => {
      const { metadata } = SAMPLE_BOOK_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      expect(Array.isArray(metadata.subjects)).toBe(true);
      expect(metadata.subjects?.length).toBeGreaterThan(0);
    });

    it("should have external links in metadata", () => {
      const { metadata } = SAMPLE_BOOK_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      expect(Array.isArray(metadata.externalLinks)).toBe(true);
      expect(metadata.externalLinks?.length).toBeGreaterThan(0);
      expect(metadata.externalLinks?.[0]).toHaveProperty("source");
      expect(metadata.externalLinks?.[0]).toHaveProperty("url");
    });

    it("should have lock fields in metadata", () => {
      const { metadata } = SAMPLE_BOOK_CONTEXT;
      expect(metadata).toBeDefined();
      if (!metadata) return;

      // Book-specific lock fields
      expect(metadata).toHaveProperty("titleLock");
      expect(metadata).toHaveProperty("numberLock");
      expect(metadata).toHaveProperty("subtitleLock");
      expect(metadata).toHaveProperty("bookTypeLock");
      expect(metadata).toHaveProperty("languageIsoLock");
      expect(metadata).toHaveProperty("isbnsLock");
      expect(metadata).toHaveProperty("awardsJsonLock");
      expect(metadata).toHaveProperty("subjectsLock");
      expect(metadata).toHaveProperty("coverLock");
      expect(metadata).toHaveProperty("customMetadataLock");
    });

    it("should have embedded parent series context", () => {
      const { series } = SAMPLE_BOOK_CONTEXT;
      expect(series).toBeDefined();
      expect(series.type).toBe("series");
      expect(series).toHaveProperty("seriesId");
      expect(series).toHaveProperty("bookCount");
      expect(series).toHaveProperty("metadata");
      expect(series).toHaveProperty("externalIds");
    });

    it("should be JSON serializable/deserializable", () => {
      const json = JSON.stringify(SAMPLE_BOOK_CONTEXT);
      const parsed = JSON.parse(json);

      expect(parsed.type).toBe("book");
      expect(parsed.bookId).toBe(SAMPLE_BOOK_CONTEXT.bookId);
      expect(parsed.fileFormat).toBe(SAMPLE_BOOK_CONTEXT.fileFormat);
      expect(parsed.series.type).toBe("series");
    });
  });

  describe("transformFullBookToBookContext", () => {
    /**
     * Creates a minimal valid FullBook object for testing
     */
    function createMockFullBook(overrides: Partial<FullBook> = {}): FullBook {
      return {
        id: "book-uuid-1",
        libraryId: "lib-uuid-1",
        libraryName: "Test Library",
        seriesId: "series-uuid-1",
        seriesName: "Test Series",
        title: "Test Book",
        titleSort: "test book",
        filePath: "/path/to/book.epub",
        fileFormat: "epub",
        fileSize: 1024000,
        fileHash: "abc123",
        pageCount: 200,
        number: 1,
        deleted: false,
        analyzed: true,
        createdAt: "2024-01-01T00:00:00Z",
        updatedAt: "2024-01-15T10:30:00Z",
        genres: [],
        tags: [],
        metadata: {
          title: "Test Book",
          titleSort: "test book",
          summary: null,
          publisher: null,
          imprint: null,
          genre: null,
          languageIso: null,
          formatDetail: null,
          blackAndWhite: null,
          manga: null,
          year: null,
          month: null,
          day: null,
          volume: null,
          count: null,
          isbns: null,
          bookType: null,
          subtitle: null,
          authors: null,
          translator: null,
          edition: null,
          originalTitle: null,
          originalYear: null,
          seriesPosition: null,
          seriesTotal: null,
          subjects: null,
          awards: null,
          customMetadata: null,
          colorists: [],
          coverArtists: [],
          editors: [],
          inkers: [],
          letterers: [],
          pencillers: [],
          writers: [],
          createdAt: "2024-01-01T00:00:00Z",
          updatedAt: "2024-01-01T00:00:00Z",
          locks: {
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
            writerLock: false,
            pencillerLock: false,
            inkerLock: false,
            coloristLock: false,
            lettererLock: false,
            coverArtistLock: false,
            editorLock: false,
          },
        },
        ...overrides,
      } as FullBook;
    }

    function createMockSeriesContext(): ReturnType<
      typeof transformFullSeriesToSeriesContext
    > {
      const series = createMockFullSeries();
      return transformFullSeriesToSeriesContext(series);
    }

    function createMockFullSeries(): FullSeries {
      return {
        id: "series-uuid-1",
        name: "Test Series",
        nameSort: "Test Series",
        libraryId: "lib-uuid-1",
        libraryName: "Test Library",
        bookCount: 5,
        deleted: false,
        createdAt: "2024-01-01T00:00:00Z",
        updatedAt: "2024-01-01T00:00:00Z",
        genres: [],
        tags: [],
        alternateTitles: [],
        externalRatings: [],
        externalLinks: [],
        externalIds: [],
        metadata: {
          title: "Test Series",
          titleSort: "Test Series",
          summary: null,
          publisher: null,
          imprint: null,
          language: null,
          ageRating: null,
          year: null,
          status: null,
          totalBookCount: null,
          readingDirection: null,
          customMetadata: null,
          authors: null,
          locks: {
            title: false,
            titleSort: false,
            summary: false,
            publisher: false,
            imprint: false,
            language: false,
            ageRating: false,
            year: false,
            status: false,
            totalBookCount: false,
            readingDirection: false,
            customMetadata: false,
            genres: false,
            tags: false,
            cover: false,
            authorsJsonLock: false,
            alternateTitles: false,
          },
          createdAt: "2024-01-01T00:00:00Z",
          updatedAt: "2024-01-01T00:00:00Z",
        },
      } as FullSeries;
    }

    it("should set type discriminator to 'book'", () => {
      const book = createMockFullBook();
      const seriesCtx = createMockSeriesContext();
      const result = transformFullBookToBookContext(book, seriesCtx);

      expect(result.type).toBe("book");
    });

    it("should transform top-level book fields", () => {
      const book = createMockFullBook({
        id: "my-book-id",
        seriesId: "my-series-id",
        libraryId: "my-lib-id",
        fileFormat: "cbz",
        pageCount: 32,
        fileSize: 52428800,
      });
      const seriesCtx = createMockSeriesContext();
      const result = transformFullBookToBookContext(book, seriesCtx);

      expect(result.bookId).toBe("my-book-id");
      expect(result.seriesId).toBe("my-series-id");
      expect(result.libraryId).toBe("my-lib-id");
      expect(result.fileFormat).toBe("cbz");
      expect(result.pageCount).toBe(32);
      expect(result.fileSize).toBe(52428800);
    });

    it("should transform book metadata scalar fields", () => {
      const book = createMockFullBook();
      book.metadata.title = "The Martian";
      book.metadata.titleSort = "Martian, The";
      book.metadata.subtitle = "A Novel";
      book.metadata.publisher = "Crown Publishing";
      book.metadata.year = 2014;
      book.metadata.languageIso = "en";
      book.metadata.isbns = "978-0553418026";
      book.metadata.bookType = "novel";

      const seriesCtx = createMockSeriesContext();
      const result = transformFullBookToBookContext(book, seriesCtx);

      expect(result.metadata?.title).toBe("The Martian");
      expect(result.metadata?.titleSort).toBe("Martian, The");
      expect(result.metadata?.subtitle).toBe("A Novel");
      expect(result.metadata?.publisher).toBe("Crown Publishing");
      expect(result.metadata?.year).toBe(2014);
      expect(result.metadata?.languageIso).toBe("en");
      expect(result.metadata?.isbns).toBe("978-0553418026");
      expect(result.metadata?.bookType).toBe("novel");
    });

    it("should transform authors from metadata", () => {
      const book = createMockFullBook();
      book.metadata.authors = [
        { name: "Andy Weir", role: "author", sortName: "Weir, Andy" },
        { name: "John Doe", role: "illustrator" },
      ];

      const seriesCtx = createMockSeriesContext();
      const result = transformFullBookToBookContext(book, seriesCtx);

      expect(result.metadata?.authors).toHaveLength(2);
      expect(result.metadata?.authors?.[0]?.name).toBe("Andy Weir");
      expect(result.metadata?.authors?.[0]?.role).toBe("author");
      expect(result.metadata?.authors?.[0]?.sortName).toBe("Weir, Andy");
      expect(result.metadata?.authors?.[1]?.name).toBe("John Doe");
    });

    it("should transform awards from metadata", () => {
      const book = createMockFullBook();
      book.metadata.awards = [
        { name: "Hugo Award", year: 2015, category: "Best Novel", won: true },
        { name: "Nebula Award", year: 2014, won: false },
      ];

      const seriesCtx = createMockSeriesContext();
      const result = transformFullBookToBookContext(book, seriesCtx);

      expect(result.metadata?.awards).toHaveLength(2);
      expect(result.metadata?.awards?.[0]?.name).toBe("Hugo Award");
      expect(result.metadata?.awards?.[0]?.won).toBe(true);
      expect(result.metadata?.awards?.[1]?.won).toBe(false);
    });

    it("should transform genres and tags from top-level arrays", () => {
      const book = createMockFullBook({
        genres: [
          {
            id: "g1",
            name: "Science Fiction",
            createdAt: "2024-01-01T00:00:00Z",
          },
          { id: "g2", name: "Adventure", createdAt: "2024-01-01T00:00:00Z" },
        ],
        tags: [
          { id: "t1", name: "mars", createdAt: "2024-01-01T00:00:00Z" },
          { id: "t2", name: "survival", createdAt: "2024-01-01T00:00:00Z" },
        ],
      });

      const seriesCtx = createMockSeriesContext();
      const result = transformFullBookToBookContext(book, seriesCtx);

      expect(result.metadata?.genres).toEqual(["Science Fiction", "Adventure"]);
      expect(result.metadata?.tags).toEqual(["mars", "survival"]);
    });

    it("should build external IDs map from array", () => {
      const book = createMockFullBook();
      const seriesCtx = createMockSeriesContext();
      const bookExternalIds: components["schemas"]["BookExternalIdDto"][] = [
        {
          id: "eid-1",
          bookId: "book-uuid-1",
          source: "plugin:goodreads",
          externalId: "18007564",
          externalUrl: "https://goodreads.com/book/show/18007564",
          metadataHash: null,
          createdAt: "2024-01-01T00:00:00Z",
          updatedAt: "2024-01-01T00:00:00Z",
        },
      ];

      const result = transformFullBookToBookContext(
        book,
        seriesCtx,
        bookExternalIds,
      );

      expect(result.externalIds).toBeDefined();
      expect(result.externalIds?.["plugin:goodreads"]).toBeDefined();
      expect(result.externalIds?.["plugin:goodreads"]?.id).toBe("18007564");
      expect(result.externalIds?.["plugin:goodreads"]?.url).toBe(
        "https://goodreads.com/book/show/18007564",
      );
    });

    it("should transform external links from separate array", () => {
      const book = createMockFullBook();
      const seriesCtx = createMockSeriesContext();
      const bookExternalLinks: components["schemas"]["BookExternalLinkDto"][] =
        [
          {
            id: "link-1",
            bookId: "book-uuid-1",
            sourceName: "Goodreads",
            url: "https://goodreads.com/book/show/18007564",
            externalId: "18007564",
            createdAt: "2024-01-01T00:00:00Z",
            updatedAt: "2024-01-01T00:00:00Z",
          },
        ];

      const result = transformFullBookToBookContext(
        book,
        seriesCtx,
        [],
        bookExternalLinks,
      );

      expect(result.metadata?.externalLinks).toHaveLength(1);
      expect(result.metadata?.externalLinks?.[0]?.source).toBe("Goodreads");
      expect(result.metadata?.externalLinks?.[0]?.url).toBe(
        "https://goodreads.com/book/show/18007564",
      );
      expect(result.metadata?.externalLinks?.[0]?.externalId).toBe("18007564");
    });

    it("should preserve custom metadata as-is", () => {
      const book = createMockFullBook();
      // Cast to bypass the restrictive Record<string, never> type
      (book.metadata as Record<string, unknown>).customMetadata = {
        myField: "value",
        nested: { key: 42 },
      };

      const seriesCtx = createMockSeriesContext();
      const result = transformFullBookToBookContext(book, seriesCtx);

      expect(result.customMetadata).toBeDefined();
      expect(result.customMetadata?.myField).toBe("value");
      expect(result.customMetadata?.nested).toEqual({ key: 42 });
    });

    it("should embed the parent series context", () => {
      const book = createMockFullBook();
      const seriesCtx = createMockSeriesContext();
      const result = transformFullBookToBookContext(book, seriesCtx);

      expect(result.series).toBeDefined();
      expect(result.series.type).toBe("series");
      expect(result.series).toBe(seriesCtx);
    });

    it("should transform lock fields from metadata.locks", () => {
      const book = createMockFullBook();
      book.metadata.locks.titleLock = true;
      book.metadata.locks.publisherLock = true;
      book.metadata.locks.coverLock = true;

      const seriesCtx = createMockSeriesContext();
      const result = transformFullBookToBookContext(book, seriesCtx);

      expect(result.metadata?.titleLock).toBe(true);
      expect(result.metadata?.publisherLock).toBe(true);
      expect(result.metadata?.coverLock).toBe(true);
      expect(result.metadata?.summaryLock).toBe(false);
      expect(result.metadata?.yearLock).toBe(false);
    });

    it("should handle null/undefined optional fields gracefully", () => {
      const book = createMockFullBook();
      const seriesCtx = createMockSeriesContext();
      const result = transformFullBookToBookContext(book, seriesCtx);

      expect(result.metadata?.summary).toBeNull();
      expect(result.metadata?.publisher).toBeNull();
      expect(result.metadata?.subtitle).toBeNull();
      expect(result.metadata?.authors).toEqual([]);
      expect(result.metadata?.awards).toEqual([]);
      expect(result.metadata?.subjects).toEqual([]);
      expect(result.metadata?.genres).toEqual([]);
      expect(result.metadata?.tags).toEqual([]);
    });

    it("should handle empty external IDs and links", () => {
      const book = createMockFullBook();
      const seriesCtx = createMockSeriesContext();
      const result = transformFullBookToBookContext(book, seriesCtx, [], []);

      expect(result.externalIds).toEqual({});
      expect(result.metadata?.externalLinks).toEqual([]);
    });
  });
});
