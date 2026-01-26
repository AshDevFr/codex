import { describe, expect, it } from "vitest";
import type { FullSeries, FullSeriesMetadata } from "@/types";
import {
	transformFullSeriesToMetadataForTemplate,
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
});
