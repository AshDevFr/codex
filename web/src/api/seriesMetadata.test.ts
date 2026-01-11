import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { seriesMetadataApi } from "./seriesMetadata";
import { api } from "./client";

// Mock the api client
vi.mock("./client", () => ({
	api: {
		get: vi.fn(),
		put: vi.fn(),
		post: vi.fn(),
		patch: vi.fn(),
		delete: vi.fn(),
	},
}));

describe("seriesMetadataApi", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	describe("getFullMetadata", () => {
		it("should fetch full metadata for a series", async () => {
			const mockMetadata = {
				metadata: {
					id: "series-123",
					name: "Test Series",
					summary: "A test series",
				},
				genres: [{ id: "genre-1", name: "Action", seriesCount: 10 }],
				tags: [{ id: "tag-1", name: "Favorite", seriesCount: 5 }],
				alternateTitles: [{ id: "alt-1", title: "Alt Title", label: "Japanese" }],
				externalRatings: [{ id: "er-1", sourceName: "MAL", rating: 85, voteCount: 1000 }],
				externalLinks: [{ id: "el-1", sourceName: "MAL", url: "https://myanimelist.net/..." }],
				locks: { name: false, summary: true },
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockMetadata });

			const result = await seriesMetadataApi.getFullMetadata("series-123");

			expect(api.get).toHaveBeenCalledWith("/series/series-123/metadata/full");
			expect(result).toEqual(mockMetadata);
		});
	});

	describe("getLocks", () => {
		it("should fetch metadata locks for a series", async () => {
			const mockLocks = {
				name: true,
				summary: false,
				publisher: true,
				year: false,
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockLocks });

			const result = await seriesMetadataApi.getLocks("series-123");

			expect(api.get).toHaveBeenCalledWith("/series/series-123/metadata/locks");
			expect(result).toEqual(mockLocks);
		});
	});

	describe("updateLocks", () => {
		it("should update metadata locks for a series", async () => {
			const mockLocks = {
				name: true,
				summary: true,
			};

			vi.mocked(api.put).mockResolvedValueOnce({ data: mockLocks });

			const result = await seriesMetadataApi.updateLocks("series-123", {
				name: true,
				summary: true,
			});

			expect(api.put).toHaveBeenCalledWith("/series/series-123/metadata/locks", {
				name: true,
				summary: true,
			});
			expect(result).toEqual(mockLocks);
		});
	});

	describe("replaceMetadata", () => {
		it("should replace all metadata for a series", async () => {
			const mockResponse = {
				metadata: {
					id: "series-123",
					name: "Updated Series",
					summary: "Updated summary",
				},
			};

			vi.mocked(api.put).mockResolvedValueOnce({ data: mockResponse });

			const result = await seriesMetadataApi.replaceMetadata("series-123", {
				name: "Updated Series",
				summary: "Updated summary",
			});

			expect(api.put).toHaveBeenCalledWith("/series/series-123/metadata", {
				name: "Updated Series",
				summary: "Updated summary",
			});
			expect(result).toEqual(mockResponse);
		});
	});

	describe("patchMetadata", () => {
		it("should partially update metadata for a series", async () => {
			const mockResponse = {
				metadata: {
					id: "series-123",
					name: "Original Name",
					summary: "Patched summary",
				},
			};

			vi.mocked(api.patch).mockResolvedValueOnce({ data: mockResponse });

			const result = await seriesMetadataApi.patchMetadata("series-123", {
				summary: "Patched summary",
			});

			expect(api.patch).toHaveBeenCalledWith("/series/series-123/metadata", {
				summary: "Patched summary",
			});
			expect(result).toEqual(mockResponse);
		});
	});

	describe("Alternate Titles", () => {
		describe("getAlternateTitles", () => {
			it("should fetch alternate titles for a series", async () => {
				const mockResponse = {
					titles: [
						{ id: "alt-1", title: "Japanese Title", label: "Japanese" },
						{ id: "alt-2", title: "Romaji Title", label: "Romaji" },
					],
				};

				vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

				const result = await seriesMetadataApi.getAlternateTitles("series-123");

				expect(api.get).toHaveBeenCalledWith("/series/series-123/alternate-titles");
				expect(result).toEqual(mockResponse.titles);
			});
		});

		describe("createAlternateTitle", () => {
			it("should create an alternate title", async () => {
				const mockTitle = {
					id: "alt-new",
					title: "New Alt Title",
					label: "Korean",
				};

				vi.mocked(api.post).mockResolvedValueOnce({ data: mockTitle });

				const result = await seriesMetadataApi.createAlternateTitle(
					"series-123",
					"New Alt Title",
					"Korean",
				);

				expect(api.post).toHaveBeenCalledWith("/series/series-123/alternate-titles", {
					title: "New Alt Title",
					label: "Korean",
				});
				expect(result).toEqual(mockTitle);
			});
		});

		describe("updateAlternateTitle", () => {
			it("should update an alternate title", async () => {
				const mockTitle = {
					id: "alt-1",
					title: "Updated Title",
					label: "Updated Label",
				};

				vi.mocked(api.put).mockResolvedValueOnce({ data: mockTitle });

				const result = await seriesMetadataApi.updateAlternateTitle(
					"series-123",
					"alt-1",
					"Updated Title",
					"Updated Label",
				);

				expect(api.put).toHaveBeenCalledWith("/series/series-123/alternate-titles/alt-1", {
					title: "Updated Title",
					label: "Updated Label",
				});
				expect(result).toEqual(mockTitle);
			});
		});

		describe("deleteAlternateTitle", () => {
			it("should delete an alternate title", async () => {
				vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

				await seriesMetadataApi.deleteAlternateTitle("series-123", "alt-1");

				expect(api.delete).toHaveBeenCalledWith("/series/series-123/alternate-titles/alt-1");
			});
		});
	});

	describe("External Ratings", () => {
		describe("getExternalRatings", () => {
			it("should fetch external ratings for a series", async () => {
				const mockResponse = {
					ratings: [
						{ id: "er-1", sourceName: "MAL", rating: 85, voteCount: 1000 },
						{ id: "er-2", sourceName: "AniList", rating: 82, voteCount: 500 },
					],
				};

				vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

				const result = await seriesMetadataApi.getExternalRatings("series-123");

				expect(api.get).toHaveBeenCalledWith("/series/series-123/external-ratings");
				expect(result).toEqual(mockResponse.ratings);
			});
		});

		describe("createExternalRating", () => {
			it("should create an external rating with vote count", async () => {
				const mockRating = {
					id: "er-new",
					sourceName: "Goodreads",
					rating: 78,
					voteCount: 2000,
				};

				vi.mocked(api.post).mockResolvedValueOnce({ data: mockRating });

				const result = await seriesMetadataApi.createExternalRating(
					"series-123",
					"Goodreads",
					78,
					2000,
				);

				expect(api.post).toHaveBeenCalledWith("/series/series-123/external-ratings", {
					source_name: "Goodreads",
					rating: 78,
					vote_count: 2000,
				});
				expect(result).toEqual(mockRating);
			});

			it("should create an external rating without vote count", async () => {
				const mockRating = {
					id: "er-new",
					sourceName: "Custom",
					rating: 90,
					voteCount: null,
				};

				vi.mocked(api.post).mockResolvedValueOnce({ data: mockRating });

				const result = await seriesMetadataApi.createExternalRating(
					"series-123",
					"Custom",
					90,
				);

				expect(api.post).toHaveBeenCalledWith("/series/series-123/external-ratings", {
					source_name: "Custom",
					rating: 90,
					vote_count: undefined,
				});
				expect(result).toEqual(mockRating);
			});
		});

		describe("deleteExternalRating", () => {
			it("should delete an external rating", async () => {
				vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

				await seriesMetadataApi.deleteExternalRating("series-123", "er-1");

				expect(api.delete).toHaveBeenCalledWith("/series/series-123/external-ratings/er-1");
			});
		});
	});

	describe("External Links", () => {
		describe("getExternalLinks", () => {
			it("should fetch external links for a series", async () => {
				const mockResponse = {
					links: [
						{ id: "el-1", sourceName: "MAL", url: "https://myanimelist.net/manga/1", externalId: "1" },
						{ id: "el-2", sourceName: "AniList", url: "https://anilist.co/manga/1", externalId: "1" },
					],
				};

				vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

				const result = await seriesMetadataApi.getExternalLinks("series-123");

				expect(api.get).toHaveBeenCalledWith("/series/series-123/external-links");
				expect(result).toEqual(mockResponse.links);
			});
		});

		describe("createExternalLink", () => {
			it("should create an external link with external id", async () => {
				const mockLink = {
					id: "el-new",
					sourceName: "MangaUpdates",
					url: "https://mangaupdates.com/series/xyz",
					externalId: "xyz",
				};

				vi.mocked(api.post).mockResolvedValueOnce({ data: mockLink });

				const result = await seriesMetadataApi.createExternalLink(
					"series-123",
					"MangaUpdates",
					"https://mangaupdates.com/series/xyz",
					"xyz",
				);

				expect(api.post).toHaveBeenCalledWith("/series/series-123/external-links", {
					source_name: "MangaUpdates",
					url: "https://mangaupdates.com/series/xyz",
					external_id: "xyz",
				});
				expect(result).toEqual(mockLink);
			});

			it("should create an external link without external id", async () => {
				const mockLink = {
					id: "el-new",
					sourceName: "Custom",
					url: "https://example.com/series",
					externalId: null,
				};

				vi.mocked(api.post).mockResolvedValueOnce({ data: mockLink });

				const result = await seriesMetadataApi.createExternalLink(
					"series-123",
					"Custom",
					"https://example.com/series",
				);

				expect(api.post).toHaveBeenCalledWith("/series/series-123/external-links", {
					source_name: "Custom",
					url: "https://example.com/series",
					external_id: undefined,
				});
				expect(result).toEqual(mockLink);
			});
		});

		describe("deleteExternalLink", () => {
			it("should delete an external link", async () => {
				vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

				await seriesMetadataApi.deleteExternalLink("series-123", "el-1");

				expect(api.delete).toHaveBeenCalledWith("/series/series-123/external-links/el-1");
			});
		});
	});

	describe("Cover Management", () => {
		describe("listCovers", () => {
			it("should list covers for a series", async () => {
				const mockResponse = {
					covers: [
						{ id: "cover-1", bookId: "book-1", selected: true },
						{ id: "cover-2", bookId: "book-2", selected: false },
					],
				};

				vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

				const result = await seriesMetadataApi.listCovers("series-123");

				expect(api.get).toHaveBeenCalledWith("/series/series-123/covers");
				expect(result).toEqual(mockResponse.covers);
			});
		});

		describe("selectCover", () => {
			it("should select a cover for a series", async () => {
				vi.mocked(api.put).mockResolvedValueOnce({ data: {} });

				await seriesMetadataApi.selectCover("series-123", "cover-2");

				expect(api.put).toHaveBeenCalledWith("/series/series-123/covers/cover-2/select");
			});
		});

		describe("deleteCover", () => {
			it("should delete a cover", async () => {
				vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

				await seriesMetadataApi.deleteCover("series-123", "cover-1");

				expect(api.delete).toHaveBeenCalledWith("/series/series-123/covers/cover-1");
			});
		});
	});
});
