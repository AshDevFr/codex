import { describe, expect, it } from "vitest";
import { mapSearchResult, mapSeriesMetadata } from "./mappers.js";
import type { MbSeries } from "./types.js";

describe("mappers", () => {
  describe("mapSearchResult", () => {
    it("should map a series to SearchResult", () => {
      const series: MbSeries = {
        id: 12345,
        state: "active",
        title: "Test Manga",
        native_title: "テストマンガ",
        romanized_title: "Tesuto Manga",
        cover: {
          raw: { url: "https://cdn.mangabaka.org/covers/12345.jpg" },
          x150: { x1: null, x2: null, x3: null },
          x250: { x1: "https://cdn.mangabaka.org/covers/12345_250.jpg", x2: null, x3: null },
          x350: { x1: null, x2: null, x3: null },
        },
        description: "A test manga description",
        type: "manga",
        year: 2020,
        status: "releasing",
        genres: ["action", "adventure"],
        rating: {
          bayesian: 8.5,
        },
      };

      const result = mapSearchResult(series);

      expect(result.externalId).toBe("12345");
      expect(result.title).toBe("Test Manga");
      expect(result.alternateTitles).toContain("テストマンガ");
      expect(result.alternateTitles).toContain("Tesuto Manga");
      expect(result.year).toBe(2020);
      expect(result.coverUrl).toBe("https://cdn.mangabaka.org/covers/12345_250.jpg");
      expect(result.preview?.status).toBe("ongoing");
      expect(result.preview?.rating).toBe(8.5);
      expect(result.preview?.description).toBe("A test manga description");
      // relevanceScore is not set - API returns results in relevance order
      expect(result.relevanceScore).toBeUndefined();
    });

    it("should handle missing fields gracefully", () => {
      const series: MbSeries = {
        id: 99999,
        state: "active",
        title: "Minimal Entry",
        cover: {
          raw: { url: null },
          x150: { x1: null, x2: null, x3: null },
          x250: { x1: null, x2: null, x3: null },
          x350: { x1: null, x2: null, x3: null },
        },
        type: "manga",
        status: "unknown",
      };

      const result = mapSearchResult(series);

      expect(result.externalId).toBe("99999");
      expect(result.title).toBe("Minimal Entry");
      expect(result.year).toBeUndefined();
      expect(result.coverUrl).toBeUndefined();
      expect(result.preview?.rating).toBeUndefined();
      expect(result.relevanceScore).toBeUndefined();
    });
  });

  describe("mapSeriesMetadata", () => {
    it("should map full series response to PluginSeriesMetadata", () => {
      const series: MbSeries = {
        id: 12345,
        state: "active",
        title: "Test Manga",
        native_title: "テストマンガ",
        romanized_title: "Tesuto Manga",
        secondary_titles: {
          en: [{ type: "alternative", title: "Test Manga: Subtitle" }],
          ja: [{ type: "native", title: "テストマンガ外伝" }],
        },
        cover: {
          raw: { url: "https://cdn.mangabaka.org/covers/12345.jpg" },
          x150: { x1: null, x2: null, x3: null },
          x250: { x1: null, x2: null, x3: null },
          x350: { x1: "https://cdn.mangabaka.org/covers/12345_350.jpg", x2: null, x3: null },
        },
        description: "A great manga about testing.",
        type: "manga",
        year: 2020,
        status: "releasing",
        is_licensed: true,
        has_anime: true,
        country_of_origin: "jp",
        content_rating: "safe",
        genres: ["action", "drama"],
        tags: ["Strong Lead", "Time Travel"],
        authors: ["Test Author"],
        artists: ["Test Artist"],
        source: {
          anilist: { id: 111, rating: 8.3, rating_normalized: 83 },
          my_anime_list: { id: 222, rating: 7.96, rating_normalized: 80 },
        },
        rating: {
          bayesian: 8.75,
        },
      };

      const result = mapSeriesMetadata(series);

      expect(result.externalId).toBe("12345");
      expect(result.externalUrl).toBe("https://mangabaka.org/12345");
      expect(result.title).toBe("Test Manga");

      // Check alternate titles
      expect(result.alternateTitles.length).toBeGreaterThanOrEqual(2);
      expect(result.alternateTitles).toContainEqual({
        title: "テストマンガ",
        language: "ja",
        titleType: "native",
      });
      expect(result.alternateTitles).toContainEqual({
        title: "Tesuto Manga",
        language: "en",
        titleType: "romaji",
      });

      expect(result.summary).toBe("A great manga about testing.");
      expect(result.status).toBe("ongoing");
      expect(result.year).toBe(2020);
      expect(result.genres).toEqual(["Action", "Drama"]);
      expect(result.tags).toEqual(["Strong Lead", "Time Travel"]);
      expect(result.authors).toEqual(["Test Author"]);
      expect(result.artists).toEqual(["Test Artist"]);
      expect(result.rating).toEqual({ score: 8.75, source: "mangabaka" });

      // Check external links
      expect(result.externalLinks).toContainEqual({
        label: "MangaBaka",
        url: "https://mangabaka.org/12345",
        linkType: "provider",
      });
      expect(result.externalLinks).toContainEqual({
        label: "AniList",
        url: "https://anilist.co/manga/111",
        linkType: "provider",
      });
      expect(result.externalLinks).toContainEqual({
        label: "MyAnimeList",
        url: "https://myanimelist.net/manga/222",
        linkType: "provider",
      });

      // Check external ratings are extracted dynamically
      expect(result.externalRatings).toContainEqual({ score: 83, source: "anilist" });
      expect(result.externalRatings).toContainEqual({ score: 80, source: "myanimelist" });
    });

    it("should dynamically extract all external ratings from sources", () => {
      const series: MbSeries = {
        id: 1668,
        state: "active",
        title: "Test Series with Many Sources",
        cover: {
          raw: { url: null },
          x150: { x1: null, x2: null, x3: null },
          x250: { x1: null, x2: null, x3: null },
          x350: { x1: null, x2: null, x3: null },
        },
        type: "manga",
        status: "completed",
        source: {
          anilist: { id: 31668, rating: 8.3, rating_normalized: 83 },
          anime_planet: { id: "test-manga", rating: 4, rating_normalized: 80 },
          anime_news_network: { id: null, rating: null, rating_normalized: null },
          kitsu: { id: 3556, rating: 7.759, rating_normalized: 78 },
          manga_updates: { id: "hly6oqa", rating: 7.74, rating_normalized: 77 },
          my_anime_list: { id: 1668, rating: 7.96, rating_normalized: 80 },
          shikimori: { id: 1668, rating: 7.96, rating_normalized: 80 },
        },
      };

      const result = mapSeriesMetadata(series);

      // Should include all sources with valid rating_normalized (excluding anime_news_network which has null)
      expect(result.externalRatings).toHaveLength(6);
      expect(result.externalRatings).toContainEqual({ score: 83, source: "anilist" });
      expect(result.externalRatings).toContainEqual({ score: 80, source: "animeplanet" });
      expect(result.externalRatings).toContainEqual({ score: 78, source: "kitsu" });
      expect(result.externalRatings).toContainEqual({ score: 77, source: "mangaupdates" });
      expect(result.externalRatings).toContainEqual({ score: 80, source: "myanimelist" });
      expect(result.externalRatings).toContainEqual({ score: 80, source: "shikimori" });

      // Should NOT include anime_news_network (rating_normalized is null)
      expect(result.externalRatings).not.toContainEqual(
        expect.objectContaining({ source: "animenewsnetwork" }),
      );
    });

    it("should map completed series correctly", () => {
      const series: MbSeries = {
        id: 1,
        state: "active",
        title: "Completed Manga",
        cover: {
          raw: { url: null },
          x150: { x1: null, x2: null, x3: null },
          x250: { x1: null, x2: null, x3: null },
          x350: { x1: null, x2: null, x3: null },
        },
        type: "manga",
        year: 2010,
        status: "completed",
      };

      const result = mapSeriesMetadata(series);

      // "completed" from MangaBaka maps to "ended" in Codex
      expect(result.status).toBe("ended");
      expect(result.year).toBe(2010);
    });

    it("should map cancelled series to abandoned", () => {
      const series: MbSeries = {
        id: 3,
        state: "active",
        title: "Cancelled Manga",
        cover: {
          raw: { url: null },
          x150: { x1: null, x2: null, x3: null },
          x250: { x1: null, x2: null, x3: null },
          x350: { x1: null, x2: null, x3: null },
        },
        type: "manga",
        status: "cancelled",
      };

      const result = mapSeriesMetadata(series);

      // "cancelled" from MangaBaka maps to "abandoned" in Codex
      expect(result.status).toBe("abandoned");
    });

    it("should detect language from country of origin", () => {
      const series: MbSeries = {
        id: 2,
        state: "active",
        title: "Korean Manhwa",
        native_title: "한국 만화",
        cover: {
          raw: { url: null },
          x150: { x1: null, x2: null, x3: null },
          x250: { x1: null, x2: null, x3: null },
          x350: { x1: null, x2: null, x3: null },
        },
        type: "manhwa",
        status: "releasing",
        country_of_origin: "kr",
      };

      const result = mapSeriesMetadata(series);

      expect(result.alternateTitles).toContainEqual({
        title: "한국 만화",
        language: "ko",
        titleType: "native",
      });
      expect(result.readingDirection).toBe("ltr"); // Manhwa is left-to-right
    });
  });
});
