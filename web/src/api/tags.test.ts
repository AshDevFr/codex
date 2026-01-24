import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";
import { tagsApi } from "./tags";

// Mock the api client
vi.mock("./client", () => ({
	api: {
		get: vi.fn(),
		put: vi.fn(),
		post: vi.fn(),
		delete: vi.fn(),
	},
}));

describe("tagsApi", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	describe("getAll", () => {
		it("should fetch all tags", async () => {
			const mockTags = [
				{ id: "tag-1", name: "Completed", seriesCount: 15 },
				{ id: "tag-2", name: "Favorite", seriesCount: 8 },
				{ id: "tag-3", name: "To Read", seriesCount: 20 },
			];
			const mockResponse = {
				data: mockTags,
				page: 1,
				pageSize: 50,
				total: 3,
				totalPages: 1,
				links: {
					self: "/api/v1/tags?page=1&page_size=50",
					first: "/api/v1/tags?page=1&page_size=50",
					last: "/api/v1/tags?page=1&page_size=50",
				},
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await tagsApi.getAll();

			expect(api.get).toHaveBeenCalledWith("/tags");
			expect(result).toEqual(mockTags);
		});

		it("should return empty array when no tags exist", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({
				data: {
					data: [],
					page: 1,
					pageSize: 50,
					total: 0,
					totalPages: 0,
					links: {
						self: "/api/v1/tags?page=1&page_size=50",
						first: "/api/v1/tags?page=1&page_size=50",
						last: "/api/v1/tags?page=1&page_size=50",
					},
				},
			});

			const result = await tagsApi.getAll();

			expect(result).toEqual([]);
		});
	});

	describe("getForSeries", () => {
		it("should fetch tags for a specific series", async () => {
			const mockResponse = {
				tags: [
					{ id: "tag-1", name: "Completed", seriesCount: 15 },
					{ id: "tag-2", name: "Favorite", seriesCount: 8 },
				],
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await tagsApi.getForSeries("series-123");

			expect(api.get).toHaveBeenCalledWith("/series/series-123/tags");
			expect(result).toEqual(mockResponse.tags);
		});
	});

	describe("setForSeries", () => {
		it("should set tags for a series (replacing existing)", async () => {
			const mockResponse = {
				tags: [
					{ id: "tag-1", name: "Reading", seriesCount: 10 },
					{ id: "tag-new", name: "New Tag", seriesCount: 1 },
				],
			};

			vi.mocked(api.put).mockResolvedValueOnce({ data: mockResponse });

			const result = await tagsApi.setForSeries("series-123", [
				"Reading",
				"New Tag",
			]);

			expect(api.put).toHaveBeenCalledWith("/series/series-123/tags", {
				tags: ["Reading", "New Tag"],
			});
			expect(result).toEqual(mockResponse.tags);
		});

		it("should clear all tags when given empty array", async () => {
			vi.mocked(api.put).mockResolvedValueOnce({ data: { tags: [] } });

			const result = await tagsApi.setForSeries("series-123", []);

			expect(api.put).toHaveBeenCalledWith("/series/series-123/tags", {
				tags: [],
			});
			expect(result).toEqual([]);
		});
	});

	describe("addToSeries", () => {
		it("should add a single tag to a series", async () => {
			const mockTag = { id: "tag-new", name: "Must Read", seriesCount: 1 };

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockTag });

			const result = await tagsApi.addToSeries("series-123", "Must Read");

			expect(api.post).toHaveBeenCalledWith("/series/series-123/tags", {
				name: "Must Read",
			});
			expect(result).toEqual(mockTag);
		});
	});

	describe("removeFromSeries", () => {
		it("should remove a tag from a series", async () => {
			vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

			await tagsApi.removeFromSeries("series-123", "tag-1");

			expect(api.delete).toHaveBeenCalledWith("/series/series-123/tags/tag-1");
		});
	});

	describe("delete", () => {
		it("should delete a tag globally", async () => {
			vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

			await tagsApi.delete("tag-1");

			expect(api.delete).toHaveBeenCalledWith("/tags/tag-1");
		});
	});

	describe("cleanup", () => {
		it("should cleanup unused tags", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: { deleted_count: 3 },
			});

			const result = await tagsApi.cleanup();

			expect(api.post).toHaveBeenCalledWith("/tags/cleanup");
			expect(result).toEqual({ deleted_count: 3 });
		});

		it("should return 0 when no tags to cleanup", async () => {
			vi.mocked(api.post).mockResolvedValueOnce({
				data: { deleted_count: 0 },
			});

			const result = await tagsApi.cleanup();

			expect(result).toEqual({ deleted_count: 0 });
		});
	});
});
